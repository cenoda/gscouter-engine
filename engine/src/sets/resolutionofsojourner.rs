//! Resolution of Sojourner, from gcsim's internal/artifacts/sojourner.

use crate::content::SetImpl;
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("sojourner-2pc");
const FOUR: ModKey = ModKey::new("sojourner-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "resolutionofsojourner";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        }
        if count >= 4 {
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, false, -1));
        }
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == FOUR && ev.info.tag == AttackTag::Extra {
            ev.stats[st::CR] += 0.30;
        }
    }
}
