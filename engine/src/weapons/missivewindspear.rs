//! Missive Windspear, from gcsim's internal/weapons/spear/missive.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("missive");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "missivewindspear";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    /// ATK +12 to 24% and EM +48 to 96.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        out[st::ATKP] += 0.09 + r * 0.03;
        out[st::EM] += 36.0 + r * 12.0;
        true
    }

    /// Any reaction event by the holder (gcsim's whole reaction range,
    /// shatter included, on any target) with the holder on field: the buff
    /// for 10 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(_, ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        s.add_mod(ci, Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 10 * 60);
    }
}
