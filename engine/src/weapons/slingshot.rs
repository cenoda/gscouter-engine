//! Slingshot, from gcsim's internal/weapons/bow/slingshot. Its exceptions
//! for Tartaglia's melee and Chasca's Nightsoul shots are for characters the
//! engine does not implement.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("slingshot");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "slingshot";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// Normal and charged attacks that hit within 0.3 s of their snapshot
    /// gain DMG; later ones lose 10%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        let travel = s.f - ev.snap_f;
        ev.stats[st::DMGP] += if travel > 18 { -0.10 } else { 0.3 + s.chars[ci].refine as f64 * 0.06 };
    }
}
