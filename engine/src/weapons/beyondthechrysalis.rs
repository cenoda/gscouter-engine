//! Beyond the Chrysalis, from gcsim's internal/weapons/sword/beyondthechrysalis.
//!
//! Its second buff adds to Stellar Swirl DMG only, which the engine never
//! deals (Stellar Swirl is an error), so it is not added; the cycle that
//! reaches it is kept. Port it with Stellar Swirl.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    /// gcsim's buffCount: which of the three effects comes next.
    count: i32,
}

const CD: ModKey = ModKey::new("beyond-the-chrysalis-crit-dmg");
const ENERGY_ICD: ModKey = ModKey::new("beyond-the-chrysalis-energy-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "beyondthechrysalis";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::Burst, o);
        s.subscribe(EventKind::Swap, o);
    }

    /// A skill or burst on field, in turn: CRIT DMG +56 to 120% for 10 s
    /// (hitlag); the Stellar Swirl buff; 5 to 7 energy (once every 4 s).
    /// Leaving the field starts the turn over and ends them.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let r = s.chars[ci].refine as f64;
        match *e {
            Event::Skill | Event::Burst => {
                if s.player.active as usize != ci {
                    return;
                }
                match s.weapon_st::<State>(ci).count {
                    0 => s.add_stat_mod(ci, CD, st::CD, 0.4 + 0.16 * r, 10 * 60, true),
                    1 => {}
                    _ => {
                        if !s.mod_active(ci, ENERGY_ICD) {
                            s.add_energy(ci, 4.5 + 0.5 * r);
                            s.add_status(ci, ENERGY_ICD, 4 * 60, true);
                        }
                    }
                }
                let st = s.weapon_st_mut::<State>(ci);
                st.count = (st.count + 1) % 3;
            }
            Event::Swap { prev, .. } if prev == ci => {
                s.weapon_st_mut::<State>(ci).count = 0;
                s.delete_mod(ci, CD);
                s.delete_mod(ci, ENERGY_ICD);
            }
            _ => {}
        }
    }
}
