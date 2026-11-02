//! Scarletproof, from gcsim's internal/artifacts/scarletproof.
//!
//! The 4-piece follows Stellar Swirl (gcsim's OnStellarSwirl), which the
//! engine never deals: Stellar Swirl is an error. Port it with Stellar
//! Swirl.

use crate::content::SetImpl;
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("scarletproof-2pc");

impl SetImpl for Set {
    const KEY: &'static str = "scarletproof";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, _: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        }
    }
}
