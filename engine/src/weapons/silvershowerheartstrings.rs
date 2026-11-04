//! Silvershower Heartstrings, from gcsim's internal/weapons/bow/heartstrings.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    prev_stacks: i32,
}

const SKILL_KEY: ModKey = ModKey::new("heartstrings-skill");
const BOND_KEY: ModKey = ModKey::new("heartstrings-bond");
const HEAL_KEY: ModKey = ModKey::new("heartstrings-healing");
const HP_BUFF: ModKey = ModKey::new("heartstrings");
const BURST_CR: ModKey = ModKey::new("heartstrings-cr");
const BURST_CR_CANCEL: ModKey = ModKey::new("heartstrings-cr-cancel");

const TASK_UPDATE: u8 = 0;
/// The stacks' statuses, by the index the update task carries.
const STACK_KEYS: [ModKey; 3] = [SKILL_KEY, BOND_KEY, HEAL_KEY];

impl WeaponImpl for Weapon {
    const KEY: &'static str = "silvershowerheartstrings";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(HP_BUFF, ModKind::Stat, o, st::HPP, false, false, -1));
        s.chars[ci].mods.push(Mod::owned(BURST_CR, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::HpDebt, o);
        s.subscribe(EventKind::Heal, o);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let stacks = Self::stacks(s, ci);
        if stacks == 0 {
            return false;
        }
        let r = s.chars[ci].refine as f64;
        let mut hp = (0.09 + 0.03 * r) * stacks as f64;
        if stacks >= 3 {
            hp += 0.03 + 0.01 * r;
        }
        out[st::HPP] += hp;
        true
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag != AttackTag::ElementalBurst {
            return;
        }
        let stacks = Self::stacks(s, ci);
        if stacks < 3 && !s.mod_active(ci, BURST_CR_CANCEL) {
            return;
        }
        let r = s.chars[ci].refine as f64;
        ev.stats[st::CR] += 0.21 + 0.07 * r;
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Skill => {
                if s.player.active as usize == ci {
                    Self::add_stack(s, ci, 0, 25 * 60);
                }
            }
            // gcsim adds the stack unless the Bond of Life went down (an
            // unchanged one, at its cap, counts)
            Event::HpDebt { on, change } => {
                if on == ci && change <= 0.0 {
                    Self::add_stack(s, ci, 1, 25 * 60);
                }
            }
            Event::Heal { caller, .. } => {
                if caller == ci {
                    Self::add_stack(s, ci, 2, 20 * 60);
                }
            }
            _ => {}
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, which: i32) {
        // gcsim's AddStack task: updates only when that stack's status is
        // no longer active
        if id == TASK_UPDATE && !s.mod_active(ci, STACK_KEYS[which as usize]) {
            Self::on_update_stacks(s, ci);
        }
    }
}

impl Weapon {
    /// gcsim's AddStack.
    fn add_stack(s: &mut Sim, ci: usize, which: i32, dur: i32) {
        s.add_status(ci, STACK_KEYS[which as usize], dur, true);
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_UPDATE, arg: which }, dur + 1);
        Self::on_update_stacks(s, ci);
    }

    fn stacks(s: &Sim, ci: usize) -> i32 {
        let mut count = 0;
        if s.mod_active(ci, SKILL_KEY) {
            count += 1;
        }
        if s.mod_active(ci, BOND_KEY) {
            count += 1;
        }
        if s.mod_active(ci, HEAL_KEY) {
            count += 1;
        }
        count
    }

    fn on_update_stacks(s: &mut Sim, ci: usize) {
        let stacks = Self::stacks(s, ci);
        let st = s.weapon_st_mut::<State>(ci);
        if st.prev_stacks == 3 && stacks < 3 {
            s.add_status(ci, BURST_CR_CANCEL, 4 * 60, true);
        }
        s.weapon_st_mut::<State>(ci).prev_stacks = stacks;
    }
}
