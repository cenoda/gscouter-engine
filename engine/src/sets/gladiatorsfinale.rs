//! Gladiator's Finale, from gcsim's internal/artifacts/gladiator.

use crate::content::SetImpl;
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("glad-2pc");
const FOUR: ModKey = ModKey::new("glad-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "gladiatorsfinale";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, _: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        }
    }

    /// gcsim's Set.Init: normal attack DMG +35% for a sword, claymore or
    /// polearm.
    fn post_init(s: &mut Sim, ci: usize, si: usize) {
        if s.chars[ci].sets[si].count >= 4 && matches!(s.chars[ci].class, WeaponClass::Spear | WeaponClass::Sword | WeaponClass::Claymore) {
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, false, -1));
        }
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::Normal {
            ev.stats[st::DMGP] += 0.35;
        }
    }
}
