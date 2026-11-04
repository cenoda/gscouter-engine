//! Dark Iron Sword, from gcsim's internal/weapons/sword/darkironsword.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("darkironsword");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "darkironsword";
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
            Reaction::Overload
                | Reaction::Superconduct
                | Reaction::StellarConduct
                | Reaction::ElectroCharged
                | Reaction::LunarCharged
                | Reaction::Quicken
                | Reaction::Aggravate
                | Reaction::Hyperbloom
                | Reaction::SwirlElectro
        ) {
            let r = s.chars[ci].refine as f64;
            s.add_mod(ci, Mod::stat(BUFF, st::ATKP, 0.15 + r * 0.05, false, true, -1), 720);
        }
    }
}
