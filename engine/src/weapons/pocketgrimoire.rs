//! pocketgrimoire, no effect in gcsim.

use crate::content::WeaponImpl;
use crate::sim::*;

pub struct Weapon;
pub struct State;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "pocketgrimoire";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(_: &mut Sim, _: usize) {}
}
