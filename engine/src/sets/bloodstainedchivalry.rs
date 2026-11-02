//! Bloodstained Chivalry, from gcsim's internal/artifacts/bloodstained.
//!
//! The 4-piece follows a kill (gcsim's OnTargetDied), which ends the
//! engine's fight against its one enemy: it never acts. Port it with
//! fights that go on after a kill.

use crate::content::SetImpl;
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("bloodstained-2pc");

impl SetImpl for Set {
    const KEY: &'static str = "bloodstainedchivalry";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, _: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::PHYP, 0.25, false, false, -1));
        }
    }
}
