//! Halberd, from gcsim's internal/weapons/spear/halberd.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("halberd-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "halberd";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) || ev.info.tag != AttackTag::Normal {
            return;
        }
        s.add_status(ci, ICD, 600, true);
        let mult = 1.2 + s.chars[ci].refine as f64 * 0.4;
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, mult);
        ai.abil = "Halberd Proc";
        ai.pattern = Pattern::Single;
        s.queue_attack(ai, 0, 0, 1);
    }
}
