//! Emberwell, from gcsim's internal/weapons/sword/emberwell.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ATK_BUFF: ModKey = ModKey::new("emberwell-atk");
const STELLAR_BUFF: ModKey = ModKey::new("emberwell-stellar");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "emberwell";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == ATK_BUFF {
            let r = s.chars[ci].refine as f64;
            out[st::ATKP] += 0.12 + 0.04 * r;
            return true;
        }
        false
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
        s.add_mod(ci, Mod::owned(ATK_BUFF, ModKind::Stat, o, st::NONE, false, true, -1), 12 * 60);
        if matches!(r, Reaction::StellarConduct | Reaction::StellarSwirl) {
            s.add_mod(ci, Mod::owned(STELLAR_BUFF, ModKind::ReactBonus, o, st::NONE, false, true, -1), 12 * 60);
        }
    }
}
