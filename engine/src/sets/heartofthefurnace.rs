//! Heart of the Furnace, from gcsim's internal/artifacts/heartofthefurnace.
//!
//! Its 4-piece also follows Stellar Swirl (gcsim's OnStellarSwirl and its
//! tags), which the engine never deals: Stellar Swirl is an error. Port
//! that part with it.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("furnace-2pc");
const ATK: ModKey = ModKey::new("furnace-4pc-atk");
const REACT: ModKey = ModKey::new("furnace-4pc-react");

impl SetImpl for Set {
    const KEY: &'static str = "heartofthefurnace";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        }
        if count >= 4 {
            let o = Owner::Set(ci as u8, si as u8);
            s.subscribe(EventKind::Reaction, o);
            s.subscribe(EventKind::EnemyDamage, o);
        }
    }

    /// Stellar DMG +50%.
    fn react_bonus(_: &Sim, _: usize, _: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if info.tag.is_stellar() {
            0.5
        } else {
            0.0
        }
    }

    /// A Stellar-Conduct by the wearer, or its Stellar damage: ATK +12% for
    /// it and Stellar DMG +50% for the party, 12 s (hitlag).
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let ev = match *e {
            Event::Reaction(Reaction::StellarConduct, ev) => ev,
            Event::EnemyDamage(ev) if ev.info.tag.is_stellar() => ev,
            _ => return,
        };
        if ev.info.actor as usize != ci {
            return;
        }
        s.add_stat_mod(ci, ATK, st::ATKP, 0.12, 12 * 60, true);
        for c in 0..s.chars.len() {
            s.add_mod(c, Mod::owned(REACT, ModKind::ReactBonus, Owner::Set(ci as u8, si as u8), st::NONE, false, true, -1), 12 * 60);
        }
    }
}
