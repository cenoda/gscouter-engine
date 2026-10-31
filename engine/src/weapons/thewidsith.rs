//! The Widsith, from gcsim's internal/weapons/catalyst/widsith: swapping
//! in, once every 30 s, gives one of ATK, elemental DMG bonus or EM, at
//! random, for 10 s.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    /// gcsim's state: which buff (EM, DMG bonus, ATK), -1 before the first.
    state: i32,
}

const KEY: ModKey = ModKey::new("widsith");
const ICD: ModKey = ModKey::new("widsith-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "thewidsith";
    type State = State;
    fn new_state() -> State {
        State { state: -1 }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Swap, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Swap { next, .. } = *e else { return };
        if next != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 1800, true);
        s.weapon_st_mut::<State>(ci).state = s.rng.intn(3);
        s.add_mod(ci, Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 600);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        match s.weapon_st::<State>(ci).state {
            0 => out[st::EM] += 180.0 + r * 60.0,
            1 => {
                let dmg = 0.36 + r * 0.12;
                for stat in [st::PYROP, st::HYDROP, st::CRYOP, st::ELECTROP, st::ANEMOP, st::GEOP, st::DENDROP] {
                    out[stat] += dmg;
                }
            }
            2 => out[st::ATKP] += 0.45 + r * 0.15,
            _ => return false,
        }
        true
    }
}
