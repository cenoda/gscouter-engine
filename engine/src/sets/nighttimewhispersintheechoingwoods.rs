//! Nighttime Whispers in the Echoing Woods, from gcsim's
//! internal/artifacts/nighttimewhispersintheechoingwoods.
//!
//! Its Moondrift part (moondriftTicker) reads Lunar-Crystallize's
//! Moondrifts: one within 20 of the player keeps the 2.5x bonus up while
//! the wearer is on field after a skill or a swap in.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State {
    /// gcsim's lastF: the Geo DMG bonus is 2.5 times up to this frame (the
    /// wearer's crystallize shield and 1 s after it).
    last_f: i32,
    /// gcsim's tickerSrcF.
    ticker_src: i32,
}

const TWO: ModKey = ModKey::new("nighttimewhispers-2pc");
const FOUR: ModKey = ModKey::new("nighttimewhispers-4pc");
/// gcsim's core status "nighttimewhispers-4pc-moondrift".
const MOONDRIFT_STATUS: ModKey = ModKey::new("nighttimewhispers-4pc-moondrift");

const TASK_TICKER: u8 = 0;

/// gcsim's moondriftTicker, every 2 s while the wearer is on field with
/// the 4-piece bonus up.
fn moondrift_ticker(s: &mut Sim, ci: usize, si: usize, src: i32) {
    if s.set_st::<State>(ci, si).ticker_src != src || s.player.active as usize != ci || !s.mod_active(ci, FOUR) {
        return;
    }
    let f = s.f;
    let (px, py) = s.player_pos;
    let near = s.constructs.iter().any(|c| c.key == MOONDRIFT && c.pos.is_some_and(|p| ((p[0] - px).powi(2) + (p[1] - py).powi(2)).sqrt() < 20.0));
    if near {
        s.add_core_status(MOONDRIFT_STATUS, 120);
        s.set_st_mut::<State>(ci, si).last_f = f + 120;
    } else if s.core_status_key(MOONDRIFT_STATUS) > 0 {
        s.add_core_status(MOONDRIFT_STATUS, 60);
        s.set_st_mut::<State>(ci, si).last_f = f + 60;
    }
    s.add_task(Task::Set { c: ci as u8, si: si as u8, id: TASK_TICKER, arg: src }, 120);
}

fn start_ticker(s: &mut Sim, ci: usize, si: usize) {
    let f = s.f;
    s.set_st_mut::<State>(ci, si).ticker_src = f;
    moondrift_ticker(s, ci, si, f);
}

impl SetImpl for Set {
    const KEY: &'static str = "nighttimewhispersintheechoingwoods";
    type State = State;
    fn new_state() -> State {
        State { last_f: 0, ticker_src: 0 }
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
                let moondrift = f + s.core_status_key(MOONDRIFT_STATUS);
                let Some(expiry) = s.shields.iter().find(|sh| sh.typ == ShieldType::Crystallize).map(|sh| sh.expiry) else { return };
                if next == ci {
                    start_ticker(s, ci, si);
                    s.set_st_mut::<State>(ci, si).last_f = expiry.max(moondrift);
                } else if prev == ci {
                    s.set_st_mut::<State>(ci, si).last_f = f + 60;
                }
            }
            // a skill on field: Geo DMG +20% for 10 s (hitlag), 2.5 times
            // that while protected by crystallize
            Event::Skill if active => {
                start_ticker(s, ci, si);
                s.add_mod(ci, Mod::owned(FOUR, ModKind::Stat, Owner::Set(ci as u8, si as u8), st::GEOP, false, true, -1), 10 * 60);
            }
            _ => {}
        }
    }

    fn task(s: &mut Sim, ci: usize, si: usize, id: u8, arg: i32) {
        match id {
            TASK_TICKER => moondrift_ticker(s, ci, si, arg),
            _ => unreachable!(),
        }
    }
}
