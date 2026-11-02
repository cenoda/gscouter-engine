//! Moonpiercer, from gcsim's internal/weapons/spear/moonpiercer: a dendro
//! reaction by the holder on the enemy (burning, quicken, aggravate,
//! spread, bloom, Lunar-Bloom; hyperbloom and burgeon from any target)
//! drops a Leaf of Revival, once every 20 s, which the active character
//! picks up `pickup_delay` frames later for ATK (none without the param).

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("moonpiercer-icd");
const BUFF: ModKey = ModKey::new("stillwood-moonshadow");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "moonpiercer";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        match r {
            Reaction::Hyperbloom | Reaction::Burgeon => {}
            Reaction::Burning | Reaction::Quicken | Reaction::Aggravate | Reaction::Spread | Reaction::Bloom | Reaction::LunarBloom => {
                if !ev.on_enemy() {
                    return;
                }
            }
            _ => return,
        }
        if ev.info.actor as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 1200, true);
        let delay = s.chars[ci].weapon_param("pickup_delay");
        if delay > 0 {
            s.add_task(Task::Weapon { c: ci as u8, id: 0, arg: 0 }, delay as i32);
        }
    }

    /// The leaf picked up: ATK +16 to 32% for 12 s (hitlag) on the active
    /// character.
    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        let active = s.player.active as usize;
        let atk = 0.12 + 0.04 * s.chars[ci].refine as f64;
        s.add_mod(active, Mod::stat(BUFF, st::ATKP, atk, false, true, -1), 720);
    }
}
