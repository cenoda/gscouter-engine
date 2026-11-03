//! Windblume Ode, from gcsim's internal/weapons/bow/windblume.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("windblume");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "windblumeode";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    /// A skill by the holder on field: ATK +16 to 32% for 6 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Skill = *e else { return };
        if s.player.active as usize != ci {
            return;
        }
        let atk = 0.12 + s.chars[ci].refine as f64 * 0.04;
        s.add_mod(ci, Mod::stat(KEY, st::ATKP, atk, false, true, -1), 360);
    }
}
