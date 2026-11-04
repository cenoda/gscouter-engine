//! End of the Line, from gcsim's internal/weapons/bow/endoftheline.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    proc_count: i32,
}

const EFFECT: ModKey = ModKey::new("endoftheline-effect");
const EFFECT_ICD: ModKey = ModKey::new("endoftheline-effect-icd");
const DMG_ICD: ModKey = ModKey::new("endoftheline-dmg-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "endoftheline";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Skill => {
                if s.player.active as usize != ci || s.mod_active(ci, EFFECT_ICD) {
                    return;
                }
                s.add_status(ci, EFFECT, 15 * 60, true);
                s.add_status(ci, EFFECT_ICD, 12 * 60, true);
                s.delete_mod(ci, DMG_ICD);
                s.weapon_st_mut::<State>(ci).proc_count = 0;
            }
            Event::EnemyDamage(ev) => {
                if s.player.active as usize != ci || ev.info.actor as usize != ci || !s.mod_active(ci, EFFECT) || s.mod_active(ci, DMG_ICD) {
                    return;
                }
                let r = s.chars[ci].refine as f64;
                let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 0.60 + r * 0.20);
                ai.abil = "End of the Line Proc";
                ai.pattern = s.circle_on_target(2.5);
                s.queue_attack(ai, 0, 0, 1);

                let st = s.weapon_st_mut::<State>(ci);
                st.proc_count += 1;
                if st.proc_count >= 3 {
                    s.delete_mod(ci, EFFECT);
                } else {
                    s.add_status(ci, DMG_ICD, 2 * 60, true);
                }
            }
            _ => {}
        }
    }
}
