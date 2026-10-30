//! Golden Troupe, from gcsim's internal/artifacts/goldentroupe.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State {
    /// The frame the holder last took the field, -1 while off it.
    last_swap: i32,
    /// The 4-piece bonus: 0.25, or 0.5 off the field.
    buff: f64,
}

const TWO: ModKey = ModKey::new("troupe-2pc");
const FOUR: ModKey = ModKey::new("troupe-4pc");

const TASK_CLEAR: u8 = 0;

fn is_skill(ev: &AttackEvent) -> bool {
    matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold)
}

impl SetImpl for Set {
    const KEY: &'static str = "goldentroupe";
    type State = State;
    fn new_state() -> State {
        State { last_swap: -1, buff: 0.0 }
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        let o = Owner::Set(ci as u8, si as u8);
        if count >= 2 {
            s.chars[ci].mods.push(Mod::owned(TWO, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        if count >= 4 {
            s.set_st_mut::<State>(ci, si).buff = 0.25;
            s.subscribe(EventKind::Swap, o);
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Attack, o, st::NONE, false, false, -1));
        }
    }

    fn post_init(s: &mut Sim, ci: usize, si: usize) {
        if s.set_st::<State>(ci, si).buff != 0.0 && s.player.active as usize != ci {
            s.set_st_mut::<State>(ci, si).buff = 0.5;
        }
    }

    fn attack_mod(s: &Sim, ci: usize, si: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if !is_skill(ev) {
            return;
        }
        ev.stats[st::DMGP] += if m.key == TWO { 0.2 } else { s.set_st::<State>(ci, si).buff };
    }

    /// Off the field the bonus doubles; it is cleared 2 s after taking the
    /// field.
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::Swap { prev, next } = *e else { return };
        let f = s.f;
        let st = s.set_st_mut::<State>(ci, si);
        if prev == ci {
            st.last_swap = -1;
            st.buff = 0.5;
        } else if next == ci {
            st.last_swap = f;
            s.add_task(Task::Set { c: ci as u8, si: si as u8, id: TASK_CLEAR, arg: f }, 120);
        }
    }

    fn task(s: &mut Sim, ci: usize, si: usize, _: u8, src: i32) {
        let active = s.player.active as usize == ci;
        let st = s.set_st_mut::<State>(ci, si);
        if st.last_swap != src || !active {
            return;
        }
        st.buff = 0.25;
    }
}
