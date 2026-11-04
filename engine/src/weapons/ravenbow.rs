//! Raven Bow, from gcsim's internal/weapons/bow/raven.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("ravenbow");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "ravenbow";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if s.enemy_aura_contains(Element::Hydro) || s.enemy_aura_contains(Element::Pyro) {
            ev.stats[st::DMGP] += 0.09 + 0.03 * s.chars[ci].refine as f64;
        }
    }
}
