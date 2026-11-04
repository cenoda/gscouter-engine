//! Blade of Atonement, from gcsim's internal/weapons/claymore/bladeofatonement.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const EM_BUFF: ModKey = ModKey::new("blade-of-atonement-em");
const ATK_BUFF: ModKey = ModKey::new("blade-of-atonement-atk");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "bladeofatonement";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        if ev.info.actor as usize != ci {
            return;
        }
        let refine = s.chars[ci].refine as f64;
        let em = 48.0 + 16.0 * refine;
        s.add_mod(ci, Mod::stat(EM_BUFF, st::EM, em, false, true, -1), 12 * 60);

        if matches!(r, Reaction::StellarConduct | Reaction::StellarSwirl) {
            let atk = 0.12 + 0.04 * refine;
            s.add_mod(ci, Mod::stat(ATK_BUFF, st::ATKP, atk, false, true, -1), 12 * 60);
        }
    }
}
