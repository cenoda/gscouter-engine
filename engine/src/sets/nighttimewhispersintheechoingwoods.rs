//! Nighttime Whispers in the Echoing Woods, from gcsim's
//! internal/artifacts/nighttimewhispersintheechoingwoods.
//!
//! Its Moondrift part reads Lunar-Crystallize's Moondrift constructs, of
//! which the engine never has any: Lunar-Crystallize is an error
//! (`Sim::check_lunar_crystallize`). So gcsim's moondriftTicker never finds
//! one, never adds the Moondrift status, and changes nothing; it is left
//! out, and the status always reads 0. Port it with Lunar-Crystallize.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State {
    /// gcsim's lastF: the Geo DMG bonus is 2.5 times up to this frame (the
    /// wearer's crystallize shield and 1 s after it).
    last_f: i32,
}

const TWO: ModKey = ModKey::new("nighttimewhispers-2pc");
const FOUR: ModKey = ModKey::new("nighttimewhispers-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "nighttimewhispersintheechoingwoods";
    type State = State;
    fn new_state() -> State {
        State { last_f: 0 }
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        if count >= 4 {
            let o = Owner::Set(ci as u8, si as u8);
            s.subscribe(EventKind::Shielded, o);
            s.subscribe(EventKind::ShieldBreak, o);
            s.subscribe(EventKind::Swap, o);
            s.subscribe(EventKind::Skill, o);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, si: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::GEOP] += if s.f <= s.set_st::<State>(ci, si).last_f { 0.2 * 2.5 } else { 0.2 };
        true
    }

    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let f = s.f;
        let active = s.player.active as usize == ci;
        match *e {
            Event::Shielded(sh) if active && sh.typ == ShieldType::Crystallize => s.set_st_mut::<State>(ci, si).last_f = sh.expiry,
            Event::ShieldBreak(sh) if active && sh.typ == ShieldType::Crystallize => s.set_st_mut::<State>(ci, si).last_f = f + 60,
            Event::Swap { prev, next } => {
                let Some(expiry) = s.shields.iter().find(|sh| sh.typ == ShieldType::Crystallize).map(|sh| sh.expiry) else { return };
                // the Moondrift status's expiry is f (see above)
                if next == ci {
                    s.set_st_mut::<State>(ci, si).last_f = expiry.max(f);
                } else if prev == ci {
                    s.set_st_mut::<State>(ci, si).last_f = f + 60;
                }
            }
            // a skill on field: Geo DMG +20% for 10 s (hitlag), 2.5 times
            // that while protected by crystallize
            Event::Skill if active => {
                s.add_mod(ci, Mod::owned(FOUR, ModKind::Stat, Owner::Set(ci as u8, si as u8), st::GEOP, false, true, -1), 10 * 60);
            }
            _ => {}
        }
    }
}
