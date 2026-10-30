//! Dragon's Bane, from gcsim's internal/weapons/spear/dragonbane.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("dragonbane");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "dragonsbane";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// DMG against an enemy with hydro or pyro.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if s.enemy.aura.get(AURA_HYDRO) > ZERO_DUR || s.enemy.aura.get(AURA_PYRO) > ZERO_DUR {
            ev.stats[st::DMGP] += 0.16 + s.chars[ci].refine as f64 * 0.04;
        }
    }
}
