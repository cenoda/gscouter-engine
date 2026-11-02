//! Crescent Pike, from gcsim's internal/weapons/spear/crescent.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("crescent-pike-buff");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "crescentpike";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::ParticleReceived, o);
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // a particle taken with the holder on field: 5 s (hitlag)
            Event::ParticleReceived => {
                if s.player.active as usize == ci {
                    s.add_status(ci, BUFF, 300, true);
                }
            }
            // while it lasts, each damaging normal or charged hit by the
            // holder adds 20 to 40% of ATK as physical damage, on the
            // target hit, a frame later
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) || ev.damage == 0.0 {
                    return;
                }
                if !s.mod_active(ci, BUFF) {
                    return;
                }
                let mult = 0.15 + s.chars[ci].refine as f64 * 0.05;
                let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, mult);
                ai.abil = "Crescent Pike Proc";
                ai.pattern = Pattern::Single;
                s.queue_attack(ai, 0, 0, 1);
            }
            _ => {}
        }
    }
}
