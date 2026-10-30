//! Harbinger of Dawn, from gcsim's internal/weapons/sword/harbinger.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("harbinger");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "harbingerofdawn";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::CR, false, false, -1));
    }

    fn mod_amount(s: &Sim, ci: usize, _on: usize, _m: &Mod, out: &mut Stats) -> bool {
        let c = &s.chars[ci];
        if c.hp_ratio >= 0.9 {
            out[st::CR] += 0.105 + 0.035 * c.refine as f64;
        }
        true
    }
}
