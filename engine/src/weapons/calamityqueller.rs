//! Calamity Queller, from gcsim's internal/weapons/spear/calamity.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    stacks: i32,
    /// gcsim's lastBuffGain: the frame of the last skill that refreshed
    /// Consummation.
    last_gain: i32,
}

const DMG: ModKey = ModKey::new("calamity-dmg");
const BUFF: ModKey = ModKey::new("calamity-consummation");

const TASK_STACK: u8 = 0;
/// arg: the frame of the skill that queued it.
const TASK_EXPIRY: u8 = 1;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "calamityqueller";
    type State = State;
    fn new_state() -> State {
        State { stacks: 0, last_gain: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(DMG, ModKind::Stat, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::Skill, o);
    }

    /// A skill by the holder on field: Consummation for 20 s (hitlag). A
    /// stack a second after it, and every second after that until six
    /// (each skill starts its own chain, as gcsim's does); the stacks end
    /// 20 s after the last skill (a task, which hitlag delays, not the
    /// mod's expiry).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Skill = *e else { return };
        if s.player.active as usize != ci {
            return;
        }
        let f = s.f;
        s.weapon_st_mut::<State>(ci).last_gain = f;
        let c = ci as u8;
        s.queue_char_task(ci, Task::Weapon { c, id: TASK_EXPIRY, arg: f }, 1200);
        s.queue_char_task(ci, Task::Weapon { c, id: TASK_STACK, arg: 0 }, 60);
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(c), st::NONE, false, true, -1), 1200);
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        let st = s.weapon_st_mut::<State>(ci);
        match id {
            TASK_STACK => {
                if st.stacks < 6 {
                    st.stacks += 1;
                    if st.stacks != 6 {
                        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_STACK, arg: 0 }, 60);
                    }
                }
            }
            TASK_EXPIRY => {
                if st.last_gain == arg {
                    st.stacks = 0;
                }
            }
            _ => unreachable!(),
        }
    }

    /// Elemental DMG +12 to 24%; Consummation's ATK +3.2 to 6.4% a stack,
    /// doubled while the holder is off field.
    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        if m.key == DMG {
            let dmg = 0.09 + r * 0.03;
            for stat in [st::PYROP, st::HYDROP, st::CRYOP, st::ELECTROP, st::ANEMOP, st::GEOP, st::DENDROP] {
                out[stat] += dmg;
            }
            return true;
        }
        let mut atk = (0.024 + r * 0.008) * s.weapon_st::<State>(ci).stacks as f64;
        if s.player.active as usize != ci {
            atk *= 2.0;
        }
        out[st::ATKP] += atk;
        true
    }
}
