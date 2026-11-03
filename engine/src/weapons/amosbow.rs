//! Amos' Bow, from gcsim's internal/weapons/bow/amos.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("amos");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "amosbow";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// Normal and charged attack DMG +12 to 24%, and 8 to 16% more for
    /// every 0.1 s since the attack's snapshot, up to five times.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        let r = s.chars[ci].refine as f64;
        let travel = (s.f - ev.snap_f) as f64 / 60.0;
        let stacks = ((travel / 0.1) as i32).min(5);
        ev.stats[st::DMGP] += 0.09 + 0.03 * r + (0.06 + 0.02 * r) * stacks as f64;
    }
}
