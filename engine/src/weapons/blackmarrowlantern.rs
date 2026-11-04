//! Blackmarrow Lantern, from gcsim's internal/weapons/catalyst/blackmarrowlantern.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("blackmarrow-lantern");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "blackmarrowlantern";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::ReactBonus, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// Bloom DMG +48 to 96%; Lunar-Bloom (direct) DMG +12 to 24%, twice
    /// that at Moonsign level 2 (Ascendant Gleam).
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        let buff = 0.09 + 0.03 * s.chars[ci].refine as f64;
        match info.tag {
            AttackTag::Bloom => buff * 4.0,
            AttackTag::DirectLunarBloom => {
                if s.chars.iter().map(|c| c.moonsign).sum::<i32>() >= 2 {
                    buff * 2.0
                } else {
                    buff
                }
            }
            _ => 0.0,
        }
    }
}
