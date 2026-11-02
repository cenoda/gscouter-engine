//! Blizzard Strayer, from gcsim's internal/artifacts/blizzard.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("bs-2pc");
const FOUR: ModKey = ModKey::new("bs-4pc");

/// CRIT Rate against the enemy: +40% when frozen, else +20% with cryo
/// (gcsim checks frozen first, so that cryo beside it does not count).
fn bonus(s: &Sim) -> f64 {
    let a = &s.enemy.aura;
    if a.contains(Element::Frozen) {
        0.4
    } else if a.contains(Element::Cryo) {
        0.2
    } else {
        0.0
    }
}

impl SetImpl for Set {
    const KEY: &'static str = "blizzardstrayer";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::CRYOP, 0.15, false, false, -1));
        if count >= 4 {
            let o = Owner::Set(ci as u8, si as u8);
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Attack, o, st::NONE, false, false, -1));
            s.subscribe(EventKind::SpecialReactionAttack, o);
        }
    }

    fn attack_mod(s: &Sim, _: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        ev.stats[st::CR] += bonus(s);
    }

    /// gcsim's workaround for lunar reactions: the same CRIT Rate on every
    /// lunar contribution against the enemy, whoever makes it (gcsim does
    /// not check the contributor).
    fn event(s: &mut Sim, _: usize, _: usize, e: &Event) {
        if let Event::SpecialReactionAttack(_) = *e {
            s.special_atk.stats[st::CR] += bonus(s);
        }
    }
}
