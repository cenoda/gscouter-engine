//! Lavawalker, from gcsim's internal/artifacts/lavawalker. Its 2-piece (pyro
//! RES) changes nothing in gcsim.

use crate::content::SetImpl;
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const FOUR: ModKey = ModKey::new("lavawalker-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "lavawalker";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 4 {
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, false, -1));
        }
    }

    /// DMG +35% against an enemy with pyro.
    fn attack_mod(s: &Sim, _: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if s.enemy.aura.contains(Element::Pyro) {
            ev.stats[st::DMGP] += 0.35;
        }
    }
}
