//! Shimenawa's Reminiscence, from gcsim's internal/artifacts/shimenawa.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("shim-2pc");
const FOUR: ModKey = ModKey::new("shim-4pc");
const FOUR_ICD: ModKey = ModKey::new("shim-4pc-icd");

impl SetImpl for Set {
    const KEY: &'static str = "shimenawasreminiscence";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        if count >= 4 {
            s.subscribe(EventKind::Skill, Owner::Set(ci as u8, si as u8));
        }
    }

    fn event(s: &mut Sim, ci: usize, si: usize, _: &Event) {
        if s.player.active as usize != ci || s.chars[ci].energy < 15.0 || s.mod_active(ci, FOUR_ICD) {
            return;
        }
        s.add_status(ci, FOUR_ICD, 600, true);
        s.add_task(Task::AddEnergy { c: ci as u8, amt: -15.0 }, 10);
        s.add_mod(ci, Mod::owned(FOUR, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, true, -1), 600);
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            ev.stats[st::DMGP] += 0.5;
        }
    }
}
