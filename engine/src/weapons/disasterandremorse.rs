//! Disaster and Remorse, from gcsim's internal/weapons/spear/
//! disasterandremorse, refinements 1 to 5.
//!
//! As in gcsim: the wielder's skill on field (once every 18 s) gives Path
//! of Conflict for 17 s and Unforgivable (normal and charged attack DMG)
//! and Irreparable (skill and burst DMG) for 3 s each, 40% to 80%, 75%
//! more with two Hexerei characters in the team (whether or not the
//! wielder is one). While Path of Conflict lasts, the wielder's normal and
//! charged attack hits extend Irreparable by 1 s and skill and burst hits
//! extend Unforgivable by 1 s, each once every 0.1 s. Both end 17 s after
//! the skill or when the wielder leaves the field.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const PATH: ModKey = ModKey::new("path-of-conflict");
const UNFORGIVABLE: ModKey = ModKey::new("unforgivable");
const IRREPARABLE: ModKey = ModKey::new("irreparable");
const PROC_ICD: ModKey = ModKey::new("disaster-remorse-proc");
const NA_CA_ICD: ModKey = ModKey::new("disaster-remorse-na-ca-icd");
const SKILL_BURST_ICD: ModKey = ModKey::new("disaster-remorse-skill-burst-icd");

const TASK_END: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "disasterandremorse";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::EnemyHit, o);
        s.subscribe(EventKind::Swap, o);
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        let applies = if m.key == UNFORGIVABLE {
            matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra)
        } else {
            matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst)
        };
        if !applies {
            return;
        }
        let bonus = 0.4 + 0.1 * (s.chars[ci].refine - 1) as f64;
        ev.stats[st::DMGP] += if s.hexerei_count() >= 2 { bonus * 1.75 } else { bonus };
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Skill => {
                if s.player.active as usize != ci || s.mod_active(ci, PROC_ICD) {
                    return;
                }
                let o = Owner::Weapon(ci as u8);
                s.add_status(ci, PROC_ICD, 18 * 60, true);
                s.add_status(ci, PATH, 17 * 60, true);
                s.add_mod(ci, Mod::owned(UNFORGIVABLE, ModKind::Attack, o, st::NONE, false, true, -1), 3 * 60);
                s.add_mod(ci, Mod::owned(IRREPARABLE, ModKind::Attack, o, st::NONE, false, true, -1), 3 * 60);
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_END, arg: 0 }, 17 * 60);
            }
            Event::EnemyHit(ev) => {
                if ev.info.actor as usize != ci || !s.mod_active(ci, PATH) {
                    return;
                }
                match ev.info.tag {
                    AttackTag::Normal | AttackTag::Extra => {
                        if s.mod_active(ci, NA_CA_ICD) {
                            return;
                        }
                        s.add_status(ci, NA_CA_ICD, 6, true);
                        s.extend_mod(ci, IRREPARABLE, 60);
                    }
                    AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst => {
                        if s.mod_active(ci, SKILL_BURST_ICD) {
                            return;
                        }
                        s.add_status(ci, SKILL_BURST_ICD, 6, true);
                        s.extend_mod(ci, UNFORGIVABLE, 60);
                    }
                    _ => {}
                }
            }
            Event::Swap { prev, .. } => {
                if prev == ci {
                    s.delete_mod(ci, UNFORGIVABLE);
                    s.delete_mod(ci, IRREPARABLE);
                    s.delete_mod(ci, PATH);
                }
            }
            _ => {}
        }
    }

    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        s.delete_mod(ci, UNFORGIVABLE);
        s.delete_mod(ci, IRREPARABLE);
    }
}
