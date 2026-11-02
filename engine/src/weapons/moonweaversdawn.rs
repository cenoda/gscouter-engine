//! Moonweaver's Dawn, from gcsim's internal/weapons/sword/moonweaversdawn.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("moonweavers-dawn");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "moonweaversdawn";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// Burst DMG +20 to 40%, and +28 to 56% more with an energy cost of 40
    /// or less, +16 to 32% with 60 or less.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag != AttackTag::ElementalBurst {
            return;
        }
        let r = s.chars[ci].refine as f64;
        let cost = s.chars[ci].energy_max;
        let mut v = 0.15 + 0.05 * r;
        if cost <= 40.0 {
            v += 0.21 + 0.07 * r;
        } else if cost <= 60.0 {
            v += 0.12 + 0.04 * r;
        }
        ev.stats[st::DMGP] += v;
    }
}
