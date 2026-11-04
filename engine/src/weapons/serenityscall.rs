//! Serenity's Call, from gcsim's internal/weapons/sword/serenityscall.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("serenityscall-hp");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "serenityscall";
    type State = State;
    fn new_state() -> State {
        State
    }

    /// Every reaction event (gcsim's ReactionEventStartDelim to
    /// ReactionEventEndDelim, Shatter and the Stellar ones included).
    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    /// A reaction by the holder on the enemy: Max HP +16 to 32% for 12 s
    /// (hitlag), twice that at Moonsign level 2 or more.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(_, ev) = *e else { return };
        if !ev.on_enemy() || ev.info.actor as usize != ci {
            return;
        }
        s.add_mod(ci, Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::HPP, false, true, -1), 12 * 60);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let buff = 0.12 + 0.04 * s.chars[ci].refine as f64;
        let level: i32 = s.chars.iter().map(|c| c.moonsign).sum();
        out[st::HPP] += if level >= 2 { buff * 2.0 } else { buff };
        true
    }
}
