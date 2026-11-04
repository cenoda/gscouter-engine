//! Night of the Sky's Unveiling, from gcsim's
//! internal/artifacts/nightoftheskysunveiling.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State {
    /// Gleaming Moon: Intent's CRIT Rate, by the Moonsign level.
    cr: f64,
}

const TWO: ModKey = ModKey::new("skys-unveiling-2pc");
const INTENT_CR: ModKey = ModKey::new("gleaming-moon-intent-cr");
const INTENT_REACT: ModKey = ModKey::new("gleaming-moon-intent-reaction");

impl SetImpl for Set {
    const KEY: &'static str = "nightoftheskysunveiling";
    type State = State;
    fn new_state() -> State {
        State { cr: 0.0 }
    }

    fn init(_: &mut Sim, _: usize, _: usize, _: i32) {}

    /// gcsim adds it all in Set.Init: EM +80; with four pieces, a lunar
    /// reaction on the enemy while the wearer is on field gives it CRIT
    /// Rate (15%, 30% at Ascendant Gleam's level, none without Moonsign)
    /// for 4 s, and while anyone has it, lunar reactions take +10%.
    fn post_init(s: &mut Sim, ci: usize, si: usize) {
        let count = s.chars[ci].sets[si].count;
        s.chars[ci].mods.push(Mod::stat(TWO, st::EM, 80.0, false, false, -1));
        if count < 4 {
            return;
        }
        let level: i32 = s.chars.iter().map(|c| c.moonsign).sum();
        s.set_st_mut::<State>(ci, si).cr = match level {
            0 => 0.0,
            1 => 0.15,
            _ => 0.30,
        };
        let o = Owner::Set(ci as u8, si as u8);
        s.subscribe(EventKind::Reaction, o);
        for c in 0..s.chars.len() {
            s.add_mod(c, Mod::owned(INTENT_REACT, ModKind::ReactBonus, o, st::NONE, false, false, -1), -1);
        }
    }

    fn react_bonus(s: &Sim, _: usize, _: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if info.tag.is_lunar() && (0..s.chars.len()).any(|c| s.mod_active(c, INTENT_CR)) {
            0.1
        } else {
            0.0
        }
    }

    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        if !matches!(r, Reaction::LunarCharged | Reaction::LunarBloom | Reaction::LunarCrystallize) || !ev.on_enemy() || s.player.active as usize != ci {
            return;
        }
        let cr = s.set_st::<State>(ci, si).cr;
        s.add_stat_mod(ci, INTENT_CR, st::CR, cr, 4 * 60, false);
    }
}
