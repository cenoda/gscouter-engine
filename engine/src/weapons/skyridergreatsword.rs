//! Skyrider Greatsword, from gcsim's internal/weapons/claymore/skyrider.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const BUFF: ModKey = ModKey::new("skyrider");
const ICD: ModKey = ModKey::new("skyrider-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "skyridergreatsword";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci
            || s.player.active as usize != ci
            || s.mod_active(ci, ICD)
            || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra)
        {
            return;
        }
        let active = s.mod_active(ci, BUFF);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        if st.stacks < 4 {
            st.stacks += 1;
        }
        s.add_status(ci, ICD, 30, true);
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 360);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let stacks = s.weapon_st::<State>(ci).stacks;
        let mult = 0.05 + 0.01 * s.chars[ci].refine as f64;
        out[st::ATKP] += mult * stacks as f64;
        true
    }
}
