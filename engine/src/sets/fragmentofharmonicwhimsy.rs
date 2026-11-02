//! Fragment of Harmonic Whimsy, from gcsim's
//! internal/artifacts/fragmentofharmonicwhimsy.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const TWO: ModKey = ModKey::new("fragmentofharmonicwhimsy-2pc");
const FOUR: ModKey = ModKey::new("fragmentofharmonicwhimsy-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "fragmentofharmonicwhimsy";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        if count >= 4 {
            s.subscribe(EventKind::HpDebt, Owner::Set(ci as u8, si as u8));
        }
    }

    fn mod_amount(s: &Sim, ci: usize, si: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::DMGP] += 0.18 * s.set_st::<State>(ci, si).stacks as f64;
        true
    }

    /// The wearer's Bond of Life changing (up or down): DMG +18% for 6 s,
    /// up to 3 stacks, on or off field. gcsim's stacks restart when the
    /// buff has run out.
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::HpDebt { on, change } = *e else { return };
        if on != ci || change == 0.0 {
            return;
        }
        let active = s.mod_active(ci, FOUR);
        let st = s.set_st_mut::<State>(ci, si);
        if !active {
            st.stacks = 0;
        }
        st.stacks = (st.stacks + 1).min(3);
        s.add_mod(ci, Mod::owned(FOUR, ModKind::Stat, Owner::Set(ci as u8, si as u8), st::NONE, false, true, -1), 6 * 60);
    }
}
