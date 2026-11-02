//! Dialogues of the Desert Sages, from gcsim's
//! internal/weapons/spear/dialoguesofthedesertsages.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("dialoguesofthedesertsages-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "dialoguesofthedesertsages";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Heal, Owner::Weapon(ci as u8));
    }

    /// A heal by the holder (on or off field, of any amount), once every
    /// 10 s (hitlag): 8 to 16 energy.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Heal { caller, .. } = *e else { return };
        if caller != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 600, true);
        let amt = 6.0 + s.chars[ci].refine as f64 * 2.0;
        s.add_energy(ci, amt);
    }
}
