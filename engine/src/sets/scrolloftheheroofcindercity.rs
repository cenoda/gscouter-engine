//! Scroll of the Hero of Cinder City, from gcsim's
//! internal/artifacts/scrolloftheheroofcindercity: the 2-piece's energy on
//! a Nightsoul Burst, and the 4-piece's DMG bonuses, with the additional
//! one when the holder is in Nightsoul's Blessing.

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

/// gcsim's "scroll-4pc-nightsoul-<element>".
fn nightsoul_key(e: Element) -> ModKey {
    match e {
        Element::Pyro => ModKey::new("scroll-4pc-nightsoul-pyro"),
        Element::Hydro => ModKey::new("scroll-4pc-nightsoul-hydro"),
        Element::Dendro => ModKey::new("scroll-4pc-nightsoul-dendro"),
        Element::Electro => ModKey::new("scroll-4pc-nightsoul-electro"),
        Element::Cryo => ModKey::new("scroll-4pc-nightsoul-cryo"),
        Element::Anemo => ModKey::new("scroll-4pc-nightsoul-anemo"),
        Element::Geo => ModKey::new("scroll-4pc-nightsoul-geo"),
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
        if count >= 2 {
            s.subscribe(EventKind::NightsoulBurst, Owner::Set(ci as u8, si as u8));
        }
        if count >= 4 {
            s.subscribe(EventKind::Reaction, Owner::Set(ci as u8, si as u8));
        }
    }

    /// 2-piece: a Nightsoul Burst gives the holder 6 energy. 4-piece: a
    /// reaction of the holder's element by the holder, on or off field:
    /// 12% DMG bonus of each element involved to the party for 15 s, and
    /// 28% more for 20 s if the holder is in Nightsoul's Blessing.
    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        if let Event::NightsoulBurst(_) = *e {
            s.add_energy(ci, 6.0);
            return;
        }
        let Event::Reaction(r, ev) = *e else { return };
        // only reactions on an enemy (or, for burgeon and hyperbloom, a gadget)
        if ev.on_player {
            return;
        }
        let elements = r.elements();
        if ev.info.actor as usize != ci || !elements.contains(&s.chars[ci].element) {
            return;
        }
        let nightsoul = s.ns_has_blessing(ci);
        for i in 0..s.chars.len() {
            for &ele in elements {
                s.add_stat_mod(i, key(ele), ele.dmg_stat(), 0.12, 15 * 60, true);
                if nightsoul {
                    s.add_stat_mod(i, nightsoul_key(ele), ele.dmg_stat(), 0.28, 20 * 60, true);
                }
            }
        }
    }
}
