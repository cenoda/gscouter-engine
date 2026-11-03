//! Whiteblind, from gcsim's internal/weapons/claymore/whiteblind.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const BUFF: ModKey = ModKey::new("whiteblind");
const ICD: ModKey = ModKey::new("whiteblind-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "whiteblind";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// ATK and DEF +6 to 12% a stack (gcsim's buff, read live).
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let v = (0.045 + 0.015 * s.chars[ci].refine as f64) * s.weapon_st::<State>(ci).stacks as f64;
        out[st::ATKP] += v;
        out[st::DEFP] += v;
        true
    }

    /// The holder's normal and charged attack damage on field, once every
    /// 0.5 s (hitlag): a stack for 6 s (hitlag), up to 4, all refreshed.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) || s.mod_active(ci, ICD) {
            return;
        }
        let active = s.mod_active(ci, BUFF);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        s.add_status(ci, ICD, 30, true);
        let st = s.weapon_st_mut::<State>(ci);
        if st.stacks < 4 {
            st.stacks += 1;
        }
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 360);
    }
}
