//! Scroll of the Hero of Cinder City, from gcsim's
//! internal/artifacts/scrolloftheheroofcindercity. The engine has no
//! Nightsoul yet: no character it implements enters Nightsoul's Blessing or
//! triggers a Nightsoul Burst, so the 2-piece and the 4-piece's Nightsoul
//! bonus never apply.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

/// gcsim's "scroll-4pc-<element>".
fn key(e: Element) -> ModKey {
    match e {
        Element::Pyro => ModKey::new("scroll-4pc-pyro"),
        Element::Hydro => ModKey::new("scroll-4pc-hydro"),
        Element::Dendro => ModKey::new("scroll-4pc-dendro"),
        Element::Electro => ModKey::new("scroll-4pc-electro"),
        Element::Cryo => ModKey::new("scroll-4pc-cryo"),
        Element::Anemo => ModKey::new("scroll-4pc-anemo"),
        Element::Geo => ModKey::new("scroll-4pc-geo"),
        _ => unreachable!(),
    }
}

impl SetImpl for Set {
    const KEY: &'static str = "scrolloftheheroofcindercity";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 4 {
            s.subscribe(EventKind::Reaction, Owner::Set(ci as u8, si as u8));
        }
    }

    /// A reaction of the holder's element by the holder, on or off field:
    /// 12% DMG bonus of each element involved to the party for 15 s.
    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        let elements = r.elements();
        if ev.info.actor as usize != ci || !elements.contains(&s.chars[ci].element) {
            return;
        }
        for i in 0..s.chars.len() {
            for &ele in elements {
                s.add_stat_mod(i, key(ele), ele.dmg_stat(), 0.12, 15 * 60, true);
            }
        }
    }
}
