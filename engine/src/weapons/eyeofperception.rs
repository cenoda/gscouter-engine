//! Eye of Perception, from gcsim's internal/weapons/catalyst/perception:
//! a normal or charged attack by the holder that deals damage, once every
//! 12 to 8 s (hitlag), fires a bolt at the closest enemy within 8 m of the
//! player (its center inside the circle) that has no bounce status: a
//! circle of 0.6 on it, 10 frames later, snapshot now, and the enemy takes
//! the bounce status for 0.6 s (hitlag). gcsim's multiplier is
//! 2.1 * refinement * 0.3 (0.63 at R1), not the in-game 240%; it is kept.
//!
//! Each hit on an enemy would bounce to the closest enemy within 8 m of it
//! without the bounce status, up to 4 times. The engine fights one enemy,
//! whose bounce status is still on when the bolt lands 10 frames later, so
//! a bounce never finds a target, as in gcsim against one enemy.

use crate::content::{Event, Owner, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("perception-icd");
const BOUNCE: ModKey = ModKey::new("eye-of-perception-bounce");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "eyeofperception";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) || s.mod_active(ci, ICD) {
            return;
        }
        let r = s.chars[ci].refine;
        s.add_status(ci, ICD, (13 - r) * 60, true);
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 2.1 * r as f64 * 0.3);
        ai.abil = "Eye of Preception Proc";
        // gcsim snapshots before it looks for the enemy
        let mut snap = s.snapshot_event(ai, 0);
        if !s.enemy_alive || s.enemy_status_active(BOUNCE) || !s.enemy_within(s.circle_on_player(0.0, 0.0, 8.0)) {
            return;
        }
        s.enemy_add_status(BOUNCE, 36, true);
        snap.info.pattern = s.circle_on_target(0.6);
        s.queue_attack_with_snap(snap, 0, 10);
    }
}
