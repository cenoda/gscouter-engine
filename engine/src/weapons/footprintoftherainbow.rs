//! Footprint of the Rainbow, from gcsim's internal/weapons/spear/footprint.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("footprint-def");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "footprintoftherainbow";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    /// A skill by the holder on field: DEF +16% to 32% for 15 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Skill = *e {
            if s.player.active as usize != ci {
                return;
            }
            let v = 0.12 + 0.04 * s.chars[ci].refine as f64;
            s.add_stat_mod(ci, BUFF, st::DEFP, v, 15 * 60, true);
        }
    }
}
