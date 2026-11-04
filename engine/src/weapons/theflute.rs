//! The Flute, from gcsim's internal/weapons/sword/flute.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const ICD: ModKey = ModKey::new("flute-icd");
const DURATION: ModKey = ModKey::new("flute-stack-duration");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "theflute";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        s.add_status(ci, ICD, 30, true);
        let active = s.mod_active(ci, DURATION);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        st.stacks += 1;
        s.add_status(ci, DURATION, 1800, true);
        if s.weapon_st::<State>(ci).stacks == 5 {
            s.weapon_st_mut::<State>(ci).stacks = 0;
            s.delete_mod(ci, DURATION);
            let r = s.chars[ci].refine as f64;
            let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 0.75 + 0.25 * r);
            ai.abil = "Flute Proc";
            ai.pattern = s.circle_on_target(4.0);
            s.queue_attack(ai, 0, 0, 1);
        }
    }
}
