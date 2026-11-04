//! Debate Club, from gcsim's internal/weapons/claymore/debateclub.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("debate-club-effect");
const ICD: ModKey = ModKey::new("debate-club-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "debateclub";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Skill => {
                if s.player.active as usize == ci {
                    // a skill resets the proc's cooldown
                    s.delete_mod(ci, ICD);
                    s.add_status(ci, BUFF, 900, true);
                }
            }
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize != ci
                    || s.player.active as usize != ci
                    || !s.mod_active(ci, BUFF)
                    || s.mod_active(ci, ICD)
                    || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra)
                {
                    return;
                }
                s.add_status(ci, ICD, 180, true);
                let mult = 0.45 + s.chars[ci].refine as f64 * 0.15;
                let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, mult);
                ai.abil = "Debate Club Proc";
                ai.pattern = s.circle_on_target(3.0);
                s.queue_attack(ai, 0, 0, 1);
            }
            _ => {}
        }
    }
}
