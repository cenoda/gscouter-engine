//! Unfinished Reverie, from gcsim's internal/artifacts/unfinishedreverie.

use crate::content::SetImpl;
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State {
    /// gcsim's lastF: the last check that found a burning enemy nearby.
    last_f: i32,
    /// DMG +10% per stack, 0 to 5; the wearer starts with 5.
    stacks: i32,
    /// gcsim's icdF: a stack changes only after this frame.
    icd_f: i32,
}

const TWO: ModKey = ModKey::new("unfinishedreverie-2pc");
const FOUR: ModKey = ModKey::new("unfinishedreverie-4pc");

const TASK_CHECK: u8 = 0;

impl SetImpl for Set {
    const KEY: &'static str = "unfinishedreverie";
    type State = State;
    fn new_state() -> State {
        State { last_f: -1, stacks: 5, icd_f: -1 }
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        if count >= 4 {
            s.queue_char_task(ci, Task::Set { c: ci as u8, si: si as u8, id: TASK_CHECK, arg: 0 }, 30);
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Stat, Owner::Set(ci as u8, si as u8), st::NONE, false, false, -1));
        }
    }

    fn mod_amount(s: &Sim, ci: usize, si: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::DMGP] += 0.1 * s.set_st::<State>(ci, si).stacks as f64;
        true
    }

    /// Every 0.5 s on the wearer's queue: a burning enemy within 8 of the
    /// player keeps the stacks up and adds one, none for over 6 s takes one
    /// away, at most one change a second.
    fn task(s: &mut Sim, ci: usize, si: usize, _: u8, _: i32) {
        let f = s.f;
        let burning = s.enemy_alive && s.enemy_within(s.circle_on_player(0.0, 0.0, 8.0)) && s.enemy.aura.get(AURA_BURNING_FUEL) > ZERO_DUR && s.enemy.aura.get(AURA_BURNING) > ZERO_DUR;
        let st = s.set_st_mut::<State>(ci, si);
        if burning {
            st.last_f = f;
        }
        if f - st.last_f > 6 * 60 && f > st.icd_f && st.stacks > 0 {
            st.stacks -= 1;
            st.icd_f = f + 60;
        } else if f == st.last_f && f > st.icd_f && st.stacks < 5 {
            st.stacks += 1;
            st.icd_f = f + 60;
        }
        s.queue_char_task(ci, Task::Set { c: ci as u8, si: si as u8, id: TASK_CHECK, arg: 0 }, 30);
    }
}
