//! Talking Stick, from gcsim's internal/weapons/claymore/talkingstick:
//! gcsim gives it no effect (common.NewNoEffect; its passive, which reacts
//! to the holder being affected by elements, is a TODO there), so neither
//! does the engine.

use crate::content::WeaponImpl;
use crate::sim::*;

pub struct Weapon;
pub struct State;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "talkingstick";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(_: &mut Sim, _: usize) {}
}
