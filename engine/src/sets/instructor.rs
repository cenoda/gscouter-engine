//! Instructor, from gcsim's internal/artifacts/instructor.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("instructor-2pc");
const FOUR: ModKey = ModKey::new("instructor-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "instructor";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::EM, 80.0, false, false, -1));
        if count >= 4 {
            s.subscribe(EventKind::Reaction, Owner::Set(ci as u8, si as u8));
        }
    }

    /// A reaction by the wearer, on field: 120 EM to the team for 8 s.
    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let Event::Reaction(_, ev) = *e else { return };
        if s.player.active as usize != ci || ev.info.actor as usize != ci {
            return;
        }
        for i in 0..s.chars.len() {
            s.add_stat_mod(i, FOUR, st::EM, 120.0, 480, true);
        }
    }
}
