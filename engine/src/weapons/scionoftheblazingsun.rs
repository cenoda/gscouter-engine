//! Scion of the Blazing Sun, from gcsim's
//! internal/weapons/bow/scionoftheblazingsun.
//!
//! gcsim applies Heartsearer in the Sunfire Arrow's attack callback, which
//! runs right after the arrow's OnEnemyDamage; the engine applies it on
//! that event (weapons have no attack callbacks), which no hook between
//! the two reads.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("scion");
const ICD: ModKey = ModKey::new("scion-icd");
const DEBUFF: ModKey = ModKey::new("scion-heartsearer");
const ABIL: &str = "Sunfire Arrow";

impl WeaponImpl for Weapon {
    const KEY: &'static str = "scionoftheblazingsun";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
    }

    /// Charged attack DMG +28 to 56% against an enemy under Heartsearer.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::Extra && s.enemy_status_active(DEBUFF) {
            ev.stats[st::DMGP] += 0.21 + 0.07 * s.chars[ci].refine as f64;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci {
            return;
        }
        // the arrow hit: Heartsearer on the enemy for 10 s (hitlag)
        if ev.info.tag == AttackTag::Weapon && ev.info.abil == ABIL {
            s.enemy_add_status(DEBUFF, 10 * 60, true);
            return;
        }
        // a charged hit by the holder on field, once every 10 s (hitlag):
        // a Sunfire Arrow of 60 to 120% of ATK, physical, in a circle of
        // radius 3.5 on the enemy, snapshot and hit 15 frames later
        if s.player.active as usize != ci || s.mod_active(ci, ICD) || ev.info.tag != AttackTag::Extra {
            return;
        }
        let mult = 0.45 + s.chars[ci].refine as f64 * 0.15;
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, mult);
        ai.abil = ABIL;
        ai.pattern = s.circle_on_target(3.5);
        s.queue_attack(ai, 0, 15, 15);
        s.add_status(ci, ICD, 10 * 60, true);
    }
}
