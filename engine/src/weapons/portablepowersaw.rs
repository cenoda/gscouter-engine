//! Portable Power Saw, from gcsim's internal/weapons/claymore/powersaw.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    stacks: i32,
}

const SYMBOL: ModKey = ModKey::new("portable-power-saw-symbol");
const ICD: ModKey = ModKey::new("portable-power-saw-icd");
const BUFF: ModKey = ModKey::new("portable-power-saw-em");

const TASK_REFUND: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "portablepowersaw";
    type State = State;
    fn new_state() -> State {
        State { stacks: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Heal, o);
        s.subscribe(EventKind::Burst, o);
        s.subscribe(EventKind::Skill, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Heal { caller, on, amount, .. } => {
                if caller != ci && on != ci {
                    return;
                }
                if amount <= 0.0 {
                    return;
                }
                if !s.mod_active(ci, SYMBOL) {
                    s.weapon_st_mut::<State>(ci).stacks = 0;
                }
                let st = s.weapon_st_mut::<State>(ci);
                if st.stacks < 3 {
                    st.stacks += 1;
                }
                s.add_status(ci, SYMBOL, 30 * 60, true);
            }
            Event::Skill | Event::Burst => {
                if !s.mod_active(ci, SYMBOL) || s.mod_active(ci, ICD) || s.player.active as usize != ci {
                    return;
                }
                s.add_status(ci, ICD, 15 * 60, true);
                let count = s.weapon_st::<State>(ci).stacks;
                s.delete_mod(ci, SYMBOL);
                s.weapon_st_mut::<State>(ci).stacks = 0;
                let r = s.chars[ci].refine as f64;
                let em = (30.0 + 10.0 * r) * count as f64;
                s.add_mod(ci, Mod::stat(BUFF, st::EM, em, false, true, -1), 10 * 60);
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_REFUND, arg: count }, 2 * 60);
            }
            _ => {}
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        if id == TASK_REFUND {
            let r = s.chars[ci].refine as f64;
            s.add_energy(ci, (1.5 + 0.5 * r) * arg as f64);
        }
    }
}
