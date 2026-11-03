//! Archaic Petra, from gcsim's internal/artifacts/archaic: a crystallize
//! shard picked up by the holder on field sets the 4-piece's element, and
//! a Lunar-Crystallize (by anyone) while the holder is on field sets hydro.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State {
    /// The element of the 4-piece's bonus, from the last shard picked up.
    element: Element,
}

const TWO: ModKey = ModKey::new("archaic-2pc");
const FOUR: ModKey = ModKey::new("archaic-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "archaicpetra";
    type State = State;
    fn new_state() -> State {
        State { element: Element::NoElement }
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::GEOP, 0.15, false, false, -1));
        if count >= 4 {
            s.subscribe(EventKind::Shielded, Owner::Set(ci as u8, si as u8));
            s.subscribe(EventKind::Reaction, Owner::Set(ci as u8, si as u8));
        }
    }

    /// The party's DMG bonus of the element, 35%, for every character's
    /// mod from this holder (gcsim shares one slice among them).
    fn mod_amount(s: &Sim, ci: usize, si: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let e = s.set_st::<State>(ci, si).element;
        out[e.dmg_stat()] += 0.35;
        true
    }

    /// A crystallize shard picked up by the holder on field, or a
    /// Lunar-Crystallize (gcsim's OnLunarCrystallize checks only that the
    /// holder is on field).
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let ele = match *e {
            Event::Shielded(sh) if sh.typ == ShieldType::Crystallize => sh.ele,
            Event::Reaction(Reaction::LunarCrystallize, _) => Element::Hydro,
            _ => return,
        };
        if s.player.active as usize != ci {
            return;
        }
        enable(s, ci, si, ele);
    }
}

/// gcsim's enableSet: the element's 35% for the whole party, 10 s (hitlag).
fn enable(s: &mut Sim, ci: usize, si: usize, ele: Element) {
    s.set_st_mut::<State>(ci, si).element = ele;
    for i in 0..s.chars.len() {
        s.add_mod(i, Mod::owned(FOUR, ModKind::Stat, Owner::Set(ci as u8, si as u8), st::NONE, false, true, -1), 10 * 60);
    }
}
