//! blackcliffslasher, from gcsim's internal/weapons/common/blackcliff.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State(pub super::blackcliff::State);

impl WeaponImpl for Weapon {
    const KEY: &'static str = "blackcliffslasher";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        super::blackcliff::init(s, ci);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::TargetDied(ev) = *e else { return };
        let mut inner = std::mem::take(&mut s.weapon_st_mut::<State>(ci).0);
        super::blackcliff::on_target_died(s, ci, ev, &mut inner);
        s.weapon_st_mut::<State>(ci).0 = inner;
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        super::blackcliff::mod_amount(s, ci, out)
    }
}
