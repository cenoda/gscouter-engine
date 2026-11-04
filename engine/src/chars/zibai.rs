//! Zibai, from gcsim's internal/characters/zibai: normal and charged
//! attacks (and their Geo forms in Lunar Phase Shift, with the fourth
//! attack's Lunar-Crystallize hit), the skill (Lunar Phase Shift, Phase
//! Shift Radiance and Spirit Steed's Stride), the burst, her Moonsign
//! (Lunar-Crystallize and its base DMG bonus), A1, A4 and every
//! constellation.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::zibai as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    radiance: f64,
    skill_src: i32,
    skills_used: i32,
    /// C6's elevation, in thousandths.
    c6_elev: u16,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const RADIANCE_NA_ICD: ModKey = ModKey::new(g::RADIANCE_NAICD_KEY);
const RADIANCE_LCR_ICD: ModKey = ModKey::new(g::RADIANCE_L_CR_ICD_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const C1: ModKey = ModKey::new(g::C1_KEY);
const C1_BUFF: ModKey = ModKey::new("zibai-c1-buff");
const C2_BUFF: ModKey = ModKey::new("zibai-c2-buff");
const C4: ModKey = ModKey::new(g::C4_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_RADIANCE: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;

/// The skill's start at its hitmark, with its src.
const TASK_SKILL_START: u8 = 0;
/// radianceTicker, with its src.
const TASK_TICK: u8 = 1;
/// A normal attack's hit: arg is counter * 4 + hit, + 32 in the mode.
const TASK_NORMAL: u8 = 2;
/// The fourth attack's Lunar-Crystallize hit.
const TASK_LAST: u8 = 3;
/// A Spirit Steed's Stride hit: arg is the hit.
const TASK_STRIDE: u8 = 4;
/// C1's end, a frame after its bonus counted.
const TASK_C1_END: u8 = 5;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];

impl Character for Char {
    const KEY: &'static str = "zibai";
    const MOONSIGN: i32 = 1;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { radiance: 0.0, skill_src: 0, skills_used: 0, c6_elev: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // onExitField
        s.subscribe(EventKind::Swap, o);
        // skillInit
        s.subscribe(EventKind::Reaction, o);
        // a1Init
        if asc >= 1 {
            s.subscribe(EventKind::MoondriftHarmony, o);
        }
        // a4Init: DEF +15% per other geo member, EM +60 per hydro member
        if asc >= 4 {
            s.add_mod(ci, Mod::owned(A4, ModKind::Stat, o, st::NONE, false, false, -1), -1);
        }
        // moonsignInit
        s.enable_lunar_crystallize();
        s.subscribe(EventKind::EnemyHit, o);
        s.subscribe(EventKind::SpecialReactionAttack, o);
        if cons >= 1 {
            s.add_mod(ci, Mod::owned(C1_BUFF, ModKind::ReactBonus, o, st::NONE, false, false, -1), -1);
        }
        if cons >= 2 {
            for i in 0..s.chars.len() {
                s.add_mod(i, Mod::owned(C2_BUFF, ModKind::ReactBonus, o, st::NONE, false, false, -1), -1);
            }
        }
        if cons >= 6 {
            s.subscribe(EventKind::ApplyAttack, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    /// In Lunar Phase Shift the skill is Spirit Steed's Stride, ready with
    /// 70 Radiance.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        if a == Action::Skill && s.mod_active(ci, SKILL) {
            if s.st::<State>(ci).radiance < 70.0 {
                return Some(Some(Failure::InsufficientStamina));
            }
            return Some(None);
        }
        None
    }

    /// C4: the sequence does not reset in Lunar Phase Shift.
    fn keeps_normal_counter(s: &Sim, ci: usize) -> bool {
        s.chars[ci].cons >= 4 && s.mod_active(ci, SKILL)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let mode = if s.mod_active(ci, SKILL) { 32 } else { 0 };
        for (i, &h) in g::ATTACK_HITMARKS[n].iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: mode + (n * 4 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], table[Action::Dash as usize], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mode = s.mod_active(ci, SKILL);
        for i in 0..2 {
            let mut ai = if mode {
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL_CHARGE[i][s.talent(ci, 1)]);
                ai.blunt = true;
                ai.poise = 60.0;
                ai.ignore_infusion = true;
                ai.use_def = true;
                ai
            } else {
                AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[i][s.talent(ci, 0)])
            };
            ai.abil = "Charge";
            ai.hitlag_factor = g::CHARGE_HIT_HALT_TIME[i];
            ai.hitlag_halt = g::CHARGE_HIT_HALT_TIME[i] * 60.0;
            ai.def_halt = g::CHARGE_CAN_BE_DEF_HALT[i];
            ai.pattern = s.circle_on_player(0.0, g::CHARGE_OFFSETS[i], g::CHARGE_RADIUS[i]);
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARKS[1], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, SKILL) {
            return stride(s, ci);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 0.0, 0.0);
        ai.abil = "Lunar Phase Shift (0 dmg)";
        ai.pattern = s.circle_on_player(0.0, 1.3, 3.1);
        let h = g::SKILL_HITMARK;
        s.queue_attack(ai, 0, h, h);
        s.add_status(ci, SKILL, 15 * 60 + h, true);
        s.set_cd_with_delay(ci, Action::Skill, 18 * 60, h);
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        st.skills_used = 0;
        st.skill_src = f;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_START, arg: f }, h);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        for i in 0..2 {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::BURST[i][lvl]);
            ai.abil = "Burst";
            ai.blunt = true;
            ai.poise = 75.0;
            ai.use_def = true;
            if i == 1 {
                ai.abil = "Burst (Lunar-Crystallize)";
                ai.tag = AttackTag::DirectLunarCrystallize;
                ai.durability = 0.0;
                ai.ignore_def = 1.0;
                ai.poise = 100.0;
            }
            ai.pattern = s.circle_on_player(0.0, 0.0, 6.7);
            let h = g::BURST_HITMARKS[i];
            s.queue_attack(ai, 0, h, h);
        }
        s.extend_mod(ci, SKILL, 102);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 7);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL_START => {
                radiance_tick(s, ci, arg);
                if s.chars[ci].ascension >= 1 {
                    s.add_status(ci, A1, 4 * 60, true);
                }
                if s.chars[ci].cons >= 1 {
                    add_radiance(s, ci, 100.0);
                    s.add_status(ci, C1, 15 * 60, true);
                }
            }
            TASK_TICK => radiance_tick(s, ci, arg),
            TASK_NORMAL => normal_hit(s, ci, arg),
            TASK_LAST => {
                let lvl = s.talent(ci, 0);
                let mut mult = g::SKILL_LAST_ATTACK_BONUS[lvl];
                // C4: Scattermoon Splendor, 250% of it
                if s.chars[ci].cons >= 4 && s.mod_active(ci, C4) {
                    s.delete_mod(ci, C4);
                    mult *= 1.0 + 1.5;
                }
                let mut ai = AttackInfo::new(ci, AttackTag::DirectLunarCrystallize, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, mult);
                ai.abil = "Normal 4 (Lunar-Crystallize)";
                ai.ignore_infusion = true;
                ai.use_def = true;
                ai.ignore_def = 1.0;
                ai.pattern = s.circle_on_player(0.0, 2.5, 4.0);
                s.queue_attack(ai, CB_PARTICLE | CB_RADIANCE, 0, 0);
            }
            TASK_STRIDE => {
                let i = arg as usize;
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL_STRIDE[i][lvl]);
                ai.abil = g::SKILL_ABIL1;
                ai.blunt = true;
                ai.poise = 75.0;
                ai.use_def = true;
                ai.hitlag_halt = 0.02 * 60.0;
                ai.hitlag_factor = 0.01;
                if i == 1 {
                    ai.abil = g::SKILL_ABIL2;
                    ai.tag = AttackTag::DirectLunarCrystallize;
                    ai.durability = 0.0;
                    ai.ignore_def = 1.0;
                    ai.flat_dmg += a1_stride_bonus(s, ci);
                    ai.poise = 50.0;
                }
                // the pattern as the skill was used, on the player
                ai.pattern = s.circle_on_player(0.0, 2.5, 3.5);
                s.queue_attack(ai, CB_PARTICLE | CB_C4, 0, 0);
            }
            TASK_C1_END => s.delete_mod(ci, C1),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) && s.rng.float64() <= 0.67 {
            s.add_status(ci, PARTICLE_ICD, 2 * 60, true);
            s.queue_particle(1.0, Element::Geo, s.particle_delay[ci]);
        }
        if cbs & CB_RADIANCE != 0 && !s.mod_active(ci, RADIANCE_NA_ICD) {
            s.add_status(ci, RADIANCE_NA_ICD, 30, true);
            add_radiance(s, ci, 5.0);
        }
        if cbs & CB_C4 != 0 && s.chars[ci].cons >= 4 {
            s.add_status(ci, C4, 30 * 60, false);
        }
        // C1's bonus counted on this hit (its ReactBonus queues its end)
        if ev.info.abil == g::SKILL_ABIL2 && s.chars[ci].cons >= 1 && s.mod_active(ci, C1) {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C1_END, arg: 0 }, 1);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key != A4 {
            return false;
        }
        let (mut geos, mut hydros) = (0.0, 0.0);
        for (i, c) in s.chars.iter().enumerate() {
            if i == ci {
                continue;
            }
            match c.element {
                Element::Hydro => hydros += 1.0,
                Element::Geo => geos += 1.0,
                _ => {}
            }
        }
        out[st::DEFP] += 0.15 * geos;
        out[st::EM] += 60.0 * hydros;
        true
    }

    fn react_bonus(s: &Sim, ci: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if m.key == C1_BUFF {
            if info.actor as usize != ci || !s.mod_active(ci, C1) || info.abil != g::SKILL_ABIL2 {
                return 0.0;
            }
            2.2
        } else if m.key == C2_BUFF {
            if !s.mod_active(ci, SKILL) || !matches!(info.tag, AttackTag::ReactionLunarCrystallize | AttackTag::DirectLunarCrystallize) {
                return 0.0;
            }
            0.3
        } else {
            0.0
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { prev, .. } => {
                if prev != ci {
                    return;
                }
                s.delete_mod(ci, SKILL);
                if s.chars[ci].cons >= 4 {
                    s.delete_mod(ci, C4);
                    s.chars[ci].normal_counter = 0;
                }
            }
            Event::Reaction(Reaction::LunarCrystallize, ev) => {
                if !ev.on_enemy() || !s.mod_active(ci, SKILL) || s.mod_active(ci, RADIANCE_LCR_ICD) {
                    return;
                }
                s.add_status(ci, RADIANCE_LCR_ICD, 4 * 60, true);
                add_radiance(s, ci, 35.0);
            }
            Event::MoondriftHarmony(_) => s.add_status(ci, A1, 4 * 60, true),
            // Moonsign: Lunar-Crystallize's base DMG +0.7% per 100 DEF, up
            // to 14%, on direct hits and reaction contributions
            Event::EnemyHit(ev) => {
                if ev.info.tag == AttackTag::DirectLunarCrystallize {
                    s.hit_base_bonus += lcr_bonus(s, ci);
                }
            }
            Event::SpecialReactionAttack(ev) => {
                if ev.info.tag == AttackTag::ReactionLunarCrystallize {
                    s.special_base_bonus += lcr_bonus(s, ci);
                    // C6's elevation on her own contributions
                    if s.chars[ci].cons >= 6 && s.mod_active(ci, C6) && ev.info.actor as usize == ci {
                        s.special_atk.elevation += s.st::<State>(ci).c6_elev;
                    }
                }
            }
            // C6's elevation on her direct Lunar-Crystallize hits
            Event::ApplyAttack(ev) => {
                if ev.info.tag == AttackTag::DirectLunarCrystallize && s.mod_active(ci, C6) && ev.info.actor as usize == ci {
                    let slot = s.apply_slot as usize;
                    s.slots[slot].elevation += s.st::<State>(ci).c6_elev;
                }
            }
            _ => {}
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<crate::gcsl::Num, String>> {
        if fields[0] != "radiance" {
            return None;
        }
        let r = if s.mod_active(ci, SKILL) { s.st::<State>(ci).radiance } else { 0.0 };
        Some(Ok(crate::gcsl::Num::Float(r)))
    }
}

fn lcr_bonus(s: &Sim, ci: usize) -> f64 {
    (total_def(&s.stats(ci)) / 100.0 * 0.007).min(0.14)
}

fn moonsign_level(s: &Sim) -> i32 {
    s.chars.iter().map(|c| c.moonsign).sum()
}

fn add_radiance(s: &mut Sim, ci: usize, amt: f64) {
    let eff = if s.chars[ci].cons >= 6 { 1.5 } else { 1.0 };
    let st = s.st_mut::<State>(ci);
    st.radiance = (st.radiance + amt * eff).min(g::MAX_RADIANCE as f64);
}

/// 1 Radiance every 6 frames in Lunar Phase Shift, on the fight's queue.
fn radiance_tick(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).skill_src != src || !s.mod_active(ci, SKILL) {
        return;
    }
    add_radiance(s, ci, 1.0);
    s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg: src }, 6);
}

/// A1: the Stride's second hit +60% of her DEF while Selenic Descent
/// lasts (C2 with Ascendant Gleam: +550%).
fn a1_stride_bonus(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 1 || !s.mod_active(ci, A1) {
        return 0.0;
    }
    let c2 = if s.chars[ci].cons >= 2 && moonsign_level(s) >= 2 { g::C2_MULT - g::A4_MULT } else { 0.0 };
    (g::A4_MULT + c2) * total_def(&s.stats(ci))
}

/// Spirit Steed's Stride: two hits, the second Lunar-Crystallize; it uses
/// 70 Radiance (C6: all of it), and the mode ends after four (C1: five).
fn stride(s: &mut Sim, ci: usize) -> ActInfo {
    for (i, &h) in g::SKILL_STRIDE_HITMARK.iter().enumerate() {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_STRIDE, arg: i as i32 }, h);
    }
    s.delete_mod(ci, RADIANCE_LCR_ICD);
    s.st_mut::<State>(ci).skills_used += 1;
    if s.chars[ci].cons < 6 {
        let st = s.st_mut::<State>(ci);
        st.radiance = (st.radiance - 70.0).max(0.0);
    } else {
        let st = s.st_mut::<State>(ci);
        st.c6_elev = (((st.radiance - 70.0) * 0.016).max(0.0) * 1000.0).round() as u16;
        st.radiance = 0.0;
        s.add_status(ci, C6, 3 * 60, true);
    }
    let max = if s.chars[ci].cons >= 1 { 5 } else { 4 };
    if s.st::<State>(ci).skills_used >= max {
        s.delete_mod(ci, SKILL);
    }
    let t = &g::SKILL_SKILL_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
}

/// A normal attack's hit, built as it lands; in Lunar Phase Shift, Geo
/// and the fourth with its Lunar-Crystallize hit 20 frames later.
fn normal_hit(s: &mut Sim, ci: usize, arg: i32) {
    let mode = arg & 32 != 0;
    let (n, i) = (((arg & 31) / 4) as usize, (arg % 4) as usize);
    let mut ai = if mode {
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL_ATTACK[n][i][s.talent(ci, 0)]);
        ai.blunt = true;
        ai.poise = g::ATTACK_POISE_DMG[n][i];
        ai.ignore_infusion = true;
        ai.use_def = true;
        ai.hitlag_halt = g::SKILL_ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
        ai
    } else {
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][s.talent(ci, 0)]);
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
        ai
    };
    ai.abil = NORMAL_NAMES[n];
    ai.hitlag_factor = 0.01;
    ai.def_halt = g::ATTACK_DEF_HALT[n][i];
    let hb = g::ATTACK_HITBOXES[n];
    let off = g::ATTACK_OFFSETS[n][i];
    ai.pattern = if n == 1 || n == 2 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
    let cbs = if mode { CB_PARTICLE | CB_RADIANCE } else { 0 };
    s.queue_attack(ai, cbs, 0, 0);
    if mode && n == 3 && moonsign_level(s) >= 2 {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_LAST, arg: 0 }, 20);
    }
}
