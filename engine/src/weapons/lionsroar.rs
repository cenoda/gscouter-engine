//! Lion's Roar, from gcsim's internal/weapons/sword/lion.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("lionsroar");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "lionsroar";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// DMG against an enemy with electro or pyro.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if s.enemy_aura_contains(Element::Electro) || s.enemy_aura_contains(Element::Pyro) {
            ev.stats[st::DMGP] += 0.16 + s.chars[ci].refine as f64 * 0.04;
        }
    }
}
