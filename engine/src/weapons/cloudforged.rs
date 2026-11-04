//! Cloudforged, from gcsim's internal/weapons/bow/cloudforged.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const BUFF: ModKey = ModKey::new("cloudforged-em");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "cloudforged";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnergyBurst, Owner::Weapon(ci as u8));
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let st = s.weapon_st::<State>(ci);
        out[st::EM] += (30.0 + 10.0 * r) * st.stacks as f64;
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnergyBurst { on, .. } = *e else { return };
        if on != ci {
            return;
        }
        let active = s.mod_active(ci, BUFF);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        if st.stacks < 2 {
            st.stacks += 1;
        }
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 18 * 60);
    }
}
