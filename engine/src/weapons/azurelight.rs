//! Azurelight, from gcsim's internal/weapons/sword/azurelight.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("azurelight-buff");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "azurelight";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    /// ATK +24 to 48%; with no energy, twice that and CRIT DMG +40 to 80%.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let c = &s.chars[ci];
        let r = c.refine as f64;
        let atkp = 0.18 + 0.06 * r;
        if c.energy == 0.0 {
            out[st::ATKP] += atkp * 2.0;
            out[st::CD] += 0.30 + 0.10 * r;
        } else {
            out[st::ATKP] += atkp;
        }
        true
    }

    /// A skill on field: the buff for 12 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Skill = *e {
            if s.player.active as usize == ci {
                s.add_mod(ci, Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 12 * 60);
            }
        }
    }
}
