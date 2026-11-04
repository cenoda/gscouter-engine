//! Tulaytullah's Remembrance, from gcsim's
//! internal/weapons/catalyst/tulaytullahsremembrance: ATK SPD +10 to 20%
//! while the player is in the normal attack state. A skill on field resets
//! the stacks, gains one every second (on the holder's queue, up to 10)
//! and adds an attack mod for 14 s (hitlag): normal attack DMG +4.8 to
//! 9.6% per stack. A normal attack hit by the holder gains two stacks
//! (up to 10), once every 0.3 s (hitlag), with or without the mod.
//! Swapping out while the mod is on loses the stacks, stops the timer and
//! removes the mod.

use crate::content::{Event, Owner, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    stacks: i32,
    /// The frame of the skill whose timer runs (-1 for none).
    src: i32,
}

const ATKSPD: ModKey = ModKey::new("tulaytullahsremembrance-atkspd");
const ICD: ModKey = ModKey::new("tulaytullahsremembrance-icd");
const BUFF: ModKey = ModKey::new("tulaytullahsremembrance-na-dmg");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "tulaytullahsremembrance";
    type State = State;
    fn new_state() -> State {
        State { stacks: 0, src: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(ATKSPD, ModKind::Stat, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::EnemyDamage, o);
        s.subscribe(EventKind::Swap, o);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if s.current_state() != AnimState::Normal {
            return false;
        }
        out[st::ATKSPD] += 0.075 + s.chars[ci].refine as f64 * 0.025;
        true
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::Normal {
            let inc = 0.036 + s.chars[ci].refine as f64 * 0.012;
            ev.stats[st::DMGP] += inc * s.weapon_st::<State>(ci).stacks as f64;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Skill => {
                if s.player.active as usize != ci {
                    return;
                }
                let f = s.f;
                let st = s.weapon_st_mut::<State>(ci);
                st.stacks = 0;
                st.src = f;
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: 0, arg: f }, 60);
                s.add_mod(ci, Mod::owned(BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 14 * 60);
            }
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize != ci || ev.info.tag != AttackTag::Normal || s.mod_active(ci, ICD) {
                    return;
                }
                s.add_status(ci, ICD, 18, true);
                let st = s.weapon_st_mut::<State>(ci);
                st.stacks = (st.stacks + 2).min(10);
            }
            Event::Swap { prev, .. } => {
                if prev != ci || !s.mod_active(ci, BUFF) {
                    return;
                }
                let st = s.weapon_st_mut::<State>(ci);
                st.stacks = 0;
                st.src = -1;
                s.delete_mod(ci, BUFF);
            }
            _ => {}
        }
    }

    /// A stack a second from the skill whose frame is `src`.
    fn task(s: &mut Sim, ci: usize, _: u8, src: i32) {
        let st = s.weapon_st_mut::<State>(ci);
        if st.stacks > 9 || src != st.src {
            return;
        }
        st.stacks += 1;
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: 0, arg: src }, 60);
    }
}
