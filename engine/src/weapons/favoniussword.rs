//! Favonius Sword, gcsim's common Favonius (`favonius.rs`).

use crate::content::{Event, WeaponImpl};
use crate::sim::*;

pub struct Weapon;
pub struct State;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "favoniussword";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        super::favonius::init(s, ci);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        super::favonius::event(s, ci, e);
    }
}
