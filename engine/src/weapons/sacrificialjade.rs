//! Sacrificial Jade, from gcsim's internal/weapons/catalyst/sacrificialjade.
//!
//! Off the field for 5 s, Max HP and Elemental Mastery rise by refinement
//! (32% and 40 at R1, up to 64% and 80 at R5). After a swap onto the
//! wielder the buff ends once they have stayed on field for 10 s. Both
//! waits are core tasks that remember the swap's frame: a later swap
//! changes `last_swap`, and the task does nothing when the frame it
//! captured no longer matches. The `stacks` param is the only one; `stacks=1`
//! starts with the buff already on. The package exports no tables; the
//! amounts are the ones `NewWeapon` computes.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    /// The frame of the swap the pending task belongs to, or -1 before any.
    last_swap: i32,
    buff: bool,
}

const KEY: ModKey = ModKey::new("sacrificial-jade");

const TASK_GAIN: u8 = 0;
const TASK_CLEAR: u8 = 1;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "sacrificialjade";
    type State = State;
    fn new_state() -> State {
        State {
            last_swap: -1,
            buff: false,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        // only exactly 1, as gcsim's p.Params["stacks"] == 1
        if s.chars[ci].weapon_param("stacks") == 1 {
            s.weapon_st_mut::<State>(ci).buff = true;
        }
        s.chars[ci].mods.push(Mod::owned(
            KEY,
            ModKind::Stat,
            Owner::Weapon(ci as u8),
            st::NONE,
            false,
            false,
            -1,
        ));
        s.subscribe(EventKind::Swap, Owner::Weapon(ci as u8));
    }

    /// Off the field from the start, the buff is checked after 5 s.
    fn post_init(s: &mut Sim, ci: usize) {
        if s.player.active as usize != ci {
            let f = s.f;
            s.weapon_st_mut::<State>(ci).last_swap = f;
            s.add_task(
                Task::Weapon {
                    c: ci as u8,
                    id: TASK_GAIN,
                    arg: f,
                },
                5 * 60,
            );
        }
    }

    /// HP% +32% to +64% and EM +40 to +80 while the buff is on.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if !s.weapon_st::<State>(ci).buff {
            return false;
        }
        let r = s.chars[ci].refine as f64;
        out[st::HPP] += 0.24 + 0.08 * r;
        out[st::EM] += 30.0 + 10.0 * r;
        true
    }

    /// Swapping off starts the 5 s wait; swapping on starts the 10 s wait.
    /// Each replaces `last_swap`, so a task from an earlier swap does not
    /// run. Swaps are applied before the core queue, so a swap on the
    /// frame a task is due cancels that task.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Swap { prev, next } = *e else {
            return;
        };
        let f = s.f;
        if prev == ci {
            s.weapon_st_mut::<State>(ci).last_swap = f;
            s.add_task(
                Task::Weapon {
                    c: ci as u8,
                    id: TASK_GAIN,
                    arg: f,
                },
                5 * 60,
            );
        } else if next == ci {
            s.weapon_st_mut::<State>(ci).last_swap = f;
            s.add_task(
                Task::Weapon {
                    c: ci as u8,
                    id: TASK_CLEAR,
                    arg: f,
                },
                10 * 60,
            );
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, src: i32) {
        if s.weapon_st::<State>(ci).last_swap != src {
            return;
        }
        let on_field = s.player.active as usize == ci;
        match id {
            TASK_GAIN if !on_field => s.weapon_st_mut::<State>(ci).buff = true,
            TASK_CLEAR if on_field => s.weapon_st_mut::<State>(ci).buff = false,
            _ => {}
        }
    }
}
