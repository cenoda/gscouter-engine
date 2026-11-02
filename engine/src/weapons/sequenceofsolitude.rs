//! Sequence of Solitude, from gcsim's internal/weapons/bow/sequenceofsolitude.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("sequence-of-solitude-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "sequenceofsolitude";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// A hit by the holder on field, once every 15 s (hitlag): physical
    /// damage of 40 to 80% of its max HP, flat, in a circle of radius 3 on
    /// the enemy, a frame later.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 15 * 60, true);
        let hp = 0.3 + s.chars[ci].refine as f64 * 0.1;
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 0.0);
        ai.abil = "Sequence of Solitude Proc";
        ai.flat_dmg = s.max_hp(ci) * hp;
        ai.pattern = s.circle_on_target(3.0);
        s.queue_attack(ai, 0, 0, 1);
    }
}
