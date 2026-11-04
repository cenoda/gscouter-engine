//! Fillet Blade, from gcsim's internal/weapons/sword/filletblade.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("fillet-blade-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "filletblade";
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
        if s.rng.float64() > 0.5 {
            return;
        }
        if ev.damage == 0.0 {
            return;
        }
        let r = s.chars[ci].refine;
        let cd = 960 - 60 * r;
        let mult = 2.0 + 0.4 * r as f64;
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, mult);
        ai.abil = "Fillet Blade Proc";
        ai.pattern = Pattern::Single;
        s.queue_attack(ai, 0, 0, 1);
        s.add_status(ci, ICD, cd, true);
    }
}
