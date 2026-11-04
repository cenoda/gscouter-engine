//! Sword of Descension, from gcsim's internal/weapons/sword/swordofdescension.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ATK_PERM: ModKey = ModKey::new("descension-atk");
const ICD: ModKey = ModKey::new("swordofdescension-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "swordofdescension";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].kind.traveler() {
            s.chars[ci].mods.push(Mod::stat(ATK_PERM, st::ATK, 66.0, false, false, -1));
        }
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) || ev.damage == 0.0 {
            return;
        }
        if s.rng.float64() < 0.5 {
            return;
        }
        s.add_status(ci, ICD, 600, true);
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 2.0);
        ai.abil = "Sword of Descension Proc";
        ai.pattern = s.circle_on_target(1.5);
        s.queue_attack(ai, 0, 0, 1);
    }
}
