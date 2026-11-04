//! Scholar, from gcsim's internal/artifacts/scholar.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("scholar-2pc");
const ICD: ModKey = ModKey::new("scholar-4pc-icd");

impl SetImpl for Set {
    const KEY: &'static str = "scholar";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ER, 0.20, false, false, -1));
        }
        if count >= 4 {
            s.subscribe(EventKind::ParticleReceived, Owner::Set(ci as u8, si as u8));
        }
    }

    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let Event::ParticleReceived = *e else { return };
        if s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 180, true);
        for c in 0..s.chars.len() {
            if matches!(s.chars[c].class, WeaponClass::Bow | WeaponClass::Catalyst) {
                s.add_energy(c, 3.0);
            }
        }
    }
}
