//! Deathmatch, from gcsim's internal/weapons/spear/deathmatch. Its bonus
//! for two or more enemies nearby never applies against the one enemy, so
//! the single-enemy ATK bonus is permanent; its enemy checks change
//! nothing and are not run.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("deathmatch");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "deathmatch";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(KEY, st::ATKP, 0.18 + 0.06 * r, false, false, -1));
    }
}
