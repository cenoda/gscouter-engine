//! Emblem of Severed Fate, from gcsim's internal/artifacts/emblem.

use crate::content::SetImpl;
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("emblem-2pc");
const FOUR: ModKey = ModKey::new("emblem-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "emblemofseveredfate";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::ER, 0.20, false, false, -1));
        if count >= 4 {
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, false, -1));
        }
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::ElementalBurst {
            let er = s.stat_ex(ci, st::ER, true);
            ev.stats[st::DMGP] += (0.25 * er).min(0.75);
        }
    }
}
