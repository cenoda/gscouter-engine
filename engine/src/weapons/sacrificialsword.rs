//! Sacrificial Sword, from gcsim's internal/weapons/common/sacrificial.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("sacrificial-cd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "sacrificialsword";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci
            || s.player.active as usize != ci
            || ev.info.tag != AttackTag::ElementalArt
            || s.mod_active(ci, ICD)
            || s.cooldown(ci, Action::Skill) == 0
            || ev.damage == 0.0
        {
            return;
        }
        let r = s.chars[ci].refine;
        let prob = 0.3 + r as f64 * 0.1;
        let cd = if r >= 4 { (19 - (r - 4) * 3) * 60 } else { (34 - r * 4) * 60 };
        if s.rng.float64() < prob {
            s.reset_cd(ci, Action::Skill);
            s.add_status(ci, ICD, cd, true);
        }
    }
}
