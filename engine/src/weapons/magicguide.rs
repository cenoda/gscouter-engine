//! Magic Guide, from gcsim's internal/weapons/catalyst/magicguide.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("magic-guide");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "magicguide";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// DMG +12 to 24% against an enemy with hydro, electro or cryo (frozen
    /// counting as cryo).
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if s.enemy_aura_contains(Element::Hydro) || s.enemy_aura_contains(Element::Electro) || s.enemy_aura_contains(Element::Cryo) {
            ev.stats[st::DMGP] += 0.09 + s.chars[ci].refine as f64 * 0.03;
        }
    }
}
