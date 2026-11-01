//! Escoffier, from gcsim's internal/characters/escoffier: normal attacks,
//! the skill and its ticks (with its Surging Blade), the burst with its
//! heals, A1, A4 and constellations 1 to 6. Charged attacks, and
//! plunges from Xianyun's airborne state.
//! C1's CRIT DMG on Stellar reactions (gcsim's OnSpecialReactionAttack,
//! IsStellarReact) is for Stellar Swirl only, which is not ported (an error
//! with any anemo character in a Stellar team, `stellar.rs`), so it never
//! applies.

use crate::content::{Character, Event, EventKind, Owner};
use crate::gcsl::Num;
use crate::gcsimdata::chars::escoffier as g;
use crate::chars::plunge;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    skill_src: i32,
    skill_travel: i32,
    a1_src: i32,
    /// Hydro and cryo members, for A4's shred.
    hydro_cryo: usize,
    /// C1: every member is hydro or cryo (with A4).
    c1_active: bool,
    /// Cold Dishes left (C2), extra-heal procs left (C4), Frozen Parfaits
    /// left (C6).
    c2_count: i32,
    c4_count: i32,
    c6_count: i32,
}

const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const ALIGNED_ICD: ModKey = ModKey::new(g::SKILL_ALIGNED_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new(g::A1KEY);
const A4_CRYO: ModKey = ModKey::new("escoffier-a4-shred-cryo");
const A4_HYDRO: ModKey = ModKey::new("escoffier-a4-shred-hydro");
const C1: ModKey = ModKey::new(g::C1_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_A4: u16 = 1 << 1;

// arg: the normal's index * 8 + the hit's
const TASK_NORMAL: u8 = 0;
// arg: the skill's frame
const TASK_SKILL_TICK: u8 = 1;
const TASK_BLADE: u8 = 2;
const TASK_BURST_HIT: u8 = 3;
const TASK_BURST_HEAL: u8 = 4;
// arg: the burst's frame
const TASK_A1: u8 = 5;

fn ceil(x: f64) -> i32 {
    x.ceil() as i32
}

/// The airborne plunges (Xianyun's), from gcsim's plunge.go.
const PLUNGE: plunge::Spec = plunge::Spec { collision_offset: 0.0, ..plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION) };

impl Character for Char {
    const KEY: &'static str = "escoffier";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let n = s.chars.iter().filter(|c| matches!(c.element, Element::Hydro | Element::Cryo)).count();
        s.st_mut::<State>(ci).hydro_cryo = n;
        // C1 (with A4): only when every member is hydro or cryo
        if s.chars[ci].cons >= 1 && s.chars[ci].ascension >= 4 {
            s.st_mut::<State>(ci).c1_active = n == s.chars.len();
        }
        // C2 and C6: OnEnemyHit (gcsim subscribes C2, then C6; one
        // subscription handles both in that order)
        if s.chars[ci].cons >= 2 {
            s.subscribe(EventKind::EnemyHit, Owner::Char(ci as u8));
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let st = s.st::<State>(ci);
        let n = match fields[0].as_str() {
            "c2-count" => st.c2_count,
            "c4-count" => st.c4_count,
            "c6-count" => st.c6_count,
            _ => return None,
        };
        Some(Ok(Num::Int(n as i64)))
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// Plunges only while airborne (Xianyun's jump).
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge";
        // gcsim's HitlagHaltFrames is 0.10 here, in frames
        ai.hitlag_halt = 0.10;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.pattern = s.box_on_target(0.0, -1.2, 3.3, 3.5);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let hitmarks = g::ATTACK_HITMARKS[n];
        for (i, &h) in hitmarks.iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: (n * 8 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hitmarks[hitmarks.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(5, |t| t as i32);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_ESCOFFIER_SKILL, Element::Cryo, 25.0, g::SKILL_INITAL[lvl]);
        ai.abil = "Low-Temperature Cooking";
        ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
        let a4 = a4_cb(s, ci);
        s.queue_attack(ai, CB_PARTICLE | a4, g::SKILL_INIT_HITMARK, g::SKILL_INIT_HITMARK);
        // the ticks are not affected by hitlag
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        st.skill_travel = travel;
        st.skill_src = f;
        for i in 0..g::SKILL_TICKS {
            s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_TICK, arg: f }, g::SKILL_FIRST_TICK_DELAY + ceil(g::SKILL_INTERVAL * i as f64));
        }
        s.add_status(ci, SKILL, g::SKILL_FIRST_TICK_DELAY + ceil((g::SKILL_TICKS - 1) as f64 * g::SKILL_INTERVAL), false);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BLADE, arg: 0 }, g::SKILL_ALIGNED_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD, 22);
        c1(s, ci);
        // C2: five Cold Dishes for 15 s
        if s.chars[ci].cons >= 2 {
            s.add_status(ci, C2, g::C2_DUR, true);
            s.st_mut::<State>(ci).c2_count = 5;
        }
        // C6: six Frozen Parfaits
        if s.chars[ci].cons >= 6 {
            s.st_mut::<State>(ci).c6_count = g::C6_LIMIT;
        }
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_HIT, arg: 0 }, g::HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_HEAL, arg: 0 }, g::INITIAL_HEAL);
        // A1: heals over time
        if s.chars[ci].ascension >= 1 {
            let f = s.f;
            s.st_mut::<State>(ci).a1_src = f;
            // C4: six more ticks, and seven chances at an extra heal
            let mut ticks = g::A1_COUNT;
            if s.chars[ci].cons >= 4 {
                s.st_mut::<State>(ci).c4_count = g::C4_LIMIT;
                ticks += g::C4_EXTRA;
            }
            for i in 0..ticks {
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1, arg: f }, g::A1_FIRST_TICK + ceil(g::A1_INTERVAL * i as f64));
            }
            s.add_status(ci, A1, g::A1_FIRST_TICK + ceil((ticks - 1) as f64 * g::A1_INTERVAL), true);
        }
        s.set_cd(ci, Action::Burst, g::BURST_CD * 60);
        s.consume_energy(ci, 5);
        c1(s, ci);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = ((arg / 8) as usize, (arg % 8) as usize);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = 0.06 * 60.0;
                ai.def_halt = true;
                let (off, hb) = (g::ATTACK_OFFSETS[n][i], g::ATTACK_HITBOXES[n][i]);
                ai.pattern = if n == 0 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_SKILL_TICK => {
                if arg != s.st::<State>(ci).skill_src {
                    return;
                }
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_ESCOFFIER_SKILL, Element::Cryo, 25.0, g::SKILL_DOT[lvl]);
                ai.abil = "Frosty Parfait";
                ai.pattern = s.circle_on_target(1.5);
                let travel = s.st::<State>(ci).skill_travel;
                let a4 = a4_cb(s, ci);
                s.queue_attack(ai, a4, 0, travel);
            }
            TASK_BLADE => {
                if s.mod_active(ci, ALIGNED_ICD) {
                    return;
                }
                s.add_status(ci, ALIGNED_ICD, g::SKILL_ALIGNED_ICD, true);
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 0.0, g::ARKHE[lvl]);
                ai.abil = "Surging Blade (escoffier)";
                ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
                let a4 = a4_cb(s, ci);
                s.queue_attack(ai, a4, 0, 0);
            }
            TASK_BURST_HIT => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 50.0, g::BURST[lvl]);
                ai.abil = "Scoring Cuts";
                ai.pattern = s.circle_on_player(0.0, 1.0, 7.0);
                let a4 = a4_cb(s, ci);
                s.queue_attack(ai, a4, 0, 0);
            }
            TASK_BURST_HEAL => {
                let lvl = s.talent(ci, 2);
                let heal = g::BURST_HEAL_FLAT[lvl] + g::BURST_HEAL_PER[lvl] * total_atk(&s.stats(ci));
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, None, false, heal, bonus);
            }
            TASK_A1 => {
                if arg != s.st::<State>(ci).a1_src {
                    return;
                }
                let scale = g::A1_SCALING + c4_extra_heal(s, ci);
                let heal = scale * total_atk(&s.stats(ci));
                let bonus = s.stat(ci, st::HEAL);
                let active = s.player.active as usize;
                s.heal(ci, active, heal, bonus);
            }
            _ => unreachable!(),
        }
    }

    /// C1: CRIT DMG +60% on cryo attacks.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.element == Element::Cryo {
            ev.stats[st::CD] += 0.6;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        let actor = ev.info.actor as usize;
        if s.player.active as usize != actor {
            return;
        }
        // C2: a cryo hit of these kinds by the active character (not
        // Escoffier) adds 240% of her ATK, using up a Cold Dish
        if actor != ci
            && s.mod_active(ci, C2)
            && s.st::<State>(ci).c2_count > 0
            && ev.info.element == Element::Cryo
            && matches!(
                ev.info.tag,
                AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge | AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst
            )
        {
            let amt = total_atk(&s.stats(ci)) * g::C2_PER;
            s.st_mut::<State>(ci).c2_count -= 1;
            s.hit_flat += amt;
        }
        // C6: a normal, charged or plunging hit by the active character
        // while the skill lasts strikes with a Frozen Parfait, once per
        // 0.5 s, six times per skill
        if s.chars[ci].cons >= 6
            && matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge)
            && s.mod_active(ci, SKILL)
            && s.st::<State>(ci).c6_count > 0
            && !s.mod_active(ci, C6_ICD)
        {
            s.add_status(ci, C6_ICD, g::C6_ICD as i32, true);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_ESCOFFIER_SKILL, Element::Cryo, 25.0, g::C6_SCALING as f64);
            ai.abil = "Special-Grade Frozen Parfait (C6)";
            ai.pattern = s.circle_on_target(4.0);
            let travel = s.st::<State>(ci).skill_travel;
            let a4 = a4_cb(s, ci);
            s.queue_attack(ai, a4, 0, travel);
            s.st_mut::<State>(ci).c6_count -= 1;
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 12, false);
            s.queue_particle(4.0, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_A4 != 0 {
            let shred = g::A4_SHRED[s.st::<State>(ci).hydro_cryo];
            s.enemy_add_resist(A4_CRYO, Element::Cryo, -shred, g::A4_DUR);
            s.enemy_add_resist(A4_HYDRO, Element::Hydro, -shred, g::A4_DUR);
        }
    }
}

/// C1 (with A4, every member hydro or cryo): each member's cryo attacks
/// get CRIT DMG +60% for 15 s, extended by that member's hitlag.
fn c1(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 1 || s.chars[ci].ascension < 4 || !s.st::<State>(ci).c1_active {
        return;
    }
    for i in 0..s.chars.len() {
        let m = Mod::owned(C1, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1);
        s.add_mod(i, m, g::C1_DUR);
    }
}

/// C4 (with A1): while procs are left, an A1 tick heals 100% of her ATK
/// more and gives her 2 energy, at her CRIT Rate.
fn c4_extra_heal(s: &mut Sim, ci: usize) -> f64 {
    if s.chars[ci].cons < 4 || s.st::<State>(ci).c4_count <= 0 {
        return 0.0;
    }
    if s.rng.float64() > s.stat(ci, st::CR) {
        return 0.0;
    }
    s.add_energy(ci, g::C4_REGEN);
    s.st_mut::<State>(ci).c4_count -= 1;
    g::C4_EXTRA_SCALING as f64
}

fn a4_cb(s: &Sim, ci: usize) -> u16 {
    if s.chars[ci].ascension >= 4 { CB_A4 } else { 0 }
}
