//! Scarletproof, from gcsim's internal/artifacts/scarletproof.

use crate::content::{Event, EventKind, Owner, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("scarletproof-2pc");
const CR: ModKey = ModKey::new("scarletproof-4pc-cr");
const DMG: ModKey = ModKey::new("scarletproof-4pc-dmg");

impl SetImpl for Set {
    const KEY: &'static str = "scarletproof";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        }
        if count >= 4 {
            s.subscribe(EventKind::Reaction, Owner::Set(ci as u8, si as u8));
        }
    }

    /// Stellar Swirl DMG, direct or not, +40%.
    fn react_bonus(_: &Sim, _: usize, _: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if info.tag.is_stellar_swirl() {
            0.4
        } else {
            0.0
        }
    }

    /// A Stellar Swirl by the wearer (on any target, as gcsim does not
    /// check): CRIT Rate +16% and the Stellar Swirl bonus for 10 s.
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::Reaction(Reaction::StellarSwirl, ev) = *e else { return };
        if ev.info.actor as usize != ci {
            return;
        }
        s.add_stat_mod(ci, CR, st::CR, 0.16, 10 * 60, true);
        s.add_mod(ci, Mod::owned(DMG, ModKind::ReactBonus, Owner::Set(ci as u8, si as u8), st::NONE, false, true, -1), 10 * 60);
    }
}
