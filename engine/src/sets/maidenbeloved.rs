//! Maiden Beloved, from gcsim's internal/artifacts/maiden.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("maiden-2pc");
const FOUR: ModKey = ModKey::new("maiden-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "maidenbeloved";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::HEAL, 0.15, false, false, -1));
        }
        if count >= 4 {
            let o = Owner::Set(ci as u8, si as u8);
            s.subscribe(EventKind::Burst, o);
            s.subscribe(EventKind::Skill, o);
        }
    }

    /// A skill or burst on field: healing received +20% for the party for
    /// 10 s (hitlag).
    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        if !matches!(*e, Event::Skill | Event::Burst) || s.player.active as usize != ci {
            return;
        }
        for c in 0..s.chars.len() {
            let mut m = Mod::new(FOUR, ModKind::HealBonus, st::HEAL, false, true, -1);
            m.value = 0.2;
            s.add_mod(c, m, 600);
        }
    }
}
