//! Kagotsurube Isshin, from gcsim's internal/weapons/sword/kagotsurubeisshin.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("kagotsurube-isshin");
const ICD: ModKey = ModKey::new("kagotsurube-isshin-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "kagotsurubeisshin";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// A normal, charged or plunging hit by the holder on field, once every
    /// 8 s (hitlag): ATK +15% for 8 s (hitlag) and a Hewing Gale of 180% of
    /// ATK, physical, in a circle of radius 3 on the target hit.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            return;
        }
        s.add_mod(ci, Mod::stat(BUFF, st::ATKP, 0.15, false, true, -1), 480);
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 1.8);
        ai.abil = "Kagotsurube Isshin Proc";
        ai.pattern = s.circle_on_target(3.0);
        s.queue_attack(ai, 0, 0, 1);
        s.add_status(ci, ICD, 480, true);
    }
}
