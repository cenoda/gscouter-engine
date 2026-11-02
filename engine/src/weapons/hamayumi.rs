//! Hamayumi, from gcsim's internal/weapons/bow/hamayumi.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("hamayumi");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "hamayumi";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// Normal attack DMG +16 to 32% and charged attack DMG +12 to 24%,
    /// doubled at full energy.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        let r = s.chars[ci].refine as f64;
        let v = match ev.info.tag {
            AttackTag::Normal => 0.12 + 0.04 * r,
            AttackTag::Extra => 0.09 + 0.03 * r,
            _ => return,
        };
        let c = &s.chars[ci];
        ev.stats[st::DMGP] += if c.energy == c.energy_max { v * 2.0 } else { v };
    }
}
