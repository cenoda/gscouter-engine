//! White Iron Greatsword, from gcsim's internal/weapons/claymore/whiteiron.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;

pub struct Weapon;
pub struct State;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "whiteirongreatsword";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::TargetDied, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::TargetDied(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        let pct = 0.06 + 0.02 * s.chars[ci].refine as f64;
        // gcsim's HealInfo leaves Caller and Target at 0: it heals the
        // character in the first slot, by its max HP
        s.heal_info(0, Some(0), true, pct, 0.0);
    }
}
