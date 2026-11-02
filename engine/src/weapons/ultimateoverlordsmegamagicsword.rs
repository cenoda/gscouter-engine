//! Ultimate Overlord's Mega Magic Sword, from gcsim's
//! internal/weapons/claymore/ultimateoverlordsmegamagicsword.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("ultimateoverlordsmegamagicsword");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "ultimateoverlordsmegamagicsword";
    type State = State;
    fn new_state() -> State {
        State
    }

    /// ATK +12 to 24%, up to twice that by the Melusines helped (the
    /// `melusines` param, 6 when absent, at most 6).
    fn init(s: &mut Sim, ci: usize) {
        let c = &s.chars[ci];
        let melusines = c.weapon_params.iter().find(|p| p.0 == "melusines").map_or(6, |p| p.1.min(6));
        let additional = (melusines as f64 / 6.0).max(0.0);
        let v = (0.09 + c.refine as f64 * 0.03) * (1.0 + additional);
        s.chars[ci].mods.push(Mod::stat(KEY, st::ATKP, v, false, false, -1));
    }
}
