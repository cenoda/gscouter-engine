//! A Teaspoon of Transcendence, from gcsim's internal/weapons/claymore/ateaspoonoftranscendence.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const ATK: ModKey = ModKey::new("a-teaspoon-of-transcendence-atk");
const STACK_ICD: ModKey = ModKey::new("a-teaspoon-of-transcendence-stack-icd");
const STELLAR: ModKey = ModKey::new("a-teaspoon-of-transcendence-stellar");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "ateaspoonoftranscendence";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// ATK +28 to 56%.
    fn init(s: &mut Sim, ci: usize) {
        let atk = 0.21 + 0.07 * s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(ATK, st::ATKP, atk, false, false, -1));
        s.subscribe(EventKind::EnemyHit, Owner::Weapon(ci as u8));
    }

    /// Stellar DMG +16 to 32% per stack (of the holder's reactions).
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if !info.tag.is_stellar() {
            return 0.0;
        }
        (0.12 + 0.04 * s.chars[ci].refine as f64) * s.weapon_st::<State>(ci).stacks as f64
    }

    /// A charged attack hit by the holder, once every 0.2 s (hitlag): a
    /// stack (up to three, from none once the buff is gone), the buff for
    /// 5 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if ev.info.actor as usize != ci || ev.info.tag != AttackTag::Extra || s.mod_active(ci, STACK_ICD) {
            return;
        }
        if !s.mod_active(ci, STELLAR) {
            s.weapon_st_mut::<State>(ci).stacks = 0;
        }
        let st = s.weapon_st_mut::<State>(ci);
        st.stacks = (st.stacks + 1).min(3);
        s.add_status(ci, STACK_ICD, 12, true);
        s.add_mod(ci, Mod::owned(STELLAR, ModKind::ReactBonus, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 5 * 60);
    }
}
