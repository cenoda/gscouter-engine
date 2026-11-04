//! Dawning Frost, from gcsim's internal/weapons/catalyst/dawningfrost.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const CA: ModKey = ModKey::new("dawning-frost-ca");
const SKILL: ModKey = ModKey::new("dawning-frost-skill");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "dawningfrost";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// Damage by the holder on field: from a charged attack EM +72 to 144,
    /// from a skill EM +48 to 96, each for 10 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        let r = s.chars[ci].refine as f64;
        match ev.info.tag {
            AttackTag::Extra => s.add_mod(ci, Mod::stat(CA, st::EM, 54.0 + r * 18.0, false, true, -1), 10 * 60),
            AttackTag::ElementalArt | AttackTag::ElementalArtHold => s.add_mod(ci, Mod::stat(SKILL, st::EM, 36.0 + r * 12.0, false, true, -1), 10 * 60),
            _ => {}
        }
    }
}
