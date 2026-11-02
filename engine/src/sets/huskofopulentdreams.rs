//! Husk of Opulent Dreams, from gcsim's internal/artifacts/huskofopulentdreams.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State {
    /// Curiosity, 0 to 4 (from the `stacks` param at the start).
    stacks: i32,
    /// gcsim's lastStackGain: the frame of the last gain, which the stack
    /// loss check queued with it must still match.
    last_gain: i32,
    /// gcsim's lastSwap: the frame the wearer last left the field, which
    /// the off-field gain chain queued with it must still match.
    last_swap: i32,
}

const TWO: ModKey = ModKey::new("husk-2pc");
const FOUR: ModKey = ModKey::new("husk-4pc");
const GAIN_ICD: ModKey = ModKey::new("husk-4pc-stack-gain-icd");

const TASK_OFF_FIELD: u8 = 0;
const TASK_LOSS: u8 = 1;

fn queue_loss(s: &mut Sim, ci: usize, si: usize, src: i32) {
    s.queue_char_task(ci, Task::Set { c: ci as u8, si: si as u8, id: TASK_LOSS, arg: src }, 360);
}

impl SetImpl for Set {
    const KEY: &'static str = "huskofopulentdreams";
    type State = State;
    fn new_state() -> State {
        State { stacks: 0, last_gain: -1, last_swap: -1 }
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        let stacks = s.chars[ci].sets[si].params.iter().find(|p| p.0 == "stacks").map_or(0, |p| p.1).min(4) as i32;
        s.set_st_mut::<State>(ci, si).stacks = stacks;
        s.chars[ci].mods.push(Mod::stat(TWO, st::DEFP, 0.30, false, false, -1));
        if count >= 4 {
            let o = Owner::Set(ci as u8, si as u8);
            s.subscribe(EventKind::Swap, o);
            s.subscribe(EventKind::EnemyDamage, o);
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Stat, o, st::NONE, false, false, -1));
        }
    }

    /// gcsim's Set.Init: off field at the start, the off-field gains begin.
    fn post_init(s: &mut Sim, ci: usize, si: usize) {
        if s.chars[ci].sets[si].count >= 4 && s.player.active as usize != ci {
            let src = s.set_st::<State>(ci, si).last_swap;
            s.add_task(Task::Set { c: ci as u8, si: si as u8, id: TASK_OFF_FIELD, arg: src }, 180);
        }
    }

    /// DEF and Geo DMG +6% per stack.
    fn mod_amount(s: &Sim, ci: usize, si: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let n = s.set_st::<State>(ci, si).stacks as f64;
        out[st::DEFP] += 0.06 * n;
        out[st::GEOP] += 0.06 * n;
        true
    }

    fn task(s: &mut Sim, ci: usize, si: usize, id: u8, src: i32) {
        let f = s.f;
        match id {
            // a stack every 3 s off field, from the swap that took the
            // wearer off
            TASK_OFF_FIELD => {
                if s.player.active as usize == ci || s.set_st::<State>(ci, si).last_swap != src {
                    return;
                }
                let st = s.set_st_mut::<State>(ci, si);
                st.stacks = (st.stacks + 1).min(4);
                st.last_gain = f;
                s.add_task(Task::Set { c: ci as u8, si: si as u8, id: TASK_OFF_FIELD, arg: src }, 180);
                queue_loss(s, ci, si, f);
            }
            // 6 s without a gain loses a stack, and again every 6 s
            _ => {
                let st = s.set_st_mut::<State>(ci, si);
                if st.last_gain != src {
                    return;
                }
                st.stacks -= 1;
                if st.stacks > 0 {
                    queue_loss(s, ci, si, src);
                }
            }
        }
    }

    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let f = s.f;
        match *e {
            Event::Swap { prev, .. } if prev == ci => {
                s.set_st_mut::<State>(ci, si).last_swap = f;
                s.add_task(Task::Set { c: ci as u8, si: si as u8, id: TASK_OFF_FIELD, arg: f }, 180);
            }
            // on field, a stack on a geo hit by the wearer, once every 0.3 s
            Event::EnemyDamage(ev) => {
                if s.player.active as usize != ci || ev.info.actor as usize != ci || s.mod_active(ci, GAIN_ICD) || ev.info.element != Element::Geo {
                    return;
                }
                let st = s.set_st_mut::<State>(ci, si);
                st.stacks = (st.stacks + 1).min(4);
                st.last_gain = f;
                s.add_status(ci, GAIN_ICD, 18, true);
                queue_loss(s, ci, si, f);
            }
            _ => {}
        }
    }
}
