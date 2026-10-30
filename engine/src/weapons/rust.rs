//! Rust, from gcsim's internal/weapons/bow/rust.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("rust");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "rust";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// Normal attack DMG up, charged attack DMG down by 10%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        match ev.info.tag {
            AttackTag::Normal => ev.stats[st::DMGP] += 0.3 + s.chars[ci].refine as f64 * 0.1,
            AttackTag::Extra => ev.stats[st::DMGP] -= 0.1,
            _ => {}
        }
    }
}
