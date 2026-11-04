//! Defender's Will, from gcsim's internal/artifacts/defenderswill.
//! 4pc does nothing in gcsim.

use crate::content::SetImpl;
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("defenderswill-2pc");

impl SetImpl for Set {
    const KEY: &'static str = "defenderswill";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, _: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::DEFP, 0.30, false, false, -1));
        }
    }
}
