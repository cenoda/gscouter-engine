//! Twin Nephrite, from gcsim's internal/weapons/catalyst/twin.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("twinnephrite");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "twinnephrite";
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
        let val = 0.10 + 0.02 * s.chars[ci].refine as f64;
        s.add_mod(ci, Mod::stat(BUFF, st::ATKP, val, false, true, -1), 900);
    }
}
