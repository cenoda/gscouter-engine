//! Akuoumaru, from gcsim's internal/weapons/claymore/akuoumaru: gcsim's common
//! Wavebreaker (`wavebreaker.rs`).

use crate::content::WeaponImpl;
use crate::sim::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    amt: f64,
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "akuoumaru";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(_s: &mut Sim, _ci: usize) {}

    fn post_init(s: &mut Sim, ci: usize) {
        let amt = super::wavebreaker::post_init(s, ci);
        s.weapon_st_mut::<State>(ci).amt = amt;
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        super::wavebreaker::attack_mod(s.weapon_st::<State>(ci).amt, ev);
    }
}
