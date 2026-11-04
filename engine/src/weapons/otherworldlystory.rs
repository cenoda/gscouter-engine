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
        s.heal_info(0, Some(ci), true, pct, 0.0);
    }
}
