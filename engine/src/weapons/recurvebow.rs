//! Recurve Bow, from gcsim's internal/weapons/bow/recurve.
//!
//! gcsim's heal names no target, so it heals the party's first character
//! (HealInfo.Target 0, Caller 0), as here; it follows a kill, which ends a
//! damage-mode fight.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;

pub struct Weapon;
pub struct State;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "recurvebow";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::TargetDied, Owner::Weapon(ci as u8));
    }

    /// The holder on field defeating the enemy: 8 to 16% of max HP.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::TargetDied(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        let pct = 0.06 + 0.02 * s.chars[ci].refine as f64;
        s.heal_info(0, Some(0), true, pct, 0.0);
    }
}
