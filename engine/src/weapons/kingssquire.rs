//! King's Squire, from gcsim's internal/weapons/bow/kingssquire.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("kingssquire");
const ICD: ModKey = ModKey::new("kingssquire-icd");

const TASK_ATTACK: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "kingssquire";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::Burst, o);
        s.subscribe(EventKind::Swap, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Skill | Event::Burst => {
                if s.player.active as usize != ci || s.mod_active(ci, ICD) {
                    return;
                }
                s.add_status(ci, ICD, 20 * 60, true);
                let r = s.chars[ci].refine as f64;
                s.add_mod(ci, Mod::stat(BUFF, st::EM, 40.0 + 20.0 * r, false, true, -1), 12 * 60);
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_ATTACK, arg: 0 }, 12 * 60);
            }
            Event::Swap { prev, .. } => {
                if prev == ci {
                    Self::trigger_attack(s, ci);
                }
            }
            _ => {}
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        if id == TASK_ATTACK {
            Self::trigger_attack(s, ci);
        }
    }
}

impl Weapon {
    fn trigger_attack(s: &mut Sim, ci: usize) {
        if !s.mod_active(ci, BUFF) {
            return;
        }
        s.delete_mod(ci, BUFF);
        let r = s.chars[ci].refine as f64;
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 0.0, 0.8 + 0.2 * r);
        ai.abil = "King's Squire Proc";
        ai.pattern = s.circle_on_target(1.6);
        s.queue_attack(ai, 0, 0, 1);
    }
}
