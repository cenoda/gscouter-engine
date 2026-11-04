//! Bloodsoaked Ruins, from gcsim's internal/weapons/spear/bloodsoakedruins.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const LC: ModKey = ModKey::new("bloodsoakedruins-lc");
const CD: ModKey = ModKey::new("bloodsoakedruins-cd");
const ENERGY_ICD: ModKey = ModKey::new("bloodsoakedruins-energy-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "bloodsoakedruins";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Burst, o);
        s.subscribe(EventKind::Reaction, o);
    }

    /// Lunar-Charged DMG (direct or not) +36 to 84%.
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        match info.tag {
            AttackTag::ReactionLunarCharge | AttackTag::DirectLunarCharged => 0.24 + 0.12 * s.chars[ci].refine as f64,
            _ => 0.0,
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // a burst on field: the bonus for 3.5 s (hitlag)
            Event::Burst => {
                if s.player.active as usize == ci {
                    s.add_mod(ci, Mod::owned(LC, ModKind::ReactBonus, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 210);
                }
            }
            // a Lunar-Charged the holder triggers on field: CRIT DMG +28 to
            // 56% for 6 s (hitlag), and 12 to 16 energy once every 14 s
            // (hitlag)
            Event::Reaction(Reaction::LunarCharged, ev) => {
                if s.player.active as usize != ci || ev.info.actor as usize != ci {
                    return;
                }
                let r = s.chars[ci].refine as f64;
                s.add_mod(ci, Mod::stat(CD, st::CD, 0.21 + 0.07 * r, false, true, -1), 6 * 60);
                if s.mod_active(ci, ENERGY_ICD) {
                    return;
                }
                s.add_status(ci, ENERGY_ICD, 14 * 60, true);
                s.add_energy(ci, 11.0 + r);
            }
            _ => {}
        }
    }
}
