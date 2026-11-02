//! Heart of Depth, from gcsim's internal/artifacts/heartofdepth.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("hod-2pc");
const FOUR: ModKey = ModKey::new("hod-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "heartofdepth";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::HYDROP, 0.15, false, false, -1));
        }
        if count >= 4 {
            s.subscribe(EventKind::Skill, Owner::Set(ci as u8, si as u8));
        }
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            ev.stats[st::DMGP] += 0.30;
        }
    }

    /// A skill on field: normal and charged attack DMG +30% for 15 s
    /// (hitlag).
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        if let Event::Skill = *e {
            if s.player.active as usize == ci {
                s.add_mod(ci, Mod::owned(FOUR, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, true, -1), 900);
            }
        }
    }
}
