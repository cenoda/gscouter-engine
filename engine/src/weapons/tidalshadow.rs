//! Tidal Shadow, from gcsim's internal/weapons/claymore/tidalshadow.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("tidal-shadow-atk-boost");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "tidalshadow";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Heal, Owner::Weapon(ci as u8));
    }

    /// The holder healed (by anyone, also for 0): ATK +24 to 48% for 8 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Heal { on, .. } = *e else { return };
        if on != ci {
            return;
        }
        let v = 0.18 + s.chars[ci].refine as f64 * 0.06;
        s.add_stat_mod(ci, BUFF, st::ATKP, v, 8 * 60, true);
    }
}
