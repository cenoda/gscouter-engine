//! Skyward Harp, from gcsim's internal/weapons/bow/skyward.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const CD: ModKey = ModKey::new("skyward harp");
const ICD: ModKey = ModKey::new("skyward-harp-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "skywardharp";
    type State = State;
    fn new_state() -> State {
        State
    }

    /// CRIT DMG +20 to 40%.
    fn init(s: &mut Sim, ci: usize) {
        let cd = 0.15 + s.chars[ci].refine as f64 * 0.05;
        s.chars[ci].mods.push(Mod::stat(CD, st::CD, cd, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// A hit by the holder on field, outside the cooldown (4.5 s less 0.5 s
    /// a refinement, hitlag), has a 60 to 100% chance (drawn before the
    /// damage is checked, as gcsim does) to deal 125% physical damage in a
    /// circle of radius 3 on the target, a frame later.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        let r = s.chars[ci].refine;
        let prob = 0.5 + 0.1 * r as f64;
        if s.rng.float64() > prob {
            return;
        }
        if ev.damage == 0.0 {
            return;
        }
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 1.25);
        ai.abil = "Skyward Harp Proc";
        ai.pattern = s.circle_on_target(3.0);
        s.queue_attack(ai, 0, 0, 1);
        s.add_status(ci, ICD, 270 - 30 * r, true);
    }
}
