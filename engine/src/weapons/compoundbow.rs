//! Compound Bow, from gcsim's internal/weapons/bow/compound.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const BUFF: ModKey = ModKey::new("compoundbow");
const STACK_KEY: ModKey = ModKey::new("compoundbow-stacks");
const ICD: ModKey = ModKey::new("compoundbow-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "compoundbow";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let st = s.weapon_st::<State>(ci);
        let r = s.chars[ci].refine as f64;
        out[st::ATKP] += (0.03 + 0.01 * r) * st.stacks as f64;
        out[st::ATKSPD] += (0.009 + 0.003 * r) * st.stacks as f64;
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        let active = s.mod_active(ci, STACK_KEY);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        if st.stacks < 4 {
            st.stacks += 1;
        }
        s.add_status(ci, ICD, 18, true);
        s.add_status(ci, STACK_KEY, 360, true);
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 360);
    }
}
