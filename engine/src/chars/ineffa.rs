//! Ineffa, from gcsim's internal/characters/ineffa: normal and charged
//! attacks, Cleaning Mode: Carrier Frequency with its shield and Birgitta's
//! discharges, Supreme Instruction: Cyclonic Exterminator, A1, A4, her
//! Lunar-Charged bonus, and every constellation. No plunges (gcsim has
//! none for her).
//!
//! She enables Lunar-Charged for the fight (`Sim::enable_lunar_charge`),
//! and her Moonsign counts toward Ascendant Gleam. Her Lunar-Charged base
//! DMG bonus (0.7% per 100 ATK, up to 14%) goes on every direct
//! Lunar-Charged hit through OnEnemyHit, and on every Lunar-Charged
//! contribution through OnSpecialReactionAttack, whoever deals them.
//!
//! As in gcsim: C4 restores 5 energy on every Lunar-Charged on an enemy (its
//! ICD status is set and never read); C6 needs C1's status, which her
//! shields give; C2's hit comes on the next hit on the enemy after the
//! burst's, or 1 s later, whichever is first. Its subscription to OnEnemyHit
//! is made at the first burst hit and keeps that place among the
//! subscribers (gcsim's Unsubscribe and Subscribe with the same key).

use crate::content::{Character, Event};
use crate::gcsimdata::chars::ineffa as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The frame Birgitta was last summoned (gcsim's birgittaSrc).
    birgitta_src: i32,
    /// C2: the index of its OnEnemyHit subscription once made, whether it
    /// waits for the next hit, whether its hit came, and the burst hit it
    /// belongs to (for its task).
    c2_sub: Option<usize>,
    c2_armed: bool,
    c2_done: bool,
    c2_token: i32,
}

const BIRGITTA: ModKey = ModKey::new(g::BIRGITTA_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const A4_BUFF: ModKey = ModKey::new("ineffa-a4-buff");
const C1: ModKey = ModKey::new(g::C1_KEY);
const C1_BUFF: ModKey = ModKey::new("ineffa-c1-buff");
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);
const SHIELD: ModKey = ModKey::new("ineffa-shield");

const CB_PARTICLE: u16 = 1 << 0;
const CB_C2: u16 = 1 << 1;

const TASK_SHIELD: u8 = 0;
const TASK_SUMMON: u8 = 1;
// arg: the birgitta_src of the summon
const TASK_DISCHARGE: u8 = 2;
const TASK_A1: u8 = 3;
const TASK_BURST_HIT: u8 = 4;
// arg: c2_token
const TASK_C2: u8 = 5;

impl Character for Char {
    const KEY: &'static str = "ineffa";
    const MOONSIGN: i32 = 1;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    /// gcsim's Init: A4, the Lunar-Charged bonus, C1, C4, C6.
    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        if s.chars[ci].ascension >= 4 {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(A4_BUFF, ModKind::Stat, o, st::EM, true, true, -1));
            }
        }
        s.enable_lunar_charge();
        s.subscribe(EventKind::EnemyHit, o);
        s.subscribe(EventKind::SpecialReactionAttack, o);
        if s.chars[ci].cons >= 1 {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(C1_BUFF, ModKind::ReactBonus, o, st::NONE, false, false, -1));
            }
        }
        if s.chars[ci].cons >= 4 {
            s.subscribe(EventKind::Reaction, o);
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let off = g::ATTACK_OFFSETS[n][i];
            let hb = g::ATTACK_HITBOXES[n][i];
            ai.pattern = if n > 1 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
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
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge";
        // gcsim gives 0.06 frames, not seconds
        ai.hitlag_halt = 0.06;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.pattern = s.circle_on_player(0.0, 0.5, 2.35);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_INITIAL[lvl]);
        ai.abil = "Cleaning Mode: Carrier Frequency";
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.5);
        s.queue_attack(ai, 0, g::SKILL_HITMARK, g::SKILL_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SHIELD, arg: 0 }, 4);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SUMMON, arg: 0 }, g::SKILL_HITMARK + 42);
        s.set_cd_with_delay(ci, Action::Skill, 16 * 60, 1);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_HIT, arg: 0 }, g::HITMARK);
        // A4: the EM buff for 20 s
        if s.chars[ci].ascension >= 4 {
            s.add_status(ci, A4, 20 * 60, true);
        }
        // C2: a shield at 37
        if s.chars[ci].cons >= 2 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SHIELD, arg: 0 }, 37);
        }
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.delete_mod(ci, BIRGITTA);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SUMMON, arg: 0 }, g::HITMARK);
        s.consume_energy(ci, 4);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SHIELD => add_shield(s, ci),
            TASK_SUMMON => {
                let f = s.f;
                s.st_mut::<State>(ci).birgitta_src = f;
                s.add_status(ci, BIRGITTA, 20 * 60, false);
                s.add_task(Task::Char { c: ci as u8, id: TASK_DISCHARGE, arg: f }, 42);
            }
            TASK_DISCHARGE => {
                if s.st::<State>(ci).birgitta_src != arg || !s.mod_active(ci, BIRGITTA) {
                    return;
                }
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_DO_T[lvl]);
                ai.abil = "Birgitta";
                ai.pattern = s.circle_on_target(4.0);
                s.queue_attack(ai, CB_PARTICLE, 0, 0);
                // A1: a direct Lunar-Charged hit 38 frames later while the
                // Lunar-Charged cloud lasts
                if s.chars[ci].ascension >= 1 && s.core_status_key(LC_CLOUD) != 0 {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, 38);
                }
                s.add_task(Task::Char { c: ci as u8, id: TASK_DISCHARGE, arg }, 119);
            }
            TASK_A1 => {
                let mut ai = AttackInfo::new(ci, AttackTag::DirectLunarCharged, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, 0.65);
                ai.abil = "Birgitta (A1)";
                ai.ignore_def = 1.0;
                ai.pattern = s.circle_on_target(4.0);
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_BURST_HIT => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::BURST[lvl]);
                ai.abil = "Supreme Instruction: Cyclonic Exterminator";
                ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
                let cb = if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
                s.queue_attack(ai, cb, 0, 0);
            }
            TASK_C2 => {
                if s.st::<State>(ci).c2_token == arg {
                    c2_hit(s, ci);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, true);
            if s.rng.float64() < 0.66 {
                s.queue_particle(1.0, Element::Electro, s.particle_delay[ci]);
            }
        }
        // C2: the burst's hit on the enemy waits for the next hit on it, or
        // 1 s
        if cbs & CB_C2 != 0 {
            let n = s.subs[EventKind::EnemyHit as usize].len();
            let st = s.st_mut::<State>(ci);
            st.c2_token += 1;
            st.c2_done = false;
            st.c2_armed = true;
            let token = st.c2_token;
            if st.c2_sub.is_none() {
                st.c2_sub = Some(n);
                s.subscribe(EventKind::EnemyHit, Owner::Char(ci as u8));
            }
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C2, arg: token }, 60);
        }
    }

    /// A4: the active character and Ineffa gain EM from 6% of her ATK
    /// without extra stats, while A4's status lasts.
    fn mod_amount(s: &Sim, ci: usize, on: usize, _: &Mod, out: &mut Stats) -> bool {
        if s.player.active as usize != on && ci != on {
            return false;
        }
        if !s.mod_active(ci, A4) {
            return false;
        }
        out[st::EM] += total_atk(&s.select_stats(ci, [st::BASEATK, st::ATKP, st::ATK], true)) * 0.06;
        true
    }

    /// C1: Lunar-Charged DMG, direct or not, +2.5% per 100 of her ATK up to
    /// 50%, while C1's status lasts.
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if !s.mod_active(ci, C1) {
            return 0.0;
        }
        if !matches!(info.tag, AttackTag::ReactionLunarCharge | AttackTag::DirectLunarCharged) {
            return 0.0;
        }
        (s.total_atk(ci) / 100.0 * 0.025).min(0.5)
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyHit(ev) => {
                // C2's own subscription
                if s.st::<State>(ci).c2_sub == Some(s.emit_sub) {
                    if s.st::<State>(ci).c2_armed {
                        c2_hit(s, ci);
                        s.st_mut::<State>(ci).c2_armed = false;
                    }
                    return;
                }
                if ev.info.tag == AttackTag::DirectLunarCharged {
                    s.hit_base_bonus += lc_bonus(s, ci);
                }
            }
            Event::SpecialReactionAttack(ev) => {
                if ev.info.tag != AttackTag::ReactionLunarCharge {
                    return;
                }
                let bonus = lc_bonus(s, ci);
                s.special_base_bonus += bonus;
                // C6: a direct Lunar-Charged hit around the player, once
                // every 3.5 s, while C1's status lasts
                if s.chars[ci].cons >= 6 && s.mod_active(ci, C1) && !s.mod_active(ci, C6_ICD) {
                    s.add_status(ci, C6_ICD, 210, true);
                    let mut ai = AttackInfo::new(ci, AttackTag::DirectLunarCharged, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, 1.35);
                    ai.abil = "A Dawning Morn for You (C6)";
                    ai.ignore_def = 1.0;
                    ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
                    s.queue_attack(ai, 0, 10, 10);
                }
            }
            // C4: 5 energy on each Lunar-Charged on an enemy
            Event::Reaction(Reaction::LunarCharged, ev) if ev.on_enemy() => {
                s.add_energy(ci, 5.0);
                s.add_status(ci, C4_ICD, 4 * 60, true);
            }
            _ => {}
        }
    }
}

/// Her Lunar-Charged base DMG bonus: 0.7% per 100 ATK, up to 14%.
fn lc_bonus(s: &Sim, ci: usize) -> f64 {
    (s.total_atk(ci) / 100.0 * 0.007).min(0.14)
}

/// Optical Flow Shield Barrier: from her ATK, electro, for 20 s, on the
/// active character; C1's status with it.
fn add_shield(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 1);
    let hp = g::SHIELD_ATK[lvl] * s.total_atk(ci) + g::SHIELD_FLAT[lvl];
    let f = s.f;
    s.add_shield(Shield { owner: ci as u8, key: SHIELD, typ: ShieldType::IneffaSkill, target: -1, ele: Element::Electro, hp, expiry: f + 20 * 60, em_bonus: 0.0 });
    if s.chars[ci].cons >= 1 {
        s.add_status(ci, C1, 20 * 60, true);
    }
}

/// C2's Punishment Edit: a direct Lunar-Charged hit around the enemy, once
/// per burst hit.
fn c2_hit(s: &mut Sim, ci: usize) {
    if s.st::<State>(ci).c2_done {
        return;
    }
    s.st_mut::<State>(ci).c2_done = true;
    let mut ai = AttackInfo::new(ci, AttackTag::DirectLunarCharged, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, 3.0);
    ai.abil = "Punishment Edit (C2)";
    ai.ignore_def = 1.0;
    ai.pattern = s.circle_on_target(6.0);
    s.queue_attack(ai, 0, 0, 0);
}
