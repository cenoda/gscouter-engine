//! Thrilling Tales of Dragon Slayers, from gcsim's
//! internal/weapons/catalyst/thrilling: swapping out gives the character
//! coming in ATK for 10 s, once in 20 s.

use crate::content::{Event, Owner, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("ttds-icd");
const TTDS: ModKey = ModKey::new("ttds");
const ICD_DUR: i32 = 20 * 60;
const ICD_EXT: i32 = 60;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "thrillingtalesofdragonslayers";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Swap, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Swap { prev, next } = *e else { return };
        // coming back within the last second of the cooldown clears it
        if next == ci && s.mod_duration(ci, "ttds-icd") < ICD_EXT {
            s.delete_mod(ci, ICD);
        }
        if prev != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, ICD_DUR + ICD_EXT, true);
        let active = s.player.active as usize;
        if s.mod_active(active, TTDS) {
            return;
        }
        let v = 0.18 + s.chars[ci].refine as f64 * 0.06;
        s.add_stat_mod(active, TTDS, st::ATKP, v, 600, true);
    }
}
