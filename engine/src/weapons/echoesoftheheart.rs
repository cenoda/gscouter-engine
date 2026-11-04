//! Echoes of the Heart, from gcsim's internal/weapons/catalyst/echoesoftheheart.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const EM_BUFF: ModKey = ModKey::new("echoes-of-the-heart-em");
const STELLAR_BUFF: ModKey = ModKey::new("echoes-of-the-heart-stellar");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "echoesoftheheart";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    fn react_bonus(s: &Sim, ci: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if m.key == STELLAR_BUFF && info.tag.is_stellar() {
            let r = s.chars[ci].refine as f64;
            0.12 + 0.04 * r
        } else {
            0.0
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        if ev.info.actor as usize != ci {
            return;
        }
        let o = Owner::Weapon(ci as u8);
        let refine = s.chars[ci].refine as f64;
        let em = 45.0 + 15.0 * refine;
        s.add_mod(ci, Mod::stat(EM_BUFF, st::EM, em, false, true, -1), 12 * 60);

        if matches!(r, Reaction::StellarConduct | Reaction::StellarSwirl) {
            s.add_mod(ci, Mod::owned(STELLAR_BUFF, ModKind::ReactBonus, o, st::NONE, false, true, -1), 12 * 60);
        }
    }
}
