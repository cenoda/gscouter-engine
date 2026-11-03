//! Rainbow Serpent's Rainbow, from gcsim's internal/weapons/bow/rainbow.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("rainbow-serpent-bow");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "rainbowserpentsrainbow";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// Damage by the holder off field: ATK +28 to 56% for 8 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize == ci {
            return;
        }
        let atk = 0.21 + s.chars[ci].refine as f64 * 0.07;
        s.add_mod(ci, Mod::stat(BUFF, st::ATKP, atk, false, true, -1), 8 * 60);
    }
}
