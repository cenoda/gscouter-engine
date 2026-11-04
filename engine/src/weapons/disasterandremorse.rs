//! Disaster and Remorse, from gcsim's internal/weapons/spear/disasterandremorse.

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

const TASK_DELETE: u8 = 0;

fn is_na_ca(t: AttackTag) -> bool {
    matches!(t, AttackTag::Normal | AttackTag::Extra)
}

fn is_skill_burst(t: AttackTag) -> bool {
    matches!(t, AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst)
}

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

    /// Unforgivable: normal and charged attack DMG +40 to 80%;
    /// Irreparable: skill and burst DMG the same. Each ×1.75 with two
    /// Hexerei characters in the team.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        let applies = if m.key == UNFORGIVABLE { is_na_ca(ev.info.tag) } else { is_skill_burst(ev.info.tag) };
        if !applies {
            return;
        }
        let bonus = 0.40 + 0.10 * (s.chars[ci].refine - 1) as f64;
        ev.stats[st::DMGP] += if s.hexerei_count() >= 2 { bonus * 1.75 } else { bonus };
    }

    /// The Path of Conflict's end, 17 s (hitlag) after it began.
    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        s.delete_mod(ci, UNFORGIVABLE);
        s.delete_mod(ci, IRREPARABLE);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // a skill on field, once every 18 s (hitlag): the Path of
            // Conflict for 17 s (hitlag), both buffs for 3 s (hitlag)
            Event::Skill => {
                if s.player.active as usize != ci || s.mod_active(ci, PROC_ICD) {
                    return;
                }
                s.add_status(ci, PROC_ICD, 18 * 60, true);
                s.add_status(ci, PATH, 17 * 60, true);
                let o = Owner::Weapon(ci as u8);
                s.add_mod(ci, Mod::owned(UNFORGIVABLE, ModKind::Attack, o, st::NONE, false, true, -1), 3 * 60);
                s.add_mod(ci, Mod::owned(IRREPARABLE, ModKind::Attack, o, st::NONE, false, true, -1), 3 * 60);
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_DELETE, arg: 0 }, 17 * 60);
            }
            // during the Path: a normal or charged hit on the enemy extends
            // Irreparable by 1 s, a skill or burst hit Unforgivable, each
            // once every 0.1 s (hitlag)
            Event::EnemyHit(ev) => {
                if ev.info.actor as usize != ci || !s.mod_active(ci, PATH) {
                    return;
                }
                if is_na_ca(ev.info.tag) {
                    if s.mod_active(ci, NA_CA_ICD) {
                        return;
                    }
                    s.add_status(ci, NA_CA_ICD, 6, true);
                    s.extend_mod(ci, IRREPARABLE, 60);
                } else if is_skill_burst(ev.info.tag) {
                    if s.mod_active(ci, SKILL_BURST_ICD) {
                        return;
                    }
                    s.add_status(ci, SKILL_BURST_ICD, 6, true);
                    s.extend_mod(ci, UNFORGIVABLE, 60);
                }
            }
            // swapping out ends it all
            Event::Swap { prev, .. } => {
                if prev != ci {
                    return;
                }
                s.delete_mod(ci, UNFORGIVABLE);
                s.delete_mod(ci, IRREPARABLE);
                s.delete_mod(ci, PATH);
            }
            _ => {}
        }
    }
}
