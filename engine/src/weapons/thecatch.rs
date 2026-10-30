//! "The Catch", from gcsim's internal/weapons/spear/catch.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("the-catch");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "thecatch";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// Burst DMG and CRIT Rate.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::ElementalBurst {
            let r = s.chars[ci].refine as f64;
            ev.stats[st::DMGP] += 0.12 + 0.04 * r;
            ev.stats[st::CR] += 0.045 + 0.015 * r;
        }
    }
}
