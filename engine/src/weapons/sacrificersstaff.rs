//! Sacrificer's Staff, from gcsim's internal/weapons/spear/sacrificersstaff.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    stacks: i32,
}

const BUFF: ModKey = ModKey::new("sacrificersstaff-buff");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "sacrificersstaff";
    type State = State;
    fn new_state() -> State {
        State { stacks: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// A skill hit by the holder, on or off field: a stack, up to three,
    /// for 6 s (hitlag); the stacks start over once the buff has ended.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            return;
        }
        let active = s.mod_active(ci, BUFF);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        st.stacks = (st.stacks + 1).min(3);
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 300);
    }

    /// ATK +8 to 16% and Energy Recharge +6 to 12% a stack.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let n = s.weapon_st::<State>(ci).stacks as f64;
        out[st::ATKP] += (0.06 + 0.02 * r) * n;
        out[st::ER] += (0.045 + 0.015 * r) * n;
        true
    }
}
