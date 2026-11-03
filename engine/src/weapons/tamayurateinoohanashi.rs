//! Tamayuratei no Ohanashi, from gcsim's internal/weapons/spear/tamayuratei.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("tamayuratei");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "tamayurateinoohanashi";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    /// A skill on field: ATK +20 to 40% for 10 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Skill = *e {
            if s.player.active as usize == ci {
                s.add_mod(ci, Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 10 * 60);
            }
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::ATKP] += 0.15 + s.chars[ci].refine as f64 * 0.05;
        true
    }
}
