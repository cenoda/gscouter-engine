//! Traveler's Handy Sword, from gcsim's
//! internal/weapons/sword/travelershandysword.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;

pub struct Weapon;
pub struct State;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "travelershandysword";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::ParticleReceived, Owner::Weapon(ci as u8));
    }

    /// A particle taken while the holder is on field heals 1 to 2% of max
    /// HP. gcsim's HealInfo leaves Caller and Target unset, so the heal
    /// goes to the team's first member, from the first member, without a
    /// bonus.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::ParticleReceived = *e else { return };
        if s.player.active as usize != ci {
            return;
        }
        let src = 0.0075 + s.chars[ci].refine as f64 * 0.0025;
        s.heal_info(0, Some(0), true, src, 0.0);
    }
}
