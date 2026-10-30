//! gcsim's internal/weapons/common/favonius, which every Favonius weapon
//! (sword, claymore, spear, catalyst, bow) is.

use crate::content::{Event, Owner};
use crate::sim::*;
use crate::types::*;

const ICD: ModKey = ModKey::new("favonius-cd");

pub fn init(s: &mut Sim, ci: usize) {
    s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
}

/// A crit by the holder, on field: by chance, 3 particles.
pub fn event(s: &mut Sim, ci: usize, e: &Event) {
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
