//! Noblesse Oblige, from gcsim's internal/artifacts/noblesse.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("nob-2pc");
const FOUR: ModKey = ModKey::new("nob-4pc");

/// The characters gcsim registers with hacks.RegisterNOSpecialChar: on
/// them the holder's own buff comes a frame late.
const SPECIAL: &[&str] = &["aloy", "beidou", "aetheranemo", "lumineanemo", "aethercryo", "luminecryo", "ningguang", "ganyu", "sayu"];

const TASK_BUFF: u8 = 0;

impl SetImpl for Set {
    const KEY: &'static str = "noblesseoblige";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::owned(TWO, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, false, -1));
        }
        if count >= 4 {
            s.subscribe(EventKind::Burst, Owner::Set(ci as u8, si as u8));
        }
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::ElementalBurst {
            ev.stats[st::DMGP] += 0.20;
        }
    }

    /// A burst by the holder, on field: ATK +20% to the party for 12 s.
    fn event(s: &mut Sim, ci: usize, si: usize, _: &Event) {
        if s.player.active as usize != ci {
            return;
        }
        let special = SPECIAL.contains(&crate::gcsl::char_name(s.chars[ci].key));
        for i in 0..s.chars.len() {
            let delay = if i == ci && special { 1 } else { 0 };
            s.queue_char_task(i, Task::Set { c: ci as u8, si: si as u8, id: TASK_BUFF, arg: i as i32 }, delay);
        }
    }

    fn task(s: &mut Sim, _: usize, _: usize, _: u8, arg: i32) {
        s.add_stat_mod(arg as usize, FOUR, st::ATKP, 0.2, 720, true);
    }
}
