//! Song of the Vigil, from gcsim's internal/weapons/spear/songofthevigil.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ENERGY_ICD: ModKey = ModKey::new("song-of-the-vigil-energy-icd");
const ATK_BUFF: ModKey = ModKey::new("song-of-the-vigil-atk");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "songofthevigil";
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
        let r_val = s.chars[ci].refine as f64;
        if !s.mod_active(ci, ENERGY_ICD) {
            s.add_status(ci, ENERGY_ICD, 9 * 60, true);
            s.add_energy(ci, 3.0 + r_val);
        }
        if matches!(r, Reaction::StellarConduct | Reaction::StellarSwirl) {
            let atk = 0.15 + 0.05 * r_val;
            s.add_mod(ci, Mod::stat(ATK_BUFF, st::ATKP, atk, false, true, -1), 12 * 60);
        }
    }
}
