//! Heretic's Molten Blade, from gcsim's internal/weapons/sword/hereticsmoltenblade.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    src: i32,
    moved: f64,
    bonus: f64,
}

const BUFF: ModKey = ModKey::new("heretics-molten-blade-gleam");
const CD: ModKey = ModKey::new("heretics-molten-blade-cd");
const TASK_TICK: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "hereticsmoltenblade";
    type State = State;
    fn new_state() -> State {
        State {
            src: -1,
            moved: 0.0,
            bonus: 0.0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::Movement, o);
        s.subscribe(EventKind::Swap, o);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::ATKP] += s.weapon_st::<State>(ci).bonus;
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Skill => {
                if s.player.active as usize != ci || s.mod_active(ci, CD) {
                    return;
                }
                s.add_status(ci, CD, 14 * 60, true);
                let f = s.f;
                let r = s.chars[ci].refine as f64;
                let min_atk = 0.135 + r * 0.045;
                let st = s.weapon_st_mut::<State>(ci);
                st.src = f;
                st.moved = 0.0;
                st.bonus = min_atk;
                s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 14 * 60);
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_TICK, arg: f }, 60);
            }
            Event::Movement(dist) => {
                if s.mod_active(ci, BUFF) {
                    s.weapon_st_mut::<State>(ci).moved += dist;
                }
            }
            Event::Swap { prev, .. } => {
                if prev == ci {
                    s.delete_mod(ci, BUFF);
                    s.weapon_st_mut::<State>(ci).src = -1;
                }
            }
            _ => {}
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, src: i32) {
        if id == TASK_TICK {
            if s.weapon_st::<State>(ci).src != src || !s.mod_active(ci, BUFF) {
                return;
            }
            let r = s.chars[ci].refine as f64;
            let min_atk = 0.135 + r * 0.045;
            let max_atk = 0.27 + r * 0.09;
            let st = s.weapon_st_mut::<State>(ci);
            st.bonus = min_atk + (max_atk - min_atk) * (st.moved / 7.0).min(1.0);
            st.moved = 0.0;
            s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_TICK, arg: src }, 60);
        }
    }
}
