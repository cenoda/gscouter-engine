//! Lithic Blade, from gcsim's internal/weapons/claymore/lithic: gcsim's common Lithic
//! (`lithic.rs`).

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "lithicblade";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        super::lithic::init(s, ci);
    }

    fn post_init(s: &mut Sim, ci: usize) {
        let stacks = super::lithic::post_init(s);
        s.weapon_st_mut::<State>(ci).stacks = stacks;
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        super::lithic::mod_amount(s, ci, s.weapon_st::<State>(ci).stacks, out);
        true
    }
}
