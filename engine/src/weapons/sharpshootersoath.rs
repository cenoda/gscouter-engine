//! Sharpshooter's Oath, from gcsim's internal/weapons/bow/sharpshooter.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("sharpshooter");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "sharpshootersoath";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// DMG against weak points +24 to 48%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.weak_point {
            ev.stats[st::DMGP] += 0.18 + s.chars[ci].refine as f64 * 0.06;
        }
    }
}
