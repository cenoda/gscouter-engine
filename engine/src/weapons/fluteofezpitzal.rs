//! Flute of Ezpitzal, from gcsim's internal/weapons/sword/fluteofezpitzal.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("flute-of-ezpitzal-def-boost");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "fluteofezpitzal";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    /// A skill on field: DEF +16 to 32% for 15 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Skill = *e {
            if s.player.active as usize == ci {
                let def = 0.12 + s.chars[ci].refine as f64 * 0.04;
                s.add_mod(ci, Mod::stat(KEY, st::DEFP, def, false, true, -1), 15 * 60);
            }
        }
    }
}
