//! Prototype Rancour, from gcsim's internal/weapons/sword/prototype.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const ICD: ModKey = ModKey::new("prototype-rancour-icd");
const BUFF: ModKey = ModKey::new("prototype-rancour");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "prototyperancour";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// ATK and DEF +4 to 8% per stack.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let v = (0.03 + 0.01 * s.chars[ci].refine as f64) * s.weapon_st::<State>(ci).stacks as f64;
        out[st::ATKP] += v;
        out[st::DEFP] += v;
        true
    }

    /// A normal or charged hit by the holder on field, once every 0.3 s
    /// (hitlag): a stack (up to four, from none once the buff is gone),
    /// the buff for 6 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        if s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 18, true);
        if !s.mod_active(ci, BUFF) {
            s.weapon_st_mut::<State>(ci).stacks = 0;
        }
        let st = s.weapon_st_mut::<State>(ci);
        if st.stacks < 4 {
            st.stacks += 1;
        }
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 360);
    }
}
