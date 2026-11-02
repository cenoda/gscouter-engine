//! Song of Stillness, from gcsim's internal/weapons/bow/songofstillness.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("songofstillness-dmg-boost");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "songofstillness";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Heal, Owner::Weapon(ci as u8));
    }

    /// The holder healed (by anyone, also for 0): DMG +16 to 32% for 8 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Heal { on, .. } = *e else { return };
        if on != ci {
            return;
        }
        let v = 0.12 + s.chars[ci].refine as f64 * 0.04;
        s.add_stat_mod(ci, BUFF, st::DMGP, v, 8 * 60, true);
    }
}
