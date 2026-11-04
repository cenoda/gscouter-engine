//! Lost Prayer to the Sacred Winds, from gcsim's
//! internal/weapons/catalyst/prayer: every 4 s (on the holder's queue,
//! from the start) on field, a stack of elemental DMG +8 to 16%, up to
//! four; swapping out loses them all. The `stacks` param sets the stacks
//! at the start (at most 4).

use crate::content::{Event, Owner, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    stacks: i32,
}

const KEY: ModKey = ModKey::new("lost-prayer");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "lostprayertothesacredwinds";
    type State = State;
    fn new_state() -> State {
        State { stacks: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let stacks = s.chars[ci].weapon_param("stacks").min(4) as i32;
        s.weapon_st_mut::<State>(ci).stacks = stacks;
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: 0, arg: 0 }, 240);
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Swap, o);
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Stat, o, st::NONE, false, false, -1));
    }

    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        if s.player.active as usize == ci {
            let st = s.weapon_st_mut::<State>(ci);
            if st.stacks < 4 {
                st.stacks += 1;
            }
        }
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: 0, arg: 0 }, 240);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Swap { .. } = *e else { return };
        if s.player.active as usize != ci {
            s.weapon_st_mut::<State>(ci).stacks = 0;
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let stacks = s.weapon_st::<State>(ci).stacks;
        if stacks == 0 {
            return false;
        }
        let p = (0.06 + s.chars[ci].refine as f64 * 0.02) * stacks as f64;
        for stat in [st::PYROP, st::HYDROP, st::CRYOP, st::ELECTROP, st::ANEMOP, st::GEOP, st::DENDROP] {
            out[stat] += p;
        }
        true
    }
}
