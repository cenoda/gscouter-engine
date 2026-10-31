//! Gilded Dreams, from gcsim's internal/artifacts/gildeddreams.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
#[derive(Default)]
pub struct State {
    em: f64,
    atk: f64,
}

const TWO: ModKey = ModKey::new("gd-2pc");
const FOUR: ModKey = ModKey::new("gd-4pc");
const ICD: ModKey = ModKey::new("gd-4pc-icd");

impl SetImpl for Set {
    const KEY: &'static str = "gildeddreams";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::EM, 80.0, false, false, -1));
        if count >= 4 {
            s.subscribe(EventKind::Reaction, Owner::Set(ci as u8, si as u8));
        }
    }

    /// Set.Init: 50 EM for each other member of another element, 14% ATK
    /// for each of the holder's, up to 3 each.
    fn post_init(s: &mut Sim, ci: usize, si: usize) {
        let ele = s.chars[ci].element;
        let same = (0..s.chars.len()).filter(|&i| i != ci && s.chars[i].element == ele).count().min(3);
        let other = (0..s.chars.len()).filter(|&i| i != ci && s.chars[i].element != ele).count().min(3);
        let st = s.set_st_mut::<State>(ci, si);
        st.em = 50.0 * other as f64;
        st.atk = 0.14 * same as f64;
    }

    fn mod_amount(s: &Sim, ci: usize, si: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let st = s.set_st::<State>(ci, si);
        out[st::EM] += st.em;
        out[st::ATKP] += st.atk;
        true
    }

    /// Any reaction the holder triggers, on any target, on or off field:
    /// the buff for 8 s, once every 8 s.
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::Reaction(_, ev) = *e else { return };
        if ev.info.actor as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 8 * 60, true);
        s.add_mod(ci, Mod::owned(FOUR, ModKind::Stat, Owner::Set(ci as u8, si as u8), st::NONE, false, true, -1), 8 * 60);
    }
}
