//! Favonius Warbow, from gcsim's internal/weapons/common/favonius.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("favonius-cd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "favoniuswarbow";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.damage == 0.0 || !ev.crit || ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        let r = s.chars[ci].refine;
        if s.rng.float64() > 0.50 + r as f64 * 0.1 {
            return;
        }
        s.queue_particle(3.0, Element::NoElement, s.particle_delay[ci]);
        s.add_status(ci, ICD, 810 - r * 90, true);
    }
}
