//! Vermillion Hereafter, from gcsim's internal/artifacts/vermillion.
//!
//! gcsim subscribes its drain and swap hooks under keys shared by every
//! wearer, so a second wearer's would replace the first's: two wearers
//! are an error.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const TWO: ModKey = ModKey::new("verm-2pc");
const FOUR: ModKey = ModKey::new("verm-4pc");
const ICD: ModKey = ModKey::new("verm-4pc-icd");

impl SetImpl for Set {
    const KEY: &'static str = "vermillionhereafter";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        }
        if count >= 4 {
            let o = Owner::Set(ci as u8, si as u8);
            s.subscribe(EventKind::Burst, o);
            s.subscribe(EventKind::Drain, o);
            s.subscribe(EventKind::Swap, o);
        }
    }

    fn post_init(s: &mut Sim, ci: usize, si: usize) {
        let kind = s.chars[ci].sets[si].kind;
        let wearers = s.chars.iter().filter(|c| c.sets.iter().any(|x| x.kind == kind && x.count >= 4)).count();
        if wearers > 1 {
            s.unsupported("set vermillionhereafter 4-piece on two characters");
        }
    }

    /// ATK +8%, and +10% per stack.
    fn mod_amount(s: &Sim, ci: usize, si: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::ATKP] += 0.08 + 0.1 * s.set_st::<State>(ci, si).stacks as f64;
        true
    }

    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        match *e {
            // a burst on field: Nascent Light for 16 s (hitlag)
            Event::Burst => {
                if s.player.active as usize != ci {
                    return;
                }
                s.set_st_mut::<State>(ci, si).stacks = 0;
                s.add_mod(ci, Mod::owned(FOUR, ModKind::Stat, Owner::Set(ci as u8, si as u8), st::ATKP, false, true, -1), 16 * 60);
            }
            // any character losing HP while it lasts, once every 0.8 s
            // (gcsim checks no character): a stack, up to 4
            Event::Drain { amount, .. } => {
                if amount <= 0.0 || !s.mod_active(ci, FOUR) || s.mod_active(ci, ICD) || s.set_st::<State>(ci, si).stacks == 4 {
                    return;
                }
                s.set_st_mut::<State>(ci, si).stacks += 1;
                s.add_status(ci, ICD, 48, true);
            }
            // any swap ends it
            Event::Swap { .. } => {
                s.delete_mod(ci, FOUR);
                s.set_st_mut::<State>(ci, si).stacks = 0;
            }
            _ => {}
        }
    }
}
