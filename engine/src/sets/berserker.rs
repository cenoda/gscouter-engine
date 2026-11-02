//! Berserker, from gcsim's internal/artifacts/berserker.

use crate::content::SetImpl;
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("berserker-2pc");
const FOUR: ModKey = ModKey::new("berserker-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "berserker";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::CR, 0.12, false, false, -1));
        }
        if count >= 4 {
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, false, -1));
        }
    }

    /// CRIT Rate +24% at 70% HP or less.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if s.current_hp(ci) / s.max_hp(ci) <= 0.7 {
            ev.stats[st::CR] += 0.24;
        }
    }
}
