//! Sacrificial Fragments, from gcsim's internal/weapons/catalyst/sacrificial:
//! gcsim's common Sacrificial, as Sacrificial Sword.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::weapons::sacrificialsword;

pub struct Weapon;
pub struct State;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "sacrificialfragments";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        <sacrificialsword::Weapon as WeaponImpl>::init(s, ci);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        <sacrificialsword::Weapon as WeaponImpl>::event(s, ci, e);
    }
}
