//! The Daybreak Chronicles, from gcsim's internal/weapons/bow/daybreakchronicles.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    na_stacks: i32,
    na_src: i32,
    skill_stacks: i32,
    skill_src: i32,
    burst_stacks: i32,
    burst_src: i32,
}

const BUFF: ModKey = ModKey::new("stirring-dawn-breeze");

const TASK_DEC_NA: u8 = 0;
const TASK_DEC_SKILL: u8 = 1;
const TASK_DEC_BURST: u8 = 2;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "thedaybreakchronicles";
    type State = State;
    fn new_state() -> State {
        State {
            na_stacks: 6,
            na_src: 0,
            skill_stacks: 6,
            skill_src: 0,
            burst_stacks: 6,
            burst_src: 0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(BUFF, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyHit, o);
        // the sources start at 0, the tasks carry the frame (0 at init)
        let f = s.f;
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_DEC_NA, arg: f }, 60);
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_DEC_SKILL, arg: f }, 60);
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_DEC_BURST, arg: f }, 60);
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key != BUFF {
            return;
        }
        let r = s.chars[ci].refine as f64;
        let per_stack = 0.075 + 0.025 * r;
        let st = s.weapon_st::<State>(ci);
        let stacks = match ev.info.tag {
            AttackTag::Normal => st.na_stacks,
            AttackTag::ElementalArt | AttackTag::ElementalArtHold => st.skill_stacks,
            AttackTag::ElementalBurst => st.burst_stacks,
            _ => return,
        };
        ev.stats[st::DMGP] += per_stack * stacks as f64;
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if ev.info.actor as usize != ci {
            return;
        }
        let f = s.f;
        let add = if s.hexerei_count() >= 2 { 2 } else { 1 };
        match ev.info.tag {
            AttackTag::Normal => {
                let st = s.weapon_st_mut::<State>(ci);
                st.na_src = f;
                st.na_stacks = (st.na_stacks + add).min(6);
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_DEC_NA, arg: f }, 60);
            }
            AttackTag::ElementalArt | AttackTag::ElementalArtHold => {
                let st = s.weapon_st_mut::<State>(ci);
                st.skill_src = f;
                st.skill_stacks = (st.skill_stacks + add).min(6);
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_DEC_SKILL, arg: f }, 60);
            }
            AttackTag::ElementalBurst => {
                let st = s.weapon_st_mut::<State>(ci);
                st.burst_src = f;
                st.burst_stacks = (st.burst_stacks + add).min(6);
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_DEC_BURST, arg: f }, 60);
            }
            _ => {}
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, src: i32) {
        match id {
            TASK_DEC_NA => {
                let st = s.weapon_st_mut::<State>(ci);
                if st.na_stacks == 0 || st.na_src != src {
                    return;
                }
                st.na_stacks -= 1;
                if st.na_stacks != 0 {
                    s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_DEC_NA, arg: src }, 60);
                }
            }
            TASK_DEC_SKILL => {
                let st = s.weapon_st_mut::<State>(ci);
                if st.skill_stacks == 0 || st.skill_src != src {
                    return;
                }
                st.skill_stacks -= 1;
                if st.skill_stacks != 0 {
                    s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_DEC_SKILL, arg: src }, 60);
                }
            }
            TASK_DEC_BURST => {
                let st = s.weapon_st_mut::<State>(ci);
                if st.burst_stacks == 0 || st.burst_src != src {
                    return;
                }
                st.burst_stacks -= 1;
                if st.burst_stacks != 0 {
                    s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_DEC_BURST, arg: src }, 60);
                }
            }
            _ => {}
        }
    }
}
