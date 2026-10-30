//! The Alley Hunter, from gcsim's internal/weapons/bow/alley, with its
//! `stacks` param.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
    active: bool,
    last_active_change: i32,
}

const KEY: ModKey = ModKey::new("alley-hunter");

const TASK_INC: u8 = 0;
const TASK_DEC: u8 = 1;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "alleyhunter";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let stacks = s.chars[ci].weapon_param("stacks").min(10) as i32;
        s.weapon_st_mut::<State>(ci).stacks = stacks;
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
        s.subscribe(EventKind::Swap, Owner::Weapon(ci as u8));
    }

    /// Off the field from the start, it stacks at once.
    fn post_init(s: &mut Sim, ci: usize) {
        let (f, active) = (s.f, s.player.active as usize == ci);
        let st = s.weapon_st_mut::<State>(ci);
        st.active = active;
        st.last_active_change = f;
        if !active {
            s.add_task(Task::Weapon { c: ci as u8, id: TASK_INC, arg: f }, 1);
        }
    }

    /// DMG +2% to +4% per stack.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        let dmg = 0.015 + s.chars[ci].refine as f64 * 0.005;
        ev.stats[st::DMGP] += dmg * s.weapon_st::<State>(ci).stacks as f64;
    }

    /// Off the field a stack a second up to 10; after 4 s on it, two lost a
    /// second.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Swap { prev, next } = *e else { return };
        let f = s.f;
        if next == ci {
            let st = s.weapon_st_mut::<State>(ci);
            st.active = true;
            st.last_active_change = f;
            s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_DEC, arg: f }, 240);
        } else if prev == ci {
            let st = s.weapon_st_mut::<State>(ci);
            st.active = false;
            st.last_active_change = f;
            s.add_task(Task::Weapon { c: ci as u8, id: TASK_INC, arg: f }, 60);
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, src: i32) {
        let st = s.weapon_st_mut::<State>(ci);
        if src != st.last_active_change {
            return;
        }
        match id {
            TASK_INC if !st.active && st.stacks < 10 => {
                st.stacks += 1;
                s.add_task(Task::Weapon { c: ci as u8, id: TASK_INC, arg: src }, 60);
            }
            TASK_DEC if st.active && st.stacks > 0 => {
                st.stacks = (st.stacks - 2).max(0);
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_DEC, arg: src }, 60);
            }
            _ => {}
        }
    }
}
