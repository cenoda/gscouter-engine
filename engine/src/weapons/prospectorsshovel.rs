//! Prospector's Shovel, from gcsim's
//! internal/weapons/spear/prospectorsshovel.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("prospectors-shovel");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "prospectorsshovel";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::ReactBonus, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// Electro-charged DMG +48 to 96%; Lunar-Charged DMG, direct or not,
    /// +12 to 24%, doubled with Ascendant Gleam (gcsim's
    /// GetMoonsignLevel() >= 2, read at the hit).
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        let buff = 0.09 + 0.03 * s.chars[ci].refine as f64;
        match info.tag {
            AttackTag::ECDamage => buff * 4.0,
            AttackTag::ReactionLunarCharge | AttackTag::DirectLunarCharged => {
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
