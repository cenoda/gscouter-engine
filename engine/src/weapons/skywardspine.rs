//! Skyward Spine, from gcsim's internal/weapons/spear/skyward.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const PERM_CR: ModKey = ModKey::new("skyward spine");
const PERM_SPD: ModKey = ModKey::new("skyward spine spd");
const ICD: ModKey = ModKey::new("skyward-spine-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "skywardspine";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(PERM_CR, st::CR, 0.06 + 0.02 * r, false, false, -1));
        s.chars[ci].mods.push(Mod::stat(PERM_SPD, st::ATKSPD, 0.12, false, false, -1));
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
        if s.rng.float64() > 0.5 {
            return;
        }
        s.add_status(ci, ICD, 120, true);
        let r = s.chars[ci].refine as f64;
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 0.25 + 0.15 * r);
        ai.abil = "Skyward Spine Proc";
        ai.pattern = s.circle_on_target(2.0);
        s.queue_attack(ai, 0, 0, 1);
    }
}
