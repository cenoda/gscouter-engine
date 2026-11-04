//! Otherworldly Story, from gcsim's internal/weapons/catalyst/otherworldly.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;

pub struct Weapon;
pub struct State;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "otherworldlystory";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::ParticleReceived, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::ParticleReceived = *e else { return };
        if s.player.active as usize != ci {
            return;
        }
        let pct = 0.0075 + 0.0025 * s.chars[ci].refine as f64;
        // gcsim's HealInfo leaves Caller and Target at 0: the heal goes to
        // the character in the first slot, not the wearer, by caller 0
        s.heal_info(0, Some(0), true, pct, 0.0);
    }
}
