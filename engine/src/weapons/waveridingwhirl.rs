//! Waveriding Whirl, from gcsim's internal/weapons/catalyst/waveridingwhirl.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF_ICD: ModKey = ModKey::new("waveridingwhirl-icd");
const BUFF: ModKey = ModKey::new("waveridingwhirl-buff");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "waveridingwhirl";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Skill = *e else { return };
        if s.player.active as usize != ci || s.mod_active(ci, BUFF_ICD) {
            return;
        }
        s.add_status(ci, BUFF_ICD, 15 * 60, true);
        let hydro_count = s.chars.iter().filter(|c| c.element == Element::Hydro).count().min(2);
        let r = s.chars[ci].refine as f64;
        let hpp = (0.15 + 0.05 * r) + (0.09 + 0.03 * r) * hydro_count as f64;
        s.add_mod(ci, Mod::stat(BUFF, st::HPP, hpp, false, true, -1), 10 * 60);
    }
}
