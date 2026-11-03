//! Fang of the Mountain King, from gcsim's internal/weapons/claymore/fangofthemountainking.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
/// gcsim's stacks.MultipleRefreshNoRemove of 6: each stack keeps the frame
/// it was added on and goes 6 s later (on the holder's queue, so hitlag
/// delays it); a seventh replaces the oldest. `serial` tells a slot's
/// removal task whether the stack it was queued for is still the one there.
pub struct State {
    src: [i32; 6],
    serial: [i32; 6],
}

const CANOPY: ModKey = ModKey::new("canopy-favor");
const SKILL_ICD: ModKey = ModKey::new("fotmk-skill-icd");
const REACT_ICD: ModKey = ModKey::new("fotmk-react-icd");
const NONE: i32 = i32::MIN;

impl State {
    fn count(&self) -> i32 {
        self.src.iter().filter(|&&f| f != NONE).count() as i32
    }
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "fangofthemountainking";
    type State = State;
    fn new_state() -> State {
        State { src: [NONE; 6], serial: [0; 6] }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Reaction, o);
        s.subscribe(EventKind::EnemyDamage, o);
    }

    /// Skill and burst DMG +10 to 20% per stack.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst) {
            return;
        }
        let per = 0.10 + (s.chars[ci].refine - 1) as f64 * 0.025;
        ev.stats[st::DMGP] += per * s.weapon_st::<State>(ci).count() as f64;
    }

    /// The stack added in slot arg & 7 runs out, unless replaced since.
    fn task(s: &mut Sim, ci: usize, _: u8, arg: i32) {
        let st = s.weapon_st_mut::<State>(ci);
        let idx = (arg & 7) as usize;
        if st.serial[idx] == arg >> 3 {
            st.src[idx] = NONE;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // burning on the enemy or burgeon by anyone (gcsim checks no
            // attacker), once every 2 s (hitlag): three stacks
            Event::Reaction(r, ev) => {
                match r {
                    Reaction::Burning if ev.on_enemy() => {}
                    Reaction::Burgeon => {}
                    _ => return,
                }
                if s.mod_active(ci, REACT_ICD) {
                    return;
                }
                s.add_status(ci, REACT_ICD, 2 * 60, true);
                add_stacks(s, ci, 3);
            }
            // a skill hit on the enemy by the holder on field, once every
            // 0.5 s: a stack
            Event::EnemyDamage(ev) => {
                if !ev.on_enemy() || ev.info.actor as usize != ci || s.player.active as usize != ci {
                    return;
                }
                if !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) || s.mod_active(ci, SKILL_ICD) {
                    return;
                }
                s.add_status(ci, SKILL_ICD, 30, false);
                add_stacks(s, ci, 1);
            }
            _ => {}
        }
    }
}

/// Stacks for 6 s each, and the buff for 6 s (hitlag).
fn add_stacks(s: &mut Sim, ci: usize, n: i32) {
    for _ in 0..n {
        let f = s.f;
        let st = s.weapon_st_mut::<State>(ci);
        let mut idx = 0;
        for i in 0..6 {
            if st.src[i] == NONE {
                idx = i;
                break;
            }
            if st.src[i] < st.src[idx] {
                idx = i;
            }
        }
        st.src[idx] = f;
        st.serial[idx] += 1;
        let arg = st.serial[idx] << 3 | idx as i32;
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: 0, arg }, 6 * 60);
    }
    s.add_mod(ci, Mod::owned(CANOPY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 6 * 60);
}
