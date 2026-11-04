//! Flins, from gcsim's internal/characters/flins: normal and charged
//! attacks, Ancient Rite: Arcane Light with Manifest Flame's normal and
//! charged attacks and Northland Spearstorm, Ancient Ritual: Cometh the
//! Night with Thunderous Symphony, A1, A4, his Lunar-Charged bonus, and
//! every constellation. No plunges (gcsim has none for him).
//!
//! He enables Lunar-Charged for the fight, and his Moonsign counts toward
//! Ascendant Gleam. As in gcsim:
//! - In Manifest Flame (10 s and the hitmark, from the skill) the skill is
//!   Northland Spearstorm, ready whenever its own cooldown (6 s, 4 s with
//!   C1) is not running, past the skill's 16 s cooldown; leaving the field
//!   ends Manifest Flame. Spearstorm opens Thunderous Symphony for 6 s:
//!   the burst is then Thunderous Symphony, ready with 30 energy, past the
//!   burst's cooldown, and drains 30 energy 3 frames later.
//! - Manifest Flame's charged attack takes the normal attack's talent
//!   level for its skill multiplier.
//! - C2's hit comes from the first Manifest Flame normal attack hit queued
//!   while C2's status lasts (gcsim's c2MakeAtkCB runs at each hit's
//!   QueueAttack, at its hitmark).
//! - C6's elevation goes on direct Lunar-Charged hits as they are applied
//!   (OnApplyAttack) and on Lunar-Charged contributions.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::flins as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {}

const A1: ModKey = ModKey::new(g::A1_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const SPEAR_STORM_CD: ModKey = ModKey::new(g::SPEAR_STORM_CD_KEY);
const SYMPHONY: ModKey = ModKey::new(g::THUNDEROUS_SYMPHONY_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C4: ModKey = ModKey::new(g::C4_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_C2: u16 = 1 << 1;

// arg: the normal counter << 4 | the hit
const TASK_SKILL_HIT: u8 = 0;
const TASK_SPEAR_STORM_CD_END: u8 = 1;
const TASK_BURST: u8 = 2;
const TASK_DRAIN: u8 = 3;

const SKILL_NORMAL_NAMES: [&str; 5] = ["Normal 0 (Skill)", "Normal 1 (Skill)", "Normal 2 (Skill)", "Normal 3 (Skill)", "Normal 4 (Skill)"];

impl Character for Char {
    const KEY: &'static str = "flins";
    const MOONSIGN: i32 = 1;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    /// gcsim's Init: leaving the field, A1, A4, the Lunar-Charged bonus,
    /// C1, C2 with Ascendant Gleam, C4, C6.
    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        s.subscribe(EventKind::Swap, o);
        if s.chars[ci].ascension >= 1 {
            s.chars[ci].mods.push(Mod::owned(A1, ModKind::ReactBonus, o, st::NONE, false, false, -1));
        }
        if s.chars[ci].ascension >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Stat, o, st::EM, true, false, -1));
        }
        s.enable_lunar_charge();
        s.subscribe(EventKind::EnemyHit, o);
        // the Lunar-Charged bonus's and C6's subscriptions, which only add
        s.subscribe(EventKind::SpecialReactionAttack, o);
        if s.chars[ci].cons >= 1 {
            s.subscribe(EventKind::Reaction, o);
        }
        if s.chars[ci].cons >= 2 && moonsign_level(s) >= 2 {
            s.subscribe(EventKind::EnemyDamage, o);
        }
        if s.chars[ci].cons >= 4 {
            s.chars[ci].mods.push(Mod::stat(C4, st::ATKP, 0.2, false, false, -1));
        }
        if s.chars[ci].cons >= 6 {
            s.subscribe(EventKind::ApplyAttack, o);
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        if a == Action::Skill && s.mod_active(ci, SKILL) {
            return Some(s.mod_active(ci, SPEAR_STORM_CD).then_some(Failure::SkillCd));
        }
        if a == Action::Burst && s.mod_active(ci, SYMPHONY) {
            return Some((!s.ignore_burst_energy && s.chars[ci].energy < 30.0).then_some(Failure::InsufficientEnergy));
        }
        None
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let manifest = s.mod_active(ci, SKILL);
        let lvl = s.talent(ci, 0);
        for i in 0..g::ATTACK[n].len() {
            if manifest {
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_HIT, arg: (n << 4 | i) as i32 }, g::ATTACK_HITMARKS[n][i]);
                continue;
            }
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.pattern = normal_pattern(s, n, i);
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            s.queue_attack_at(ci, ai, 0, g::ATTACK_HITMARKS[n][i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = if s.mod_active(ci, SKILL) {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_POLE_EXTRA_ATTACK, Element::Electro, 25.0, g::SKILL_CHARGE[lvl]);
            ai.ignore_infusion = true;
            ai
        } else {
            AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGE[lvl])
        };
        ai.abil = "Charge";
        ai.pattern = s.box_on_target(0.0, 0.0, 4.0, 6.0);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, SKILL) {
            return spear_storm(s, ci);
        }
        // a hit of no damage (it stacks the Jade Spear)
        let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, 0.0);
        ai.abil = "Arcane Light (0 dmg)";
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
        s.queue_attack(ai, 0, g::SKILL_HITMARK, g::SKILL_HITMARK);
        s.add_status(ci, SKILL, 10 * 60 + g::SKILL_HITMARK, true);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SPEAR_STORM_CD_END, arg: 0 }, 10 * 60 + g::SKILL_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 16 * 60, g::SKILL_HITMARK);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, SYMPHONY) {
            return symphony(s, ci);
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST, arg: 0 }, g::BURST_HITMARK_INITIAL);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 4);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL_HIT => {
                let (n, i) = ((arg >> 4) as usize, (arg & 15) as usize);
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_ATTACK[n][i][lvl]);
                ai.abil = SKILL_NORMAL_NAMES[n];
                ai.pattern = normal_pattern(s, n, i);
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.def_halt = g::ATTACK_DEF_HALT[n][i];
                ai.ignore_infusion = true;
                // C2: the first hit queued while its status lasts
                let mut cbs = CB_PARTICLE;
                if s.chars[ci].cons >= 2 && s.mod_active(ci, C2) {
                    s.delete_mod(ci, C2);
                    cbs |= CB_C2;
                }
                s.queue_attack(ai, cbs, 0, 0);
            }
            TASK_SPEAR_STORM_CD_END => s.delete_mod(ci, SPEAR_STORM_CD),
            TASK_BURST => {
                let lvl = s.talent(ci, 2);
                let ap = s.circle_on_player(0.0, 0.0, 7.0);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::BURST_INIT[lvl]);
                ai.abil = "Cometh the Night (Initial)";
                ai.pattern = ap;
                s.queue_attack(ai, 0, 0, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::DirectLunarCharged, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, g::BURST_MID[lvl]);
                ai.abil = "Cometh the Night (Mid)";
                ai.ignore_def = 1.0;
                ai.pattern = ap;
                let mid = if moonsign_level(s) >= 2 { 4 } else { 2 };
                for &h in &g::BURST_HITMARK_MID[..mid] {
                    s.queue_attack(ai, 0, 0, h);
                }
                ai.abil = "Cometh the Night (Final)";
                ai.mult = g::BURST_FINAL[lvl];
                s.queue_attack(ai, 0, 0, g::BURST_HITMARK_FINAL);
            }
            TASK_DRAIN => s.consume_energy_partial(ci, 30.0),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 2 * 60, true);
            s.queue_particle(1.0, Element::Electro, s.particle_delay[ci]);
        }
        // C2: a direct Lunar-Charged hit around the enemy
        if cbs & CB_C2 != 0 {
            let mut ai = AttackInfo::new(ci, AttackTag::DirectLunarCharged, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, 0.5);
            ai.abil = "The Devil's Wall (C2)";
            ai.ignore_def = 1.0;
            ai.pattern = s.circle_on_target(2.5);
            s.queue_attack(ai, 0, 20, 20);
        }
    }

    /// A4: EM from 8% of his ATK without extra stats, up to 160 (10% and
    /// 220 with C4).
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let (scale, max) = if s.chars[ci].cons >= 4 { (0.1, 220.0) } else { (0.08, 160.0) };
        out[st::EM] += (total_atk(&s.select_stats(ci, [st::BASEATK, st::ATKP, st::ATK], true)) * scale).min(max);
        true
    }

    /// A1: his Lunar-Charged DMG, direct or not, +20% with Ascendant Gleam.
    fn react_bonus(s: &Sim, _: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if moonsign_level(s) < 2 {
            return 0.0;
        }
        if !matches!(info.tag, AttackTag::DirectLunarCharged | AttackTag::ReactionLunarCharge) {
            return 0.0;
        }
        0.2
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // leaving the field ends Manifest Flame
            Event::Swap { prev, .. } => {
                if prev != ci || !s.mod_active(ci, SKILL) {
                    return;
                }
                s.delete_mod(ci, SKILL);
                s.delete_mod(ci, SPEAR_STORM_CD);
                s.delete_mod(ci, SYMPHONY);
            }
            Event::EnemyHit(ev) => {
                if ev.info.tag == AttackTag::DirectLunarCharged {
                    s.hit_base_bonus += lc_bonus(s, ci);
                }
            }
            Event::SpecialReactionAttack(ev) => {
                if ev.info.tag != AttackTag::ReactionLunarCharge {
                    return;
                }
                s.special_base_bonus += lc_bonus(s, ci);
                if s.chars[ci].cons >= 6 {
                    s.special_atk.elevation += c6_elevation(s, ci, ev.info.actor as usize);
                }
            }
            Event::ApplyAttack(ev) => {
                if ev.info.tag == AttackTag::DirectLunarCharged {
                    s.apply_elevation += c6_elevation(s, ci, ev.info.actor as usize);
                }
            }
            // C1: 8 energy on a Lunar-Charged on the enemy, once every 5.5 s
            Event::Reaction(Reaction::LunarCharged, ev) if ev.on_enemy() => {
                if s.mod_active(ci, C1_ICD) {
                    return;
                }
                s.add_energy(ci, 8.0);
                s.add_status(ci, C1_ICD, 330, true);
            }
            // C2 with Ascendant Gleam: his electro damage on field takes
            // 25% of the enemy's electro RES for 7 s
            Event::EnemyDamage(ev) => {
                if s.player.active as usize != ci || ev.info.actor as usize != ci || ev.info.element != Element::Electro {
                    return;
                }
                s.enemy_add_resist(C2, Element::Electro, -0.25, 7 * 60);
            }
            _ => {}
        }
    }
}

/// gcsim's GetMoonsignLevel: the Moonsign of the team.
fn moonsign_level(s: &Sim) -> i32 {
    s.chars.iter().map(|c| c.moonsign).sum()
}

/// His Lunar-Charged base DMG bonus: 0.7% per 100 ATK, up to 14%.
fn lc_bonus(s: &Sim, ci: usize) -> f64 {
    (s.total_atk(ci) / 100.0 * 0.007).min(0.14)
}

/// C6's elevation, in thousandths, of a Lunar-Charged hit by `actor`: 35%
/// for his own, 45% with Ascendant Gleam, which gives the others' 10%.
fn c6_elevation(s: &Sim, ci: usize, actor: usize) -> u16 {
    let gleam = moonsign_level(s) >= 2;
    if actor == ci {
        return if gleam { 450 } else { 350 };
    }
    if gleam {
        100
    } else {
        0
    }
}

/// A normal attack hit's area: boxes for the first two, circles after.
fn normal_pattern(s: &Sim, n: usize, i: usize) -> Pattern {
    let off = g::ATTACK_OFFSETS[n][i];
    let hb = g::ATTACK_HITBOXES[n][i];
    if n <= 1 {
        s.box_on_player(off[0], off[1], hb[0], hb[1])
    } else {
        s.circle_on_player(off[0], off[1], hb[0])
    }
}

/// Northland Spearstorm, Manifest Flame's skill.
fn spear_storm(s: &mut Sim, ci: usize) -> ActInfo {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_DMG[lvl]);
    ai.abil = "Northland Spearstorm";
    ai.pattern = s.box_on_player(0.0, 0.0, 4.0, 6.0);
    s.queue_attack(ai, 0, g::SPEAR_STORM_HITMARK, g::SPEAR_STORM_HITMARK);
    let cd = if s.chars[ci].cons >= 1 { 4 * 60 } else { 6 * 60 };
    s.add_status(ci, SPEAR_STORM_CD, cd, false);
    s.add_status(ci, SYMPHONY, 6 * 60, true);
    if s.chars[ci].cons >= 2 {
        s.add_status(ci, C2, 6 * 60, true);
    }
    let t = &g::SPEAR_STORM_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
}

/// Thunderous Symphony, the burst after Spearstorm.
fn symphony(s: &mut Sim, ci: usize) -> ActInfo {
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::DirectLunarCharged, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, g::SYMPHONY[lvl]);
    ai.abil = "Thunderous Symphony";
    ai.ignore_def = 1.0;
    ai.pattern = s.circle_on_player(0.0, 0.0, 7.0);
    s.queue_attack(ai, 0, g::SYMPHONY_HITMARK, g::SYMPHONY_HITMARK);
    if moonsign_level(s) >= 2 && s.core_status_key(LC_CLOUD) > 0 {
        ai.mult = g::SYMPHONY_EXTRA[lvl];
        s.queue_attack(ai, 0, g::SYMPHONY_HITMARK, g::SYMPHONY_EXTRA_HITMARK);
    }
    s.add_task(Task::Char { c: ci as u8, id: TASK_DRAIN, arg: 0 }, 3);
    s.delete_mod(ci, SYMPHONY);
    let t = &g::SYMPHONY_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Burst)
}
