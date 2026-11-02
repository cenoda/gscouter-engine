//! The Exile, from gcsim's internal/artifacts/exile.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("exile-2pc");
const FOUR: ModKey = ModKey::new("exile-4pc");

const TASK_ENERGY: u8 = 0;

impl SetImpl for Set {
    const KEY: &'static str = "theexile";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ER, 0.20, false, false, -1));
        }
        if count >= 4 {
            s.subscribe(EventKind::Burst, Owner::Set(ci as u8, si as u8));
        }
    }

    /// 2 energy to the character in arg, on its own queue.
    fn task(s: &mut Sim, _: usize, _: usize, _: u8, arg: i32) {
        s.add_energy(arg as usize, 2.0);
    }

    /// A burst on field, unless someone has the buff: the others get 2
    /// energy every 2 s for 6 s.
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::Burst = *e else { return };
        if s.player.active as usize != ci || (0..s.chars.len()).any(|c| s.mod_active(c, FOUR)) {
            return;
        }
        for c in 0..s.chars.len() {
            if c == ci {
                continue;
            }
            s.add_status(c, FOUR, 360, true);
            for i in [120, 240, 360] {
                s.queue_char_task(c, Task::Set { c: ci as u8, si: si as u8, id: TASK_ENERGY, arg: c as i32 }, i);
            }
        }
    }
}
