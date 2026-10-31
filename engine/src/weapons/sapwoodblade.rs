//! Sapwood Blade, from gcsim's internal/weapons/sword/sapwoodblade: a
//! dendro reaction by the wielder (burning, quicken, aggravate, spread,
//! bloom, hyperbloom, burgeon) drops a leaf, once every 20 s, which the
//! active character picks up `pickup_delay` frames later for EM (none
//! without the param).

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("sapwoodblade-icd");
const BUFF: ModKey = ModKey::new("forest-sanctuary");

const TASK_PICKUP: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "sapwoodblade";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        if !matches!(r, Reaction::Burning | Reaction::Quicken | Reaction::Aggravate | Reaction::Spread | Reaction::Bloom | Reaction::Hyperbloom | Reaction::Burgeon) {
            return;
        }
        if ev.info.actor as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 1200, true);
        let delay = s.chars[ci].weapon_param("pickup_delay");
        if delay > 0 {
            s.add_task(Task::Weapon { c: ci as u8, id: TASK_PICKUP, arg: 0 }, delay as i32);
        }
    }

    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        let active = s.player.active as usize;
        let em = 45.0 + 15.0 * s.chars[ci].refine as f64;
        s.add_mod(active, Mod::stat(BUFF, st::EM, em, false, true, -1), 720);
    }
}
