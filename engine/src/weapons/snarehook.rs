//! Snare Hook, from gcsim's internal/weapons/bow/snarehook.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("snarehook-em");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "snarehook";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let mut em = 45.0 + 15.0 * r;
        if s.chars.iter().map(|c| c.moonsign).sum::<i32>() >= 2 {
            em *= 2.0;
        }
        out[st::EM] += em;
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        // gcsim keeps the reactions on an enemy (args[0] an *enemy.Enemy):
        // hyperbloom and burgeon come from the dendro core and are not
        if matches!(r, Reaction::Hyperbloom | Reaction::Burgeon) || !ev.on_enemy() || ev.info.actor as usize != ci {
            return;
        }
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::EM, false, true, -1), 12 * 60);
    }
}
