//! Jadefall's Splendor, from gcsim's internal/weapons/catalyst/jadefallssplendor.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;

pub struct Weapon;
pub struct State {
    src: i32,
}

const BUFF: ModKey = ModKey::new("jadefall-buff");
const TASK_ENERGY: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "jadefallssplendor";
    type State = State;
    fn new_state() -> State {
        State { src: -1 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Burst, o);
        s.subscribe(EventKind::Shielded, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let trigger = match *e {
            Event::Burst => s.player.active as usize == ci,
            Event::Shielded(shd) => shd.owner as usize == ci,
            _ => false,
        };
        if !trigger {
            return;
        }
        let f = s.f;
        s.weapon_st_mut::<State>(ci).src = f;
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_ENERGY, arg: f }, 142);
        // the DMG bonus is fixed from the Max HP when the buff is added
        let r = s.chars[ci].refine as f64;
        let dmg_mul = 0.001 + r * 0.002;
        let dmg_cap = 0.04 + r * 0.08;
        let mut final_dmg = s.max_hp(ci) * 0.001 * dmg_mul;
        if final_dmg > dmg_cap {
            final_dmg = dmg_cap;
        }
        let stat = s.chars[ci].element.dmg_stat();
        s.add_mod(ci, Mod::stat(BUFF, stat, final_dmg, false, true, -1), 180);
    }

    fn task(s: &mut Sim, ci: usize, id: u8, src: i32) {
        if id == TASK_ENERGY && s.weapon_st::<State>(ci).src == src {
            let r = s.chars[ci].refine as f64;
            s.add_energy(ci, 4.0 + r * 0.5);
        }
    }
}
