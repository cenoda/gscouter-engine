//! Gambler, from gcsim's internal/artifacts/gambler.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("gambler-2pc");
const ICD: ModKey = ModKey::new("gambler-4pc-icd");

impl SetImpl for Set {
    const KEY: &'static str = "gambler";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::owned(TWO, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, false, -1));
        }
        if count >= 4 {
            s.subscribe(EventKind::TargetDied, Owner::Set(ci as u8, si as u8));
        }
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == TWO && ev.info.tag == AttackTag::ElementalArt {
            ev.stats[st::DMGP] += 0.20;
        }
    }

    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let Event::TargetDied(ev) = *e else { return };
        if s.mod_active(ci, ICD) || ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        s.reset_cd(ci, Action::Skill);
        s.add_status(ci, ICD, 900, true);
    }
}
