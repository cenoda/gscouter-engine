//! Black Tassel, from gcsim's internal/weapons/spear/blacktassel: gcsim
//! gives it no effect (common.NewNoEffect), so neither does the engine.

use crate::content::WeaponImpl;
use crate::sim::*;

pub struct Weapon;
pub struct State;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "blacktassel";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(_: &mut Sim, _: usize) {}
}
