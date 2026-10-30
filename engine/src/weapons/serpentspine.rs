//! Serpent Spine, from gcsim's internal/weapons/claymore/spine.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    stacks: i32,
}

const KEY: ModKey = ModKey::new("spine");
const ICD: ModKey = ModKey::new("spine-dmgtaken-icd");

const TASK_STACK: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "serpentspine";
    type State = State;
    fn new_state() -> State {
        State { stacks: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let stacks = s.chars[ci].weapon_param("stacks").min(5) as i32;
        s.weapon_st_mut::<State>(ci).stacks = stacks;
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_STACK, arg: 0 }, 240);
        s.subscribe(EventKind::Drain, Owner::Weapon(ci as u8));
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// A stack every 4 s on field, up to five.
    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        if s.player.active as usize == ci {
            let st = s.weapon_st_mut::<State>(ci);
            if st.stacks < 5 {
                st.stacks += 1;
            }
        }
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_STACK, arg: 0 }, 240);
    }

    /// DMG from the stacks.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let stacks = s.weapon_st::<State>(ci).stacks;
        if stacks <= 0 {
            return false;
        }
        out[st::DMGP] += stacks as f64 * (0.05 + s.chars[ci].refine as f64 * 0.01);
        true
    }

    /// Damage taken from outside on field loses a stack, once a second.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Drain { amount, external, .. } = *e else { return };
        if !external || amount <= 0.0 || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 60, true);
        let st = s.weapon_st_mut::<State>(ci);
        if st.stacks > 0 {
            st.stacks -= 1;
        }
    }
}
