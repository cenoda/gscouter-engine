//! Sethos, from gcsim's internal/characters/sethos: normal attacks and
//! their Dusk Bolts in Twilight Meditation, aimed shots (physical, charged
//! and the Shadowpiercing Shot, A1's charge skip on energy), Ancient Rite
//! (its particles and the energy its reactions refund), the burst, A4 and
//! every constellation.
//!
//! C4 gives its Elemental Mastery when one Dusk Bolt or Shadowpiercing
//! Shot hits two enemies; the engine's fights have one enemy (profile.rs
//! refuses others), so it never does, as in gcsim with one target.

use crate::content::{Character, Event, EventKind};
use crate::gamedata::chars::sethos as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The snapshot frame of the last skill hit that refunded energy (one
    /// refund an attack, gcsim's lastSkillFrame).
    last_skill_frame: i32,
    a4_count: i32,
    /// The Dusk Bolt the current normal attack's earlier hit became: its
    /// flat damage and pattern (gcsim's closures share one AttackInfo, so a
    /// later hit of the attack stays a Dusk Bolt once the burst ends).
    dusk: Option<(f64, Pattern)>,
    /// The energy each Shadowpiercing Shot took (C6 returns it), by the
    /// parity of the shot.
    c6_energy: [f64; 2],
    c6_parity: usize,
}

const BURST: ModKey = ModKey::new(g::BURST_BUFF_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::SKILL_PARTICLE_ICD_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const C1: ModKey = ModKey::new("sethos-c1");
const C2: ModKey = ModKey::new(g::C2_KEY);
const C2_CONSUMING: ModKey = ModKey::new(g::C2_CONSUMING_KEY);
const C2_REGAINING: ModKey = ModKey::new(g::C2_REGAINING_KEY);
const C2_BURST: ModKey = ModKey::new(g::C2_BURST_KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_A4: u16 = 1 << 1;
const CB_C6_0: u16 = 1 << 2;
const CB_C6_1: u16 = 1 << 3;

/// A normal attack's hit: arg is travel << 8 | counter << 4 | hit.
const TASK_NORMAL: u8 = 0;
/// An aimed shot: arg is travel << 8 | parity << 5 | weakspot << 4 | hold;
/// the energy A1 counted for it is in `c6_energy[parity]`.
const TASK_AIM: u8 = 1;
/// A4's buff comes back when its cooldown ends.
const TASK_A4: u8 = 2;

const FR_AIM: u8 = 0;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];
const DUSK_NAMES: [&str; 3] = ["Dusk Bolt 0", "Dusk Bolt 1", "Dusk Bolt 2"];

/// gcsim's attacks.AimParamPhys, AimParamLv1, AimParamLv2.
const AIM_PHYS: usize = 0;
const AIM_LV1: usize = 1;
const AIM_LV2: usize = 2;

impl Character for Char {
    const KEY: &'static str = "sethos";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { last_skill_frame: -1, a4_count: 0, dusk: None, c6_energy: [0.0; 2], c6_parity: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        s.subscribe(EventKind::Reaction, o);
        s.subscribe(EventKind::Swap, o);
        a4_start(s, ci);
        if s.chars[ci].cons >= 1 {
            s.chars[ci].mods.push(Mod::owned(C1, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        if s.chars[ci].cons >= 2 {
            s.chars[ci].mods.push(Mod::owned(C2, ModKind::Stat, o, st::ELECTROP, false, false, -1));
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            Action::Aim => p.0.iter().find(|(k, _)| !matches!(k.as_str(), "hold" | "travel" | "weakspot")).map(|(k, _)| format!("aim param {k}")),
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// gcsim's errors for an aim in the burst or with another hold.
    fn invalid(s: &Sim, ci: usize, a: Action, p: &Params) -> Option<String> {
        if a == Action::Aim {
            if s.mod_active(ci, BURST) {
                return Some("sethos: Cannot aim while in burst".into());
            }
            if let Some(h) = p.get("hold") {
                if !(0..=2).contains(&h) {
                    return Some(format!("invalid hold param supplied, got {h}"));
                }
            }
        }
        crate::content::template_invalid(s, a)
    }

    /// Normal attacks: each hit is decided at its hitmark, a Dusk Bolt
    /// while Twilight Meditation lasts.
    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        s.st_mut::<State>(ci).dusk = None;
        let h = g::ATTACK_HITMARKS[n];
        for (i, &hm) in h.iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: travel << 8 | (n as i32) << 4 | i as i32 }, hm);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h[h.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    /// Aimed shots: physical (hold 0), charged (hold 1), or the
    /// Shadowpiercing Shot (hold 2, the default); A1 skips 0.285 s of the
    /// charge per energy, up to 20 energy, which the shot then takes.
    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").map_or(AIM_LV2, |h| h as usize);
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let weakspot = p.get("weakspot") == Some(1);
        let (skip, energy) = a1_calc(s, ci);
        let skip = skip.min(g::AIMED_HITMARKS[hold] - g::START_CHARGE);
        let state = s.st_mut::<State>(ci);
        state.c6_parity ^= 1;
        let parity = state.c6_parity;
        state.c6_energy[parity] = energy;
        let arg = travel << 8 | (parity as i32) << 5 | i32::from(weakspot) << 4 | hold as i32;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_AIM, arg }, g::AIMED_HITMARKS[hold] - skip);
        let t = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Char { id: FR_AIM, arg: (hold as i16) << 10 | skip as i16 }, t[0] - skip, g::AIMED_HITMARKS[hold] - skip, AnimState::Aim)
    }

    /// Ancient Rite: Thunderous Roar of Sand, 2 particles.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL[lvl]);
        ai.abil = "Ancient Rite: Thunderous Roar of Sand";
        ai.pattern = s.circle_on_player(0.0, 0.0, 4.5);
        s.queue_attack(ai, CB_PARTICLE, 0, 13);
        s.set_cd_with_delay(ci, Action::Skill, 8 * 60, 10);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Walk as usize], AnimState::Skill)
    }

    /// Secret Rite: Twilight Shadowpiercer: 8 s of Dusk Bolts, ended by a
    /// swap.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_status(ci, BURST, 8 * 60, true);
        c2_add_stack(s, ci, C2_BURST);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 7);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_AIM => g::AIMED_FRAMES[(arg >> 10) as usize][next as usize] - (arg & 0x3ff) as i32,
            _ => unreachable!(),
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => normal_hit(s, ci, arg >> 8, ((arg >> 4) & 0xf) as usize, (arg & 0xf) as usize),
            TASK_AIM => aim_hit(s, ci, arg >> 8, (arg >> 5) & 1 == 1, (arg >> 4) & 1 == 1, (arg & 0xf) as usize),
            TASK_A4 => a4_start(s, ci),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 30, true);
            s.queue_particle(2.0, Element::Electro, s.particle_delay[ci]);
        }
        // A4: the first hit starts its 5 s and its 15 s cooldown; four
        // hits end it
        if cbs & CB_A4 != 0 && s.mod_active(ci, A4) {
            if s.st::<State>(ci).a4_count == 0 {
                s.add_status(ci, A4, 5 * 60, true);
                if !s.mod_active(ci, A4_ICD) {
                    s.add_status(ci, A4_ICD, g::A4_ICD, true);
                    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, g::A4_ICD);
                }
            }
            let state = s.st_mut::<State>(ci);
            state.a4_count += 1;
            if state.a4_count >= 4 {
                s.delete_mod(ci, A4);
            }
        }
        // C6: the shot's energy back, once every 15 s
        for (bit, parity) in [(CB_C6_0, 0), (CB_C6_1, 1)] {
            if cbs & bit != 0 && !s.mod_active(ci, C6_ICD) {
                s.add_status(ci, C6_ICD, g::C6_ICD_DUR, true);
                let energy = s.st::<State>(ci).c6_energy[parity];
                s.add_energy(ci, energy);
            }
        }
    }

    /// C2: 15% Electro DMG Bonus a stack, at most 2.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let stacks = [C2_CONSUMING, C2_REGAINING, C2_BURST].iter().filter(|&&k| s.mod_active(ci, k)).count().min(2);
        if stacks == 0 {
            return false;
        }
        out[st::ELECTROP] += 0.15 * stacks as f64;
        true
    }

    /// C1: the Shadowpiercing Shot's CRIT Rate +15%.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::Extra && ev.info.abil == g::SHADOW_PIERCE_SHOT_AIL {
            ev.stats[st::CR] += 0.15;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // the skill's refund: 12 energy once an attack, on any of these
            // reactions (gcsim does not check the target)
            Event::Reaction(r, ev) => {
                if !matches!(
                    r,
                    Reaction::Overload
                        | Reaction::ElectroCharged
                        | Reaction::LunarCharged
                        | Reaction::Superconduct
                        | Reaction::StellarConduct
                        | Reaction::SwirlElectro
                        | Reaction::Hyperbloom
                        | Reaction::Quicken
                        | Reaction::Aggravate
                ) {
                    return;
                }
                if ev.info.actor as usize != ci || ev.info.tag != AttackTag::ElementalArt {
                    return;
                }
                if s.st::<State>(ci).last_skill_frame == ev.snap_f {
                    return;
                }
                s.st_mut::<State>(ci).last_skill_frame = ev.snap_f;
                s.add_energy(ci, g::SKILL_ENERGY_REGEN as f64);
                c2_add_stack(s, ci, C2_REGAINING);
            }
            Event::Swap { .. } => {
                if s.mod_active(ci, BURST) {
                    s.delete_mod(ci, BURST);
                }
            }
            _ => {}
        }
    }
}

/// A1: the charge skipped (0.285 s an energy) and the energy it takes, up
/// to 20.
fn a1_calc(s: &Sim, ci: usize) -> (i32, f64) {
    if s.chars[ci].ascension < 1 {
        return (0, 0.0);
    }
    let energy = s.chars[ci].energy.min(20.0);
    ((0.285 * energy * 60.0) as i32, energy)
}

/// A1: the shot takes its energy (half for a charged shot); C2's stack.
fn a1_consume(s: &mut Sim, ci: usize, energy: f64, hold: usize) {
    match hold {
        AIM_LV1 => s.add_energy(ci, -energy * 0.5),
        AIM_LV2 => s.add_energy(ci, -energy),
        _ => return,
    }
    c2_add_stack(s, ci, C2_CONSUMING);
}

/// A4: the buff, until it is used (gcsim's a4, again after its cooldown).
fn a4_start(s: &mut Sim, ci: usize) {
    if s.chars[ci].ascension < 4 {
        return;
    }
    s.add_status(ci, A4, -1, true);
    s.st_mut::<State>(ci).a4_count = 0;
}

fn c2_add_stack(s: &mut Sim, ci: usize, key: ModKey) {
    if s.chars[ci].cons >= 2 {
        s.add_status(ci, key, g::C2_DUR, true);
    }
}

/// The box from the player through the enemy, 15 long (gcsim's piercing).
fn pierce_pattern(s: &Sim) -> Pattern {
    let (dx, dy) = (s.player_pos.0 - s.enemy_pos.0, s.player_pos.1 - s.enemy_pos.1);
    let dist = (dx * dx + dy * dy).sqrt();
    s.box_on_target(0.0, -dist, 0.1, 15.0)
}

fn normal_hit(s: &mut Sim, ci: usize, travel: i32, n: usize, i: usize) {
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
    ai.abil = NORMAL_NAMES[n];
    ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
    let dusk = if s.mod_active(ci, BURST) {
        let flat = g::BURST_EM[s.talent(ci, 2)] * s.stat(ci, st::EM);
        let d = (flat, pierce_pattern(s));
        s.st_mut::<State>(ci).dusk = Some(d);
        Some(d)
    } else {
        s.st::<State>(ci).dusk
    };
    if let Some((flat, pattern)) = dusk {
        ai.abil = DUSK_NAMES[n];
        ai.tag = AttackTag::Extra;
        ai.icd_tag = icd::TAG_ELEMENTAL_BURST;
        ai.element = Element::Electro;
        ai.flat_dmg = flat;
        ai.pattern = pattern;
    }
    s.queue_attack(ai, 0, 0, travel);
}

fn aim_hit(s: &mut Sim, ci: usize, travel: i32, parity: bool, weakspot: bool, hold: usize) {
    let lvl = s.talent(ci, 0);
    let energy = s.st::<State>(ci).c6_energy[usize::from(parity)];
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::FULLAIM[lvl]);
    ai.abil = "Fully-Charged Aimed Shot";
    ai.deployable = true;
    ai.hitlag_factor = 0.01;
    ai.weak_point = weakspot;
    if hold != AIM_LV2 {
        // HitlagOnHeadshotOnly
        if weakspot {
            ai.hitlag_halt = 0.12 * 60.0;
        }
        if hold == AIM_PHYS {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        s.queue_attack(ai, 0, 0, travel);
        a1_consume(s, ci, energy, hold);
        return;
    }
    // the Shadowpiercing Shot
    let em = s.stat(ci, st::EM);
    ai.abil = g::SHADOW_PIERCE_SHOT_AIL;
    ai.durability = 50.0;
    ai.mult = g::SHADOWPIERCE_ATK[lvl];
    ai.flat_dmg = g::SHADOWPIERCE_EM[lvl] * em;
    if weakspot {
        ai.hitlag_halt = 0.12 * 60.0;
    }
    if s.mod_active(ci, A4) {
        ai.flat_dmg += 7.0 * em;
    }
    ai.pattern = pierce_pattern(s);
    a1_consume(s, ci, energy, AIM_LV2);
    let mut cbs = 0;
    if s.chars[ci].ascension >= 4 {
        cbs |= CB_A4;
    }
    if s.chars[ci].cons >= 6 && s.chars[ci].ascension >= 1 {
        cbs |= if parity { CB_C6_1 } else { CB_C6_0 };
    }
    s.queue_attack(ai, cbs, 0, travel);
}
