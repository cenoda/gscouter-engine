//! Prototype Archaic, from gcsim's internal/weapons/claymore/prototype.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("prototype-archaic-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "prototypearchaic";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        if s.rng.float64() >= 0.5 {
            return;
        }
        s.add_status(ci, ICD, 900, true);
        let r = s.chars[ci].refine as f64;
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 1.8 + 0.6 * r);
        ai.abil = "Prototype Archaic Proc";
        ai.pattern = s.circle_on_target(3.0);
        s.queue_attack(ai, 0, 0, 1);
    }
}
