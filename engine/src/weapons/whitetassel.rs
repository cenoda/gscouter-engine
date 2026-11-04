//! White Tassel, from gcsim's internal/weapons/spear/whitetassel.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("whitetassel");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "whitetassel";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// Normal attack DMG +24 to 48%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::Normal {
            ev.stats[st::DMGP] += 0.18 + 0.06 * s.chars[ci].refine as f64;
        }
    }
}
