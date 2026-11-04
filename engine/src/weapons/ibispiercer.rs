//! Ibis Piercer, from gcsim's internal/weapons/bow/ibispiercer.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const BUFF: ModKey = ModKey::new("ibispiercer-stacks");
const ICD: ModKey = ModKey::new("ibispiercer-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "ibispiercer";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let st = s.weapon_st::<State>(ci);
        out[st::EM] += (30.0 + 10.0 * r) * st.stacks as f64;
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || ev.info.tag != AttackTag::Extra || s.mod_active(ci, ICD) {
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
        s.add_status(ci, ICD, 30, true);
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 6 * 60);
    }
}
