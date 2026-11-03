//! Crane's Echoing Call, from gcsim's internal/weapons/catalyst/cranesechoingcall.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("crane-dmg%");
const ENERGY_ICD: ModKey = ModKey::new("crane-energy-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "cranesechoingcall";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// Plunging attack DMG +28 to 80% (on every party member).
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::Plunge {
            ev.stats[st::DMGP] += 0.15 + s.chars[ci].refine as f64 * 0.13;
        }
    }

    /// A plunge hit: the holder's gives every party member the buff for
    /// 20 s (hitlag); anyone's restores 2.5 to 3.5 energy to the holder,
    /// once every 0.7 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.tag != AttackTag::Plunge {
            return;
        }
        if ev.info.actor as usize == ci {
            let o = Owner::Weapon(ci as u8);
            for j in 0..s.chars.len() {
                s.add_mod(j, Mod::owned(BUFF, ModKind::Attack, o, st::NONE, false, true, -1), 20 * 60);
            }
        }
        if s.mod_active(ci, ENERGY_ICD) {
            return;
        }
        s.add_status(ci, ENERGY_ICD, 42, true);
        let amt = 2.25 + s.chars[ci].refine as f64 * 0.25;
        s.add_energy(ci, amt);
    }
}
