//! Sapwood Blade, from gcsim's internal/weapons/sword/sapwoodblade. It
//! procs on dendro reactions (burning, quicken, aggravate, spread, bloom,
//! hyperbloom, burgeon), none of which the engine causes yet, so it does
//! nothing.

use crate::content::WeaponImpl;
use crate::sim::*;

pub struct Weapon;
pub struct State;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "sapwoodblade";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(_: &mut Sim, _: usize) {}
}
