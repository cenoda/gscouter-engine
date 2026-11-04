//! Frostbreath, from gcsim's internal/weapons/spear/frostbreath.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("frostbreath-atk");
const BUFF_ICD: ModKey = ModKey::new("frostbreath-buff-icd");
const ENERGY_ICD: ModKey = ModKey::new("frostbreath-energy-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "frostbreath";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        out[st::ATKP] += 0.15 + 0.05 * r;
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        if ev.info.actor as usize != ci || s.mod_active(ci, BUFF_ICD) {
            return;
        }
        let hydro_or_cryo = matches!(
            r,
            Reaction::Melt
                | Reaction::Frozen
                | Reaction::Shatter
                | Reaction::Superconduct
                | Reaction::SwirlCryo
                | Reaction::SwirlHydro
                | Reaction::CrystallizeCryo
                | Reaction::CrystallizeHydro
                | Reaction::Vaporize
                | Reaction::ElectroCharged
                | Reaction::Bloom
                | Reaction::LunarCharged
                | Reaction::LunarBloom
                | Reaction::LunarCrystallize
                | Reaction::StellarConduct
                | Reaction::StellarSwirl
        );
        if !hydro_or_cryo {
            return;
        }

        s.add_status(ci, BUFF_ICD, 16 * 60, true);
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 15 * 60);

        let refine = s.chars[ci].refine as f64;
        let energy = 4.5 + 1.5 * refine;
        for i in 0..s.chars.len() {
            if !s.mod_active(i, ENERGY_ICD) {
                s.add_status(i, ENERGY_ICD, 18, true);
                s.add_energy(i, energy);
            }
        }
    }
}
