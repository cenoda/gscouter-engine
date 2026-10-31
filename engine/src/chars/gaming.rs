//! Gaming, from gcsim's internal/characters/gaming: normal attacks, the
//! skill and the plunge that follows it (Charmed Cloudstrider), the burst
//! and Man Chai, A1, A4 and every constellation. Plunges other than the
//! skill's need Xianyun.

use crate::content::{template_invalid, Character, Event};
use crate::gcsimdata::chars::gaming as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The frames Man Chai takes to come back.
    man_chai_walk_back: i32,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const MAN_CHAI: ModKey = ModKey::new(g::MAN_CHAI_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const A4_HEAL: ModKey = ModKey::new("gaming-a4-heal-bonus");
const A4_DMG: ModKey = ModKey::new("gaming-a4-dmg-bonus");
const C2: ModKey = ModKey::new(g::C2_KEY);
const C4: ModKey = ModKey::new(g::C4_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_A1: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;

const TASK_AFTER_PLUNGE: u8 = 0;
const TASK_A1_HEAL: u8 = 1;
const TASK_BURST_START: u8 = 2;
const TASK_MAN_CHAI: u8 = 3;
const TASK_MAN_CHAI_BACK: u8 = 4;

impl Character for Char {
    const KEY: &'static str = "gaming";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].ascension >= 4 {
            let o = Owner::Char(ci as u8);
            s.chars[ci].mods.push(Mod::owned(A4_HEAL, ModKind::Stat, o, st::HEAL, false, false, -1));
            s.chars[ci].mods.push(Mod::owned(A4_DMG, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        if s.chars[ci].cons >= 2 {
            s.subscribe(EventKind::Heal, Owner::Char(ci as u8));
        }
        if s.chars[ci].cons >= 6 {
            s.chars[ci].mods.push(Mod::owned(C6, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1));
        }
        s.subscribe(EventKind::Swap, Owner::Char(ci as u8));
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst | Action::LowPlunge | Action::HighPlunge => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// Only the plunge right after the skill; the others need Xianyun.
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        match a {
            Action::LowPlunge if s.player.last.action != Action::Skill => Some("low_plunge can only be used while airborne".into()),
            Action::HighPlunge if s.player.last.action != Action::Skill => Some("high_plunge can only be used while airborne".into()),
            _ => template_invalid(s, a),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.blunt = true;
        ai.poise = g::ATTACK_POISE_DMG[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        let off = g::ATTACK_OFFSETS[n];
        ai.pattern = if n == 2 {
            s.box_on_player(0.0, off, g::ATTACK_HITBOXES[n][0], g::ATTACK_HITBOXES[n][1])
        } else {
            s.fan_on_player(0.0, off, g::ATTACK_HITBOXES[n][0], g::ATTACK_FAN_ANGLES[n])
        };
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD, g::SKILL_CD_START);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::LowPlunge as usize], AnimState::Skill)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        special_plunge(s, ci, p)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        special_plunge(s, ci, p)
    }

    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        s.st_mut::<State>(ci).man_chai_walk_back = p.get(g::MAN_CHAI_PARAM).filter(|&d| d > 0).map_or(100, |d| d as i32);
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_START, arg: 0 }, g::BURST_START);
        s.add_task(Task::Char { c: ci as u8, id: TASK_MAN_CHAI, arg: 0 }, g::BURST_HITMARK + 1);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BURST[lvl]);
        ai.abil = "Suanni's Gilded Dance";
        ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.set_cd(ci, Action::Burst, g::BURST_CD);
        s.consume_energy(ci, g::BURST_ENERGY_FRAME);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_AFTER_PLUNGE => {
                if s.mod_active(ci, BURST) && s.chars[ci].hp_ratio > 0.5 {
                    queue_man_chai(s, ci);
                }
                if s.chars[ci].hp_ratio > g::HP_DRAIN_THRESHOLD {
                    let max = s.max_hp(ci);
                    let current = s.current_hp(ci);
                    let mut drain = 0.15 * max;
                    if (current - drain) / max <= g::HP_DRAIN_THRESHOLD {
                        drain = current - g::HP_DRAIN_THRESHOLD * max;
                    }
                    s.drain(ci, drain);
                }
            }
            TASK_A1_HEAL => {
                if !s.mod_active(ci, A1) {
                    return;
                }
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, Some(ci), true, 0.015, bonus);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1_HEAL, arg: 0 }, 12);
            }
            TASK_BURST_START => {
                s.add_status(ci, BURST, g::BURST_DURATION, true);
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, Some(ci), true, 0.3, bonus);
            }
            TASK_MAN_CHAI => queue_man_chai(s, ci),
            TASK_MAN_CHAI_BACK => {
                if s.player.active as usize != ci {
                    return;
                }
                s.reset_cd(ci, Action::Skill);
                s.delete_mod(ci, MAN_CHAI);
                // C1
                if s.chars[ci].cons >= 1 {
                    let bonus = s.stat(ci, st::HEAL);
                    s.heal_info(ci, Some(ci), true, 0.15, bonus);
                }
            }
            _ => unreachable!(),
        }
    }

    /// A4's healing bonus, under half HP.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if s.chars[ci].hp_ratio >= 0.5 {
            return false;
        }
        out[st::HEAL] += 0.2;
        true
    }

    /// The plunge's DMG at half HP or more (A4), its CRIT from C6.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if ev.info.abil != g::SPECIAL_PLUNGE_KEY {
            return;
        }
        if m.key == A4_DMG {
            if s.chars[ci].hp_ratio >= 0.5 {
                ev.stats[st::DMGP] += 0.2;
            }
        } else {
            ev.stats[st::CR] += 0.2;
            ev.stats[st::CD] += 0.4;
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD, true);
            s.queue_particle(2.0, Element::Pyro, s.particle_delay[ci]);
        }
        if cbs & CB_A1 != 0 && !s.mod_active(ci, A1) {
            s.add_status(ci, A1, 48, true);
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1_HEAL, arg: 0 }, 12);
        }
        if cbs & CB_C4 != 0 && !s.mod_active(ci, C4) {
            s.add_status(ci, C4, 12, true);
            s.add_energy(ci, 2.0);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // C2: an overheal of Gaming's own heal target
            Event::Heal { target, overheal, .. } => {
                if overheal > 0.0 && target == Some(ci) {
                    s.add_stat_mod(ci, C2, st::ATKP, 0.2, 5 * 60, true);
                }
            }
            Event::Swap { prev, .. } => {
                if prev == ci && s.mod_active(ci, BURST) {
                    s.delete_mod(ci, BURST);
                }
            }
            _ => {}
        }
    }
}

fn special_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
    s.st_mut::<State>(ci).man_chai_walk_back = p.get(g::MAN_CHAI_PARAM).filter(|&d| d > 0).map_or(92, |d| d as i32);
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL[lvl]);
    ai.abil = g::SPECIAL_PLUNGE_KEY;
    ai.blunt = true;
    ai.poise = 150.0;
    ai.ignore_infusion = true;
    ai.pattern = s.circle_on_player(0.0, 0.0, if s.chars[ci].cons >= 6 { 6.0 } else { 4.0 });
    let a1 = if s.chars[ci].ascension >= 1 { CB_A1 } else { 0 };
    let c4 = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
    let h = g::SPECIAL_PLUNGE_HITMARK;
    s.queue_attack(ai, CB_PARTICLE | a1 | c4, h, h);
    s.add_task(Task::Char { c: ci as u8, id: TASK_AFTER_PLUNGE, arg: 0 }, h + 1);
    let t = &g::SPECIAL_PLUNGE_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Plunge)
}

fn queue_man_chai(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, MAN_CHAI) {
        return;
    }
    let back = s.st::<State>(ci).man_chai_walk_back;
    s.add_status(ci, MAN_CHAI, back, false);
    s.add_task(Task::Char { c: ci as u8, id: TASK_MAN_CHAI_BACK, arg: 0 }, back);
}
