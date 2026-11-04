//! Martial Artist, from gcsim's internal/artifacts/martialartist.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("martialartist-2pc");
const FOUR: ModKey = ModKey::new("martialartist-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "martialartist";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::owned(TWO, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, false, -1));
        }
        if count >= 4 {
            s.subscribe(EventKind::Skill, Owner::Set(ci as u8, si as u8));
        }
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            ev.stats[st::DMGP] += if m.key == TWO { 0.15 } else { 0.25 };
        }
    }

    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        if let Event::Skill = *e {
            if s.player.active as usize == ci {
                s.add_mod(ci, Mod::owned(FOUR, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, true, -1), 480);
            }
        }
    }
}
