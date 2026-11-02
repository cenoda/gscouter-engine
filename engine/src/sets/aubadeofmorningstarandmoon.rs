//! Aubade of Morningstar and Moon, from gcsim's internal/artifacts/aubade.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State {
    /// Lunar reaction DMG: 20%, 60% at Ascendant Gleam's level.
    buff: f64,
}

const TWO: ModKey = ModKey::new("aubade-2pc");
const FOUR: ModKey = ModKey::new("aubade-4pc-dmg");
const ON_FIELD: ModKey = ModKey::new("aubade-4pc-onfield");

impl SetImpl for Set {
    const KEY: &'static str = "aubadeofmorningstarandmoon";
    type State = State;
    fn new_state() -> State {
        State { buff: 0.0 }
    }

    fn init(_: &mut Sim, _: usize, _: usize, _: i32) {}

    /// gcsim adds it all in Set.Init: EM +80; with four pieces lunar
    /// reaction DMG while off field, and for 3 s after coming on.
    fn post_init(s: &mut Sim, ci: usize, si: usize) {
        let count = s.chars[ci].sets[si].count;
        s.chars[ci].mods.push(Mod::stat(TWO, st::EM, 80.0, false, false, -1));
        if count < 4 {
            return;
        }
        let level: i32 = s.chars.iter().map(|c| c.moonsign).sum();
        s.set_st_mut::<State>(ci, si).buff = if level >= 2 { 0.6 } else { 0.2 };
        let o = Owner::Set(ci as u8, si as u8);
        s.subscribe(EventKind::Swap, o);
        s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::ReactBonus, o, st::NONE, false, false, -1));
    }

    fn react_bonus(s: &Sim, ci: usize, si: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if s.player.active as usize == ci && !s.mod_active(ci, ON_FIELD) {
            return 0.0;
        }
        if info.tag.is_lunar() {
            s.set_st::<State>(ci, si).buff
        } else {
            0.0
        }
    }

    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let Event::Swap { prev, next } = *e else { return };
        if prev == ci {
            s.delete_mod(ci, ON_FIELD);
        } else if next == ci {
            s.add_status(ci, ON_FIELD, 3 * 60, true);
        }
    }
}
