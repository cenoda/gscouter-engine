//! Flame-Forged Insight, from gcsim's
//! internal/weapons/claymore/flameforgedinsight.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("flame-forged-insight-icd");
const BUFF: ModKey = ModKey::new("flame-forged-insight-buff");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "flameforgedinsight";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    /// Electro-charged, Lunar-Charged, bloom, Lunar-Bloom or a
    /// crystallize by the holder on the enemy, once every 15 s (hitlag):
    /// 12 to 24 energy and EM +60 to 120 for 15 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        if !matches!(
            r,
            Reaction::ElectroCharged
                | Reaction::LunarCharged
                | Reaction::Bloom
                | Reaction::LunarBloom
                | Reaction::CrystallizeHydro
                | Reaction::CrystallizePyro
                | Reaction::CrystallizeElectro
                | Reaction::CrystallizeCryo
                | Reaction::LunarCrystallize
        ) {
            return;
        }
        if !ev.on_enemy() || ev.info.actor as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 15 * 60, true);
        let rf = s.chars[ci].refine as f64;
        s.add_energy(ci, 9.0 + rf * 3.0);
        s.add_mod(ci, Mod::stat(BUFF, st::EM, 45.0 + rf * 15.0, false, true, -1), 15 * 60);
    }
}
