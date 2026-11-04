//! Mappa Mare, from gcsim's internal/weapons/catalyst/mappa: a reaction
//! the holder triggers on field (any of gcsim's reaction events, on any
//! target) gives a stack of elemental DMG +8 to 16% for 10 s (hitlag), up
//! to two; the stacks start over once the buff is gone.

use crate::content::{Event, Owner, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    stacks: i32,
}

const KEY: ModKey = ModKey::new("mappa-mare");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "mappamare";
    type State = State;
    fn new_state() -> State {
        State { stacks: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(_, ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        let active = s.mod_active(ci, KEY);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        if st.stacks < 2 {
            st.stacks += 1;
        }
        // the amount is fixed when the mod is added; the stacks change only
        // here, so reading them is the same
        s.add_mod(ci, Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 600);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let p = (0.06 + s.chars[ci].refine as f64 * 0.02) * s.weapon_st::<State>(ci).stacks as f64;
        for stat in [st::PYROP, st::HYDROP, st::CRYOP, st::ELECTROP, st::ANEMOP, st::GEOP, st::DENDROP] {
            out[stat] += p;
        }
        true
    }
}
