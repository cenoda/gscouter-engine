//! Oathsworn Eye, from gcsim's internal/weapons/catalyst/oathsworneye.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("oathsworn");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "oathsworneye";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    /// A skill by the holder on field: ER +24 to 48% for 10 s, extended by
    /// hitlag. gcsim checks the active character, not the skill's caster.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Skill = *e {
            if s.player.active as usize != ci {
                return;
            }
            let r = s.chars[ci].refine as f64;
            s.add_stat_mod(ci, BUFF, st::ER, 0.18 + 0.06 * r, 10 * 60, true);
        }
    }
}
