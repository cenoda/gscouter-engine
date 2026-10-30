//! Festering Desire, from gcsim's internal/weapons/sword/festering.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("festering");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "festeringdesire";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// Skill DMG and CRIT Rate.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            let r = s.chars[ci].refine as f64;
            ev.stats[st::CR] += 0.045 + 0.015 * r;
            ev.stats[st::DMGP] += 0.12 + 0.04 * r;
        }
    }
}
