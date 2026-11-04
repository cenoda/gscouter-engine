//! Skyrider Sword, from gcsim's internal/weapons/sword/skyrider.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("skyrider");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "skyridersword";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Burst, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Burst = *e else { return };
        if s.player.active as usize != ci {
            return;
        }
        let val = 0.09 + 0.03 * s.chars[ci].refine as f64;
        s.add_mod(ci, Mod::stat(BUFF, st::ATKP, val, false, true, -1), 900);
    }
}
