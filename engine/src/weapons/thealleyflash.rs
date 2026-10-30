//! The Alley Flash, from gcsim's internal/weapons/sword/alley. It is
//! locked out by HP drained from outside, which needs `hurt`; the engine
//! does not run configs with it, so the bonus is permanent.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("alleyflash");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "thealleyflash";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(KEY, st::DMGP, 0.09 + 0.03 * r, false, false, -1));
    }
}
