//! Prototype Amber, from gcsim's internal/weapons/catalyst/prototype: a
//! burst by the holder on field gives the holder 4 to 6 energy (by
//! refinement) and heals every party member 4 to 6% of their max HP, every
//! 2 s for 6 s. Each heal waits on the queue of the character healed, as
//! gcsim's QueueCharTask does, so their hitlag delays it.

use crate::content::{Event, Owner, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const TASK_ENERGY: u8 = 0;
/// arg: the character healed.
const TASK_HEAL: u8 = 1;

fn amount(s: &Sim, ci: usize) -> f64 {
    3.5 + s.chars[ci].refine as f64 * 0.5
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "prototypeamber";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Burst, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Burst = *e else { return };
        if s.player.active as usize != ci {
            return;
        }
        let c = ci as u8;
        for i in [120, 240, 360] {
            s.queue_char_task(ci, Task::Weapon { c, id: TASK_ENERGY, arg: 0 }, i);
        }
        for x in 0..s.chars.len() {
            for i in [120, 240, 360] {
                s.queue_char_task(x, Task::Weapon { c, id: TASK_HEAL, arg: x as i32 }, i);
            }
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        let e = amount(s, ci);
        match id {
            TASK_ENERGY => s.add_energy(ci, e),
            TASK_HEAL => {
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, Some(arg as usize), true, e / 100.0, bonus);
            }
            _ => unreachable!(),
        }
    }
}
