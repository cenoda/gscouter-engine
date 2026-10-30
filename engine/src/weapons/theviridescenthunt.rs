//! The Viridescent Hunt, from gcsim's internal/weapons/bow/viridescent.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("viridescent-hunt-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "theviridescenthunt";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// A normal or charged hit by the holder on field has half a chance to
    /// summon a cyclone: nine hits over 4 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) || s.mod_active(ci, ICD) {
            return;
        }
        if s.rng.float64() > 0.5 {
            return;
        }
        let r = s.chars[ci].refine;
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 0.3 + r as f64 * 0.1);
        ai.abil = "Viridescent";
        for i in (0..=240).step_by(30) {
            s.queue_attack(ai, 0, 0, i + 1);
        }
        s.add_status(ci, ICD, 900 - r * 60, true);
    }
}
