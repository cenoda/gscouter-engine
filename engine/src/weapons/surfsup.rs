//! Surf's Up, from gcsim's internal/weapons/catalyst/surfsup: max HP +20
//! to 40%. A skill on field, once every 15 s (hitlag), sets 4 Scorching
//! Summer stacks and an attack mod for 14 s (hitlag): normal attack DMG
//! +12 to 24% per stack (at most 4 counted). While it lasts, a normal
//! attack hit by the holder on field loses a stack, once every 1.5 s
//! (hitlag), and a vaporize the holder triggers on field gains one, once
//! every 1.5 s (hitlag), up to 5 so that it carries over a hit; at 5 it
//! goes back to 4 after 0.5 s (on the holder's queue).

use crate::content::{Event, Owner, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    stacks: i32,
}

const HP: ModKey = ModKey::new("surfs-up-hp%");
const ICD: ModKey = ModKey::new("surfs-up-icd");
const BUFF: ModKey = ModKey::new("surfs-up-buff");
const LOSE_ICD: ModKey = ModKey::new("surfs-up-stack-loss-icd");
const GAIN_ICD: ModKey = ModKey::new("surfs-up-stack-gain-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "surfsup";
    type State = State;
    fn new_state() -> State {
        State { stacks: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(HP, st::HPP, 0.15 + r * 0.05, false, false, -1));
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::Reaction, o);
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::Normal {
            let per = 0.09 + s.chars[ci].refine as f64 * 0.03;
            ev.stats[st::DMGP] += per * s.weapon_st::<State>(ci).stacks.min(4) as f64;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Skill => {
                if s.player.active as usize != ci || s.mod_active(ci, ICD) {
                    return;
                }
                s.add_status(ci, ICD, 15 * 60, true);
                s.weapon_st_mut::<State>(ci).stacks = 4;
                s.add_mod(ci, Mod::owned(BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 14 * 60);
            }
            Event::Reaction(Reaction::Vaporize, ev) => {
                if !ev.on_enemy() || ev.info.actor as usize != ci || s.player.active as usize != ci {
                    return;
                }
                if !s.mod_active(ci, BUFF) || s.mod_active(ci, GAIN_ICD) {
                    return;
                }
                let st = s.weapon_st_mut::<State>(ci);
                if st.stacks < 5 {
                    st.stacks += 1;
                }
                if st.stacks == 5 {
                    s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: 0, arg: 0 }, 30);
                }
                s.add_status(ci, GAIN_ICD, 90, true);
            }
            Event::EnemyDamage(ev) => {
                if !ev.on_enemy() || ev.info.actor as usize != ci || ev.info.tag != AttackTag::Normal || s.player.active as usize != ci {
                    return;
                }
                if !s.mod_active(ci, BUFF) || s.mod_active(ci, LOSE_ICD) {
                    return;
                }
                let st = s.weapon_st_mut::<State>(ci);
                if st.stacks > 0 {
                    st.stacks -= 1;
                }
                s.add_status(ci, LOSE_ICD, 90, true);
            }
            _ => {}
        }
    }

    /// Back to 4 stacks after carrying a fifth.
    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        s.weapon_st_mut::<State>(ci).stacks = 4;
    }
}
