//! Alyosha, from gcsim's internal/characters/alyosha: normal and charged
//! attacks, the skill (tap and hold) with the Hunter's Mark and Hunter's
//! Precision, the burst (the Fulgurite Hunting Field and Tugarin), his
//! Radiance: Stellar-Conduct bonus, A1, A4 and every constellation.

use crate::content::{Character, Event};
use crate::gamedata::chars::alyosha as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    skill_stacks: i32,
    burst_src: i32,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const MARK: ModKey = ModKey::new(g::SKILL_MARK_KEY);
const PRECISION: ModKey = ModKey::new(g::SKILL_BUFF_KEY);
const PRECISION_ATK: ModKey = ModKey::new("alyosha-hunters-precision-atkp");
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const SSC: ModKey = ModKey::new("alyosha-ssc");
const A4: ModKey = ModKey::new(g::A4_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C6_EM: ModKey = ModKey::new("alyosha-c6-em");

const CB_PARTICLE: u16 = 1 << 0;
/// triggerSkillMarkCB(true): activates a mark, or applies one.
const CB_MARK: u16 = 1 << 1;
/// triggerSkillMarkCB(false): only activates a mark.
const CB_MARK_ACTIVATE: u16 = 1 << 2;
/// C2: the field's tick applies a mark.
const CB_C2: u16 = 1 << 3;

/// The mark's check 6 frames after the hit: arg 1 can apply one.
const TASK_MARK: u8 = 0;
/// burstTicker, with its src.
const TASK_TICK: u8 = 1;
/// Tugarin's maul, 39 frames after the field's tick.
const TASK_DOG: u8 = 2;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];

impl Character for Char {
    const KEY: &'static str = "alyosha";
    const MOONSIGN: i32 = 1;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { skill_stacks: 0, burst_src: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let n = s.chars.len();
        // skillInit: Hunter's Precision's ATK for the active member
        for i in 0..n {
            s.add_mod(i, Mod::owned(PRECISION_ATK, ModKind::Stat, o, st::ATKP, false, false, -1), -1);
        }
        // a4Init
        if s.chars[ci].ascension >= 4 {
            s.add_mod(ci, Mod::owned(A4, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        }
        // stellarInit
        for i in 0..n {
            s.add_mod(i, Mod::owned(SSC, ModKind::ReactBonus, o, st::NONE, false, false, -1), -1);
        }
        // c1Init
        if s.chars[ci].cons >= 1 {
            s.subscribe(EventKind::Reaction, o);
        }
        // c6Init
        if s.chars[ci].cons >= 6 {
            for i in 0..n {
                s.add_mod(i, Mod::owned(C6_EM, ModKind::Stat, o, st::EM, false, false, -1), -1);
            }
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let allowed: &[&str] = match a {
            Action::Attack | Action::Charge | Action::Burst => &[],
            Action::Skill => &["hold"],
            _ => return Some(name.into()),
        };
        p.0.iter().find(|(k, _)| !allowed.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let marks = g::ATTACK_HITMARKS[n];
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let off = g::ATTACK_OFFSETS[n][i];
            let hb = g::ATTACK_HITBOXES[n][i];
            ai.pattern = if n == 1 || (n == 2 && i == 1) || n == 3 { s.box_on_player(off[0], off[1], hb[0], hb[1]) } else { s.circle_on_player(off[0], off[1], hb[0]) };
            let cb = if n == 3 { CB_MARK } else { 0 };
            s.queue_attack_at(ci, ai, cb, marks[i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *marks.last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = s.circle_on_target(0.8);
        s.queue_attack(ai, 0, 0, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        if p.get("hold").is_some_and(|h| h > 0) {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_HOLD[lvl]);
            ai.abil = "Skill (Hold)";
            ai.pattern = s.fan_on_player(0.0, 0.0, 15.0, 150.0);
            let h = g::SKILL_HOLD_HITMARK;
            s.queue_attack(ai, CB_PARTICLE | CB_MARK, h, h);
            s.set_cd_with_delay(ci, Action::Skill, 15 * 60, 111);
            let t = &g::SKILL_HOLD_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_TAP[lvl]);
        ai.abil = "Skill";
        ai.pattern = s.box_on_player(0.0, 0.0, 2.5, 10.0);
        let h = g::SKILL_HITMARK;
        s.queue_attack(ai, CB_PARTICLE | CB_MARK, h, h);
        s.set_cd_with_delay(ci, Action::Skill, 15 * 60, 16);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    /// Hunter's Advance: the field and Tugarin, every 118 frames from 79.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let f = s.f;
        s.st_mut::<State>(ci).burst_src = f;
        s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg: f }, g::FIELD_FIRST_TICK);
        let dur = 14 * 60 + if s.chars[ci].cons >= 2 { 6 * 60 } else { 0 };
        s.add_status(ci, BURST, dur, true);
        s.set_cd_with_delay(ci, Action::Burst, 18 * 60, 1);
        s.consume_energy(ci, 7);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_MARK => {
                if s.enemy_status_active(MARK) {
                    s.enemy_delete_status(MARK);
                    if !s.mod_active(ci, PRECISION) {
                        s.st_mut::<State>(ci).skill_stacks = 0;
                    }
                    let cap = 1 + if s.chars[ci].cons >= 6 { 1 } else { 0 };
                    let st = s.st_mut::<State>(ci);
                    st.skill_stacks = (st.skill_stacks + 1).min(cap);
                    s.add_status(ci, PRECISION, g::SKILL_BUFF_DUR, true);
                    return;
                }
                if arg == 1 {
                    s.enemy_add_status(MARK, g::SKILL_BUFF_DUR, true);
                }
            }
            TASK_TICK => {
                if s.st::<State>(ci).burst_src != arg || !s.mod_active(ci, BURST) {
                    return;
                }
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_ALYOSHA_BURST, Element::Electro, 25.0, g::BURST[lvl]);
                ai.abil = "Fulgurite Hunting Field";
                // the field stays where it was cast, on the enemy
                ai.pattern = s.circle_on_target(6.0);
                let cb = if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
                s.queue_attack(ai, cb, 0, 0);
                s.add_task(Task::Char { c: ci as u8, id: TASK_DOG, arg: 0 }, 39);
                s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg }, g::FIELD_INTERVAL);
            }
            TASK_DOG => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_ALYOSHA_BURST, Element::Electro, 25.0, g::BURST_TICK[lvl]);
                ai.abil = "Tugarin";
                ai.pattern = s.circle_on_target(1.0);
                s.queue_attack(ai, CB_MARK_ACTIVATE, 0, 0);
                // A1: Tugarin heals the active member by 120% of his ATK
                if s.chars[ci].ascension >= 1 {
                    let active = s.player.active as usize;
                    let src = 1.2 * s.total_atk(ci);
                    let bonus = s.stat(ci, st::HEAL);
                    s.heal(ci, active, src, bonus);
                }
                // C4: and the member with the lowest HP by 60%
                if s.chars[ci].cons >= 4 {
                    let mut lowest = ci;
                    let mut ratio = s.chars[ci].hp_ratio;
                    for i in 0..s.chars.len() {
                        if s.chars[i].hp_ratio < ratio {
                            lowest = i;
                            ratio = s.chars[i].hp_ratio;
                        }
                    }
                    let src = 0.6 * s.total_atk(ci);
                    let bonus = s.stat(ci, st::HEAL);
                    s.heal(ci, lowest, src, bonus);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 30, true);
            s.queue_particle(5.0, Element::Electro, s.particle_delay[ci]);
        }
        if cbs & (CB_MARK | CB_MARK_ACTIVATE) != 0 {
            let arg = i32::from(cbs & CB_MARK != 0);
            s.add_task(Task::Char { c: ci as u8, id: TASK_MARK, arg }, 6);
        }
        if cbs & CB_C2 != 0 {
            s.enemy_add_status(MARK, g::SKILL_BUFF_DUR, true);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool {
        if s.player.active as usize != on || !s.mod_active(ci, PRECISION) {
            return false;
        }
        let stacks = s.st::<State>(ci).skill_stacks;
        if m.key == PRECISION_ATK {
            out[st::ATKP] += g::SKILL_BUFF[s.talent(ci, 1)] * stacks as f64;
            true
        } else if m.key == C6_EM {
            if stacks < 2 {
                return false;
            }
            out[st::EM] += 100.0;
            true
        } else {
            false
        }
    }

    /// A4: skill and burst DMG +0.35% per 1% of his Energy Recharge, up to
    /// 70%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A4 && matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst) {
            ev.stats[st::DMGP] += (s.stat(ci, st::ER) / 0.01 * 0.0035).min(0.7);
        }
    }

    /// Radiance: Stellar-Conduct: the active member's Stellar-Conduct DMG
    /// +20% per stack of Hunter's Precision.
    fn react_bonus(s: &Sim, ci: usize, on: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if !s.mod_active(ci, crate::stellar::POLESTAR_FIELD) || !s.mod_active(ci, PRECISION) || info.tag != AttackTag::DirectStellarConduct || s.player.active as usize != on {
            return 0.0;
        }
        0.2 * s.st::<State>(ci).skill_stacks as f64
    }

    /// C1: 15 energy on an electro reaction on the enemy, once every 18 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        let electro = matches!(
            r,
            Reaction::Overload
                | Reaction::ElectroCharged
                | Reaction::LunarCharged
                | Reaction::Superconduct
                | Reaction::StellarConduct
                | Reaction::SwirlElectro
                | Reaction::CrystallizeElectro
                | Reaction::Hyperbloom
                | Reaction::Quicken
                | Reaction::Aggravate
        );
        if !electro || !ev.on_enemy() || s.mod_active(ci, C1_ICD) {
            return;
        }
        s.add_status(ci, C1_ICD, 18 * 60, true);
        s.add_energy(ci, 15.0);
    }
}
