//! Exaiphanes' Blade, from gcsim's internal/weapons/sword/exaiphanesblade.
//!
//! It does anything only on a Traveler (gcsim's NewWeapon returns at once
//! for any other character). The engine has no Traveler yet, so the
//! Traveler's part is ported but no supported team reaches it.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("exaiphanes-blade-on-hit-icd");
const ATK: ModKey = ModKey::new("exaiphanes-blade-atk");
const CD: ModKey = ModKey::new("exaiphanes-blade-cd");

fn is_traveler(s: &Sim, ci: usize) -> bool {
    let name = crate::gcsl::char_name(s.chars[ci].key);
    name.starts_with("aether") || name.starts_with("lumine")
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "exaiphanesblade";
    type State = State;
    fn new_state() -> State {
        State
    }

    /// On a Traveler: the on-hit buff, and from refinement 2 CRIT DMG +6%
    /// per element (the `elements` param, 7 when absent, 1 to 7).
    fn init(s: &mut Sim, ci: usize) {
        if !is_traveler(s, ci) {
            return;
        }
        s.subscribe(EventKind::EnemyHit, Owner::Weapon(ci as u8));
        if s.chars[ci].refine < 2 {
            return;
        }
        let ele = s.chars[ci].weapon_params.iter().find(|p| p.0 == "elements").map_or(7, |p| p.1).clamp(1, 7);
        s.chars[ci].mods.push(Mod::stat(CD, st::CD, ele as f64 * 0.06, false, false, -1));
    }

    /// A hit by the holder, once every 5 s (hitlag): ATK +16 to 40% for
    /// 8 s (hitlag) and 3 energy (5 from refinement 3).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        let r = s.chars[ci].refine;
        let (atk, energy) = if r >= 3 { (r as f64 * 0.08, 5.0) } else { (0.12 + r as f64 * 0.04, 3.0) };
        s.add_status(ci, ICD, 5 * 60, true);
        s.add_mod(ci, Mod::stat(ATK, st::ATKP, atk, false, true, -1), 8 * 60);
        s.add_energy(ci, energy);
    }
}
