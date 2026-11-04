//! Messenger, from gcsim's internal/weapons/bow/messenger.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("messenger-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "messenger";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        // gcsim triggers on a hit on a weak point (an aimed shot with
        // weakspot=1), of any attack tag
        if ev.info.actor as usize != ci || s.player.active as usize != ci || !ev.info.weak_point || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 10 * 60, true);
        let mult = 0.75 + s.chars[ci].refine as f64 * 0.25;
        // AttackTagNone, StrikeTypePierce; HitWeakPoint makes it crit
        let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, mult);
        ai.abil = "Messenger Proc";
        ai.pattern = Pattern::Single;
        ai.weak_point = true;
        s.queue_attack(ai, 0, 0, 1);
    }
}
