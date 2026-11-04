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
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) || ev.info.tag != AttackTag::Extra || !ev.crit {
            return;
        }
        s.add_status(ci, ICD, 600, true);
        let mult = 0.75 + s.chars[ci].refine as f64 * 0.25;
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, mult);
        ai.abil = "Messenger Proc";
        ai.pattern = Pattern::Single;
        let mut snap = s.snapshot_event(ai, 0);
        snap.stats[st::CR] += 1.0;
        s.queue_attack_with_snap(snap, 0, 1);
    }
}
