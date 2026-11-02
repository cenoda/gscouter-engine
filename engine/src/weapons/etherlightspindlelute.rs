//! Etherlight Spindlelute, from gcsim's
//! internal/weapons/catalyst/etherlightspindlelute.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("etherlight");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "etherlightspindlelute";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    /// A skill by the holder on field: EM +100 to 200 for 20 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Skill = *e {
            if s.player.active as usize != ci {
                return;
            }
            let v = 75.0 + 25.0 * s.chars[ci].refine as f64;
            s.add_stat_mod(ci, BUFF, st::EM, v, 20 * 60, true);
        }
    }
}
