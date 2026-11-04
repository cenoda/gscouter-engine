//! Silverlight, from gcsim's internal/weapons/sword/silverlight.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    counter: usize,
}

const ICD: ModKey = ModKey::new("silver-light-stack-icd");
const MOD_1: ModKey = ModKey::new("silver-light-em-1");
const MOD_2: ModKey = ModKey::new("silver-light-em-2");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "silverlight";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Skill = *e else { return };
        if s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 12, true);
        let st = s.weapon_st_mut::<State>(ci);
        let key = if st.counter == 0 { MOD_1 } else { MOD_2 };
        st.counter = (st.counter + 1) % 2;
        let r = s.chars[ci].refine as f64;
        let em = 39.0 + 13.0 * r;
        s.add_mod(ci, Mod::stat(key, st::EM, em, false, true, -1), 12 * 60);
    }
}
