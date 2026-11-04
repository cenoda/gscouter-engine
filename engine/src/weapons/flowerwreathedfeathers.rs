//! Flower-Wreathed Feathers, from gcsim's internal/weapons/bow/flowerwreathedfeathers.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    stacks: i32,
    leave_src: i32,
}

const ICD: ModKey = ModKey::new("flowerwreathedfeathers-icd");
const BUFF: ModKey = ModKey::new("flowerwreathedfeathers");

const TASK_CLEAR: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "flowerwreathedfeathers";
    type State = State;
    fn new_state() -> State {
        State {
            stacks: 0,
            leave_src: -1,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::ActionExec, o);
        s.subscribe(EventKind::StateChange, o);
        s.subscribe(EventKind::Swap, o);
        s.chars[ci].mods.push(Mod::owned(BUFF, ModKind::Attack, o, st::NONE, false, false, -1));
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == BUFF && ev.info.tag == AttackTag::Extra {
            let r = s.chars[ci].refine as f64;
            let st = s.weapon_st::<State>(ci);
            ev.stats[st::DMGP] += (0.045 + 0.015 * r) * st.stacks as f64;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // gcsim's OnAimShoot comes before the aimed shot runs and
            // OnActionExec after it, on the same frame; the stacks count
            // in attack mods, which read them when the shot lands
            Event::ActionExec(Action::Aim) => {
                if s.player.active as usize != ci || s.mod_active(ci, ICD) {
                    return;
                }
                s.add_status(ci, ICD, 30, true);
                let st = s.weapon_st_mut::<State>(ci);
                st.leave_src = -1;
                if st.stacks < 6 {
                    st.stacks += 1;
                }
            }
            Event::StateChange(next) => {
                // gcsim's OnStateChange carries the previous state, which
                // is still the player's state while the event runs
                if s.player.active as usize != ci || s.player.state != AnimState::Aim || next == AnimState::Aim {
                    return;
                }
                if s.weapon_st::<State>(ci).leave_src != -1 {
                    return;
                }
                let f = s.f;
                s.weapon_st_mut::<State>(ci).leave_src = f;
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_CLEAR, arg: f }, 10 * 60);
            }
            Event::Swap { prev, .. } => {
                if prev != ci {
                    return;
                }
                if s.weapon_st::<State>(ci).leave_src != -1 {
                    return;
                }
                let f = s.f;
                s.weapon_st_mut::<State>(ci).leave_src = f;
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_CLEAR, arg: f }, 10 * 60);
            }
            _ => {}
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, src: i32) {
        if id == TASK_CLEAR {
            if s.weapon_st::<State>(ci).leave_src != src {
                return;
            }
            if s.player.active as usize == ci && s.current_state() == AnimState::Aim {
                return;
            }
            s.weapon_st_mut::<State>(ci).stacks = 0;
        }
    }
}
