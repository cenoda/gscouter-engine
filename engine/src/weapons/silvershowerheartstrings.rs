//! Silvershower Heartstrings, from gcsim's internal/weapons/bow/heartstrings.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    prev_stacks: i32,
}

const SKILL_KEY: ModKey = ModKey::new("heartstrings-skill");
const BOND_KEY: ModKey = ModKey::new("heartstrings-bond");
const HEAL_KEY: ModKey = ModKey::new("heartstrings-healing");
const HP_BUFF: ModKey = ModKey::new("heartstrings");
const BURST_CR: ModKey = ModKey::new("heartstrings-cr");
const BURST_CR_CANCEL: ModKey = ModKey::new("heartstrings-cr-cancel");

const TASK_UPDATE: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "silvershowerheartstrings";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(HP_BUFF, ModKind::Stat, o, st::NONE, false, false, -1));
        s.chars[ci].mods.push(Mod::owned(BURST_CR, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::HpDebt, o);
        s.subscribe(EventKind::Heal, o);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let stacks = Self::stacks(s, ci);
        if stacks == 0 {
            return false;
        }
        let r = s.chars[ci].refine as f64;
        let mut hp = (0.09 + 0.03 * r) * stacks as f64;
        if stacks >= 3 {
            hp += 0.03 + 0.01 * r;
        }
        out[st::HPP] += hp;
        true
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag != AttackTag::ElementalBurst {
            return;
        }
        let stacks = Self::stacks(s, ci);
        if stacks < 3 && !s.mod_active(ci, BURST_CR_CANCEL) {
            return;
        }
        let r = s.chars[ci].refine as f64;
        ev.stats[st::CR] += 0.21 + 0.07 * r;
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Skill => {
                if s.player.active as usize == ci {
                    s.add_status(ci, SKILL_KEY, 25 * 60, true);
                    s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_UPDATE, arg: 0 }, 25 * 60 + 1);
                    Self::on_update_stacks(s, ci);
                }
            }
            Event::HpDebt { on, change } => {
                if on == ci && change < 0.0 {
                    s.add_status(ci, BOND_KEY, 25 * 60, true);
                    s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_UPDATE, arg: 0 }, 25 * 60 + 1);
                    Self::on_update_stacks(s, ci);
                }
            }
            Event::Heal { caller, .. } => {
                if caller == ci {
                    s.add_status(ci, HEAL_KEY, 20 * 60, true);
                    s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_UPDATE, arg: 0 }, 20 * 60 + 1);
                    Self::on_update_stacks(s, ci);
                }
            }
            _ => {}
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        if id == TASK_UPDATE {
            Self::on_update_stacks(s, ci);
        }
    }
}

impl Weapon {
    fn stacks(s: &Sim, ci: usize) -> i32 {
        let mut count = 0;
        if s.mod_active(ci, SKILL_KEY) {
            count += 1;
        }
        if s.mod_active(ci, BOND_KEY) {
            count += 1;
        }
        if s.mod_active(ci, HEAL_KEY) {
            count += 1;
        }
        count
    }

    fn on_update_stacks(s: &mut Sim, ci: usize) {
        let stacks = Self::stacks(s, ci);
        let st = s.weapon_st_mut::<State>(ci);
        if st.prev_stacks == 3 && stacks < 3 {
            s.add_status(ci, BURST_CR_CANCEL, 4 * 60, true);
        }
        s.weapon_st_mut::<State>(ci).prev_stacks = stacks;
    }
}
