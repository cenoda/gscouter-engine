//! Xiangling, from gcsim's internal/characters/xiangling: normal and
//! charged attacks, Guoba (a gadget with its pyro window, which absorb
//! checks read) and Pyronado, A1, A4's chili pepper (the skill's
//! `a4_delay`), C1, C2, C4 and C6.

use crate::content::Character;
use crate::gcsimdata::chars::xiangling as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// Guoba's breath, snapshot when Guoba spawns.
    guoba: AttackEvent,
    /// Pyronado's spin, snapshot when it starts.
    spin: AttackEvent,
}

const GUOBA_STATUS: ModKey = ModKey::new("xianglingguoba");
const BURST_STATUS: ModKey = ModKey::new("xianglingburst");
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const C1: ModKey = ModKey::new("xiangling-c1");
const C2_DEBUFF: ModKey = ModKey::new(g::C2_DEBUFF);
const C6: ModKey = ModKey::new("xiangling-c6");
const C6_STATUS: ModKey = ModKey::new("xlc6");
const A4: ModKey = ModKey::new("xiangling-a4");

const CB_C2: u16 = 1 << 0;
const CB_C1: u16 = 1 << 1;
const CB_PARTICLE: u16 = 1 << 2;

// arg: the normal's index * 8 + the hit's
const TASK_NORMAL: u8 = 0;
// arg: the a4 delay, -1 for none
const TASK_GUOBA: u8 = 1;
const TASK_GUOBA_AURA_END: u8 = 2;
const TASK_A4: u8 = 3;
const TASK_C2_EXPLODE: u8 = 4;
// arg: the initial hit
const TASK_BURST_HIT: u8 = 5;
const TASK_SPIN_START: u8 = 6;
const TASK_SPIN: u8 = 7;

const GADGET_GUOBA: u8 = 0;
/// Guoba's lifetime.
const GUOBA_DURATION: i32 = 438;

impl Character for Char {
    const KEY: &'static str = "xiangling";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        let empty = AttackEvent::new(AttackInfo::new(0, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, 0.0), 0);
        State { guoba: empty, spin: empty }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
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

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::NC[lvl]);
        ai.abil = "Charge";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        ActInfo::new(Frames::Table(&g::CHARGE_FRAMES), g::CHARGE_FRAMES[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        // the frames from Guoba's expiry until the chili pepper is picked up
        let a4_delay = p.get("a4_delay").map_or(-1, |d| d.min(10 * 60) as i32);
        s.add_task(Task::Char { c: ci as u8, id: TASK_GUOBA, arg: a4_delay }, 13);
        s.set_cd_with_delay(ci, Action::Skill, 12 * 60, 13);
        ActInfo::new(Frames::Table(&g::SKILL_FRAMES), g::SKILL_FRAMES[0], g::SKILL_FRAMES[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        for (i, &h) in g::BURST_HITMARKS.iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_HIT, arg: i as i32 }, h);
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SPIN_START, arg: 0 }, 56);
        s.set_cd_with_delay(ci, Action::Burst, 20 * 60, 18);
        s.consume_energy(ci, 24);
        ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_FRAMES[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = ((arg / 8) as usize, (arg % 8) as usize);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.def_halt = true;
                let cb = if s.chars[ci].cons >= 2 && n == 4 { CB_C2 } else { 0 };
                s.queue_attack(ai, cb, 0, 0);
            }
            TASK_GUOBA => {
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::GUOBA_TICK[lvl]);
                ai.abil = "Guoba";
                let ev = s.snapshot_event(ai, 0);
                s.st_mut::<State>(ci).guoba = ev;
                s.add_status(ci, GUOBA_STATUS, GUOBA_DURATION, false);
                // 1.3 in front of the player, radius 0.2
                s.add_gadget(ci, GADGET_GUOBA, GUOBA_DURATION, 0.0, 1.3, 0.2);
                if arg >= 0 && s.chars[ci].ascension >= 4 {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, GUOBA_DURATION + arg);
                }
            }
            TASK_GUOBA_AURA_END => s.set_gadget_aura(ci, GADGET_GUOBA, AURA_PYRO, 0.0),
            TASK_A4 => {
                // the chili pepper, picked up by the active character
                let active = s.player.active as usize;
                s.add_stat_mod(active, A4, st::ATKP, 0.1, 10 * 60, true);
            }
            TASK_C2_EXPLODE => {
                let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, 0.75);
                ai.abil = "Oil Meets Fire (C2)";
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_BURST_HIT => {
                let i = arg as usize;
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::PYRONADO_INITIAL[i][lvl]);
                ai.abil = ["Pyronado Hit 1", "Pyronado Hit 2", "Pyronado Hit 3"][i];
                ai.hitlag_halt = 0.03 * 60.0;
                ai.hitlag_factor = 0.01;
                ai.def_halt = true;
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_SPIN_START => {
                let max = if s.chars[ci].cons >= 4 { 14 * 60 } else { 10 * 60 };
                s.add_core_status(BURST_STATUS, max);
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::PYRONADO_SPIN[lvl]);
                ai.abil = "Pyronado";
                let ev = s.snapshot_event(ai, 0);
                s.st_mut::<State>(ci).spin = ev;
                for delay in (0..=max).step_by(73) {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_SPIN, arg: 0 }, delay);
                }
                if s.chars[ci].cons >= 6 {
                    s.add_core_status(C6_STATUS, max);
                    for i in 0..s.chars.len() {
                        s.add_stat_mod(i, C6, st::PYROP, 0.15, max, true);
                    }
                }
            }
            TASK_SPIN => {
                let ev = s.st::<State>(ci).spin;
                s.queue_attack_with_snap(ev, 0, 0);
            }
            _ => unreachable!(),
        }
    }

    /// Guoba breathes every 100 ticks from its 103rd, with a pyro window
    /// of 31 frames on itself.
    fn gadget_tick(s: &mut Sim, ci: usize, _id: u8, timer: i32) {
        if !matches!(timer, 103 | 203 | 303 | 403) {
            return;
        }
        s.set_gadget_aura(ci, GADGET_GUOBA, AURA_PYRO, g::INFUSE_DURABILITY as f64);
        s.add_task(Task::Char { c: ci as u8, id: TASK_GUOBA_AURA_END, arg: 0 }, g::INFUSE_WINDOW + 1);
        let ev = s.st::<State>(ci).guoba;
        s.queue_attack_with_snap(ev, CB_C1 | CB_PARTICLE, 10);
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_C2 != 0 && !s.enemy_status_active(C2_DEBUFF) {
            s.queue_enemy_task(Task::Char { c: ci as u8, id: TASK_C2_EXPLODE, arg: 0 }, 120);
            s.enemy_add_status(C2_DEBUFF, 120, true);
        }
        if cbs & CB_C1 != 0 && s.chars[ci].cons >= 1 {
            s.enemy_add_resist(C1, Element::Pyro, -0.15, 6 * 60);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 60, false);
            s.queue_particle(1.0, Element::Pyro, s.particle_delay[ci]);
        }
    }
}
