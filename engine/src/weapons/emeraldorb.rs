//! Emerald Orb, from gcsim's internal/weapons/catalyst/emeraldorb.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("emeraldorb");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "emeraldorb";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(rx, atk) = *e else { return };
        if atk.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        if matches!(
            rx,
            Reaction::Vaporize
                | Reaction::ElectroCharged
                | Reaction::LunarCharged
                | Reaction::Frozen
                | Reaction::SwirlHydro
                | Reaction::Bloom
                | Reaction::LunarBloom
        ) {
            let r = s.chars[ci].refine as f64;
            s.add_mod(ci, Mod::stat(BUFF, st::ATKP, 0.15 + r * 0.05, false, true, -1), 720);
        }
    }
}
