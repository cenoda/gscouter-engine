//! Sturdy Bone, from gcsim's internal/weapons/sword/sturdybone.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const BUFF: ModKey = ModKey::new("sturdy-bone");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "sturdybone";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Dash, o);
        s.subscribe(EventKind::EnemyHit, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Dash => {
                if s.player.active as usize != ci {
                    return;
                }
                s.weapon_st_mut::<State>(ci).stacks = 18;
                s.add_status(ci, BUFF, 7 * 60, true);
            }
            Event::EnemyHit(ev) => {
                if s.player.active as usize != ci || ev.info.actor as usize != ci || ev.info.tag != AttackTag::Normal || !s.mod_active(ci, BUFF) {
                    return;
                }
                let st = s.weapon_st_mut::<State>(ci);
                if st.stacks <= 0 {
                    return;
                }
                st.stacks -= 1;
                let r = s.chars[ci].refine as f64;
                let dmg_added = s.total_atk(ci) * (0.12 + 0.40 * r);
                s.hit_flat += dmg_added;
            }
            _ => {}
        }
    }
}
