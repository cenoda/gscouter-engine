//! Sangonomiya Kokomi, from gcsim's internal/characters/kokomi: normal and
//! charged attacks, Kurage's Oath (the Bake-Kurage's ticks and heals),
//! Nereid's Ascension (the Ceremonial Garment's damage bonuses, heals and
//! its end on any swap), her passive (Healing Bonus +25%, CRIT Rate
//! -100%), A1, A4 and every constellation.
//!
//! The Bake-Kurage is one attack snapshot at the cast, applied again on
//! each tick with the burst's flat bonus of that moment (or the one
//! frozen when she left the field early in the jellyfish's first two
//! seconds), as gcsim re-applies its one AttackEvent.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::kokomi as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    skill_flat_dmg: f64,
    skill_last_used: i32,
    swap_early_f: i32,
    /// The Bake-Kurage's attack, snapshot at the cast.
    jelly: AttackEvent,
}

const SKILL_STATUS: ModKey = ModKey::new("kokomiskill");
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const PASSIVE: ModKey = ModKey::new("kokomi-passive");
const C4: ModKey = ModKey::new("kokomi-c4");
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const C6: ModKey = ModKey::new("kokomi-c6");

const CB_PARTICLE: u16 = 1 << 0;
const CB_BURST_HEAL: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;

/// The first tick (the fight's queue).
const TASK_FIRST_TICK: u8 = 0;
/// The later ticks, with the cast's frame (the fight's queue).
const TASK_TICK: u8 = 1;
/// A1: the Bake-Kurage refreshed by the burst.
const TASK_A1: u8 = 2;

const FR_CHARGE: u8 = 0;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];

impl Character for Char {
    const KEY: &'static str = "sangonomiyakokomi";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            skill_flat_dmg: 0.0,
            skill_last_used: 0,
            swap_early_f: 0,
            jelly: AttackEvent::new(AttackInfo::new(0, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0), 0),
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        // passive: Healing Bonus +25% and CRIT Rate -100%
        s.chars[ci].mods.push(Mod::owned(PASSIVE, ModKind::Stat, o, st::NONE, false, false, -1));
        // onExitField
        s.subscribe(EventKind::Swap, o);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack => p.0.iter().find(|(k, _)| k != "travel").map(|(k, _)| format!("{name} param {k}")),
            Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let h = g::ATTACK_HITMARKS[n];
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.flat_dmg = burst_dmg_bonus(s, ci, AttackTag::Normal);
        let burst = s.core_status_key(BURST) > 0;
        ai.pattern = s.circle_on_target(if burst { 1.2 } else { 0.7 });
        let cbs = CB_BURST_HEAL | if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
        s.queue_attack(ai, cbs, h, h + travel);
        if n as i32 == s.chars[ci].normal_hit_num - 1 {
            c1(s, ci, h, travel);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    /// The charged attack, 14 frames sooner out of a normal attack.
    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge";
        ai.flat_dmg = burst_dmg_bonus(s, ci, AttackTag::Extra);
        let windup = if s.current_state() == AnimState::Normal { 14 } else { 0 };
        let burst = s.core_status_key(BURST) > 0;
        ai.pattern = s.circle_on_target(if burst { 4.0 } else { 3.5 });
        let cbs = CB_BURST_HEAL | if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
        let h = g::CHARGE_HITMARK - windup;
        s.queue_attack(ai, cbs, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: FR_CHARGE, arg: windup as i16 }, t[0] - windup, h, AnimState::Charge)
    }

    /// Kurage's Oath: the Bake-Kurage, a tick now and every 2 s.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_core_status(SKILL_STATUS, 12 * 60 + 30 + 1);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::SKILL_DMG[lvl]);
        ai.abil = "Bake-Kurage";
        ai.pattern = s.circle_on_player(0.0, 3.0, 6.0);
        let jelly = s.snapshot_event(ai, CB_PARTICLE);
        let f = s.f;
        let flat = burst_dmg_bonus(s, ci, AttackTag::ElementalArt);
        let st = s.st_mut::<State>(ci);
        st.jelly = jelly;
        st.swap_early_f = -1;
        st.skill_last_used = f;
        st.skill_flat_dmg = flat;
        s.add_task(Task::Char { c: ci as u8, id: TASK_FIRST_TICK, arg: 0 }, g::SKILL_HITMARK);
        s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg: f }, g::SKILL_HITMARK + 126);
        s.set_cd_with_delay(ci, Action::Skill, 20 * 60, 20);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::SKILL_HITMARK, AnimState::Skill)
    }

    /// Nereid's Ascension: the hit (from her max HP now) and the Ceremonial
    /// Garment for 10 s.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Hydro, 50.0, 0.0);
        ai.abil = "Nereid's Ascension";
        ai.flat_dmg = g::BURST_DMG[lvl] * s.max_hp(ci);
        ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.add_core_status(BURST, 10 * 60);
        let flat = burst_dmg_bonus(s, ci, AttackTag::ElementalArt);
        s.st_mut::<State>(ci).skill_flat_dmg = flat;
        if s.chars[ci].ascension >= 1 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, 46);
        }
        // C4: ATK SPD +10% for 10 s
        if s.chars[ci].cons >= 4 {
            s.add_stat_mod(ci, C4, st::ATKSPD, 0.1, 10 * 60, false);
        }
        s.consume_energy(ci, 57);
        s.set_cd_with_delay(ci, Action::Burst, 18 * 60, 46);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_CHARGE => g::CHARGE_FRAMES[next as usize] - arg as i32,
            _ => unreachable!(),
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_FIRST_TICK => skill_tick(s, ci),
            TASK_TICK => {
                if s.core_status_key(SKILL_STATUS) == 0 || s.st::<State>(ci).skill_last_used > arg {
                    return;
                }
                skill_tick(s, ci);
                s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg }, 120);
            }
            // A1: the Bake-Kurage's duration refreshed
            TASK_A1 => {
                if s.core_status_key(SKILL_STATUS) > 0 {
                    s.add_core_status(SKILL_STATUS, 12 * 60 + 1);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 60, false);
            if s.rng.float64() < 0.67 {
                s.queue_particle(1.0, Element::Hydro, s.particle_delay[ci]);
            }
        }
        if cbs & CB_BURST_HEAL != 0 && s.core_status_key(BURST) > 0 {
            burst_heal(s, ci);
        }
        // C4: 0.8 energy, once in 0.2 s
        if cbs & CB_C4 != 0 && s.core_status_key(BURST) > 0 && !s.mod_active(ci, C4_ICD) {
            s.add_status(ci, C4_ICD, 12, true);
            s.add_energy(ci, 0.8);
        }
    }

    fn mod_amount(_: &Sim, _: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == PASSIVE {
            out[st::HEAL] += 0.25;
            out[st::CR] += -1.0;
        }
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // A4: her normal and charged hits in the Garment add 15% of her
            // Healing Bonus times her max HP
            Event::EnemyHit(ev) => {
                if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) || s.core_status_key(BURST) == 0 {
                    return;
                }
                s.hit_flat += s.stat(ci, st::HEAL) * 0.15 * s.max_hp(ci);
            }
            // onExitField: any swap ends the Garment; leaving the field
            // freezes the Bake-Kurage's bonus
            Event::Swap { prev, .. } => {
                if prev == ci {
                    let f = s.f;
                    let flat = burst_dmg_bonus(s, ci, AttackTag::ElementalArt);
                    let st = s.st_mut::<State>(ci);
                    st.swap_early_f = f;
                    st.skill_flat_dmg = flat;
                }
                s.delete_core_status(BURST);
            }
            _ => {}
        }
    }
}

/// burstDmgBonus: the Garment's flat bonus from her max HP now.
fn burst_dmg_bonus(s: &Sim, ci: usize, tag: AttackTag) -> f64 {
    if s.core_status_key(BURST) == 0 {
        return 0.0;
    }
    let lvl = s.talent(ci, 2);
    match tag {
        AttackTag::Normal => g::BURST_BONUS_NORMAL[lvl] * s.max_hp(ci),
        AttackTag::Extra => g::BURST_BONUS_CHARGE[lvl] * s.max_hp(ci),
        AttackTag::ElementalArt => g::BURST_BONUS_SKILL[lvl] * s.max_hp(ci),
        _ => 0.0,
    }
}

/// A tick of the Bake-Kurage: its damage now, and a heal of the active
/// character when the player stands in its area.
fn skill_tick(s: &mut Sim, ci: usize) {
    let st = s.st::<State>(ci);
    let flat = if st.swap_early_f > st.skill_last_used && st.swap_early_f < st.skill_last_used + 100 { st.skill_flat_dmg } else { burst_dmg_bonus(s, ci, AttackTag::ElementalArt) };
    let mut ev = s.st::<State>(ci).jelly;
    ev.info.flat_dmg = flat;
    s.st_mut::<State>(ci).jelly.info.flat_dmg = flat;
    s.queue_attack_with_snap(ev, CB_PARTICLE, 0);
    if !s.player_within(ev.info.pattern) {
        return;
    }
    let lvl = s.talent(ci, 1);
    let maxhp = max_hp(&ev.stats);
    let mut src = g::SKILL_HEAL_PCT[lvl] * maxhp + g::SKILL_HEAL_FLAT[lvl];
    // C2: +4.5% of her max HP on an active character at half HP or less
    let active = s.player.active as usize;
    if s.chars[ci].cons >= 2 && s.chars[active].hp_ratio <= 0.5 {
        src += 0.045 * maxhp;
    }
    s.heal(ci, active, src, ev.stats[st::HEAL]);
}

/// makeBurstHealCB: on the first hit, a heal of every member (C2's bonus
/// at half HP or less), then C6.
fn burst_heal(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 2);
    let maxhp = s.max_hp(ci);
    let heal = g::BURST_HEAL_PCT[lvl] * maxhp + g::BURST_HEAL_FLAT[lvl];
    for i in 0..s.chars.len() {
        let mut src = heal;
        if s.chars[ci].cons >= 2 && s.chars[i].hp_ratio <= 0.5 {
            src += 0.006 * s.max_hp(ci);
        }
        let bonus = s.stat(ci, st::HEAL);
        s.heal(ci, i, src, bonus);
    }
    // C6: HYDRO DMG +40% for 8 s when a member is at 80% HP or more
    if s.chars[ci].cons >= 6 && s.chars.iter().any(|c| c.hp_ratio >= 0.8) {
        s.add_mod(ci, Mod::stat(C6, st::HYDROP, 0.4, true, false, -1), 480);
    }
}

/// C1: in the Garment, the third normal attack adds a hydro hit of 30% of
/// her max HP.
fn c1(s: &mut Sim, ci: usize, f: i32, travel: i32) {
    if s.chars[ci].cons < 1 || s.core_status_key(BURST) == 0 {
        return;
    }
    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
    ai.abil = "At Water's Edge (C1)";
    ai.flat_dmg = 0.3 * s.max_hp(ci);
    ai.pattern = s.circle_on_target(1.2);
    s.queue_attack(ai, 0, f, f + travel);
}
