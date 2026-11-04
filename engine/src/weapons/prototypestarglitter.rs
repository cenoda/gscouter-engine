//! Prototype Starglitter, from gcsim's internal/weapons/spear/prototype.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    stacks: i32,
}

const KEY: ModKey = ModKey::new("prototype");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "prototypestarglitter";
    type State = State;
    fn new_state() -> State {
        State { stacks: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    /// A skill by the holder on field adds a stack, up to two, and refreshes
    /// the buff for 12 s (hitlag); the stacks start over once the buff has
    /// ended.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Skill = *e else { return };
        if s.player.active as usize != ci {
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
        s.add_mod(ci, Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 720);
    }

    /// 8 to 16% per stack on normal and charged attacks, which gcsim's
    /// buff adds as ATK%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        let stacks = s.weapon_st::<State>(ci).stacks;
        ev.stats[st::ATKP] += (0.06 + 0.02 * s.chars[ci].refine as f64) * stacks as f64;
    }
}
