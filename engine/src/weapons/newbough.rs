//! New Bough, from gcsim's internal/weapons/sword/newbough.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const WINDOW: ModKey = ModKey::new("new-bough-window");
const ICD: ModKey = ModKey::new("new-bough-icd");
const ATK_KEY: ModKey = ModKey::new("new-bough-verdant-atk");
const EM_KEY: ModKey = ModKey::new("new-bough-verdant-em");
const STELLAR_KEY: ModKey = ModKey::new("new-bough-verdant-stellar");

fn radiance(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, crate::stellar::POLESTAR_FIELD) || s.mod_active(ci, crate::stellar::RADIANCE_SWIRL)
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "newbough";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::EnemyHit, o);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let stacks = s.weapon_st::<State>(ci).stacks as f64;
        let rad = radiance(s, ci);
        if m.key == ATK_KEY {
            let atk = if rad { 0.045 + 0.015 * r } else { 0.03 + 0.01 * r };
            out[st::ATKP] += atk * stacks;
            return true;
        }
        if m.key == EM_KEY {
            if !rad {
                out[st::EM] += (15.0 + 5.0 * r) * stacks;
            }
            return true;
        }
        false
    }

    fn react_bonus(s: &Sim, ci: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if m.key == STELLAR_KEY && info.tag.is_stellar() && radiance(s, ci) {
            let r = s.chars[ci].refine as f64;
            (0.06 + 0.02 * r) * s.weapon_st::<State>(ci).stacks as f64
        } else {
            0.0
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Skill => {
                if s.player.active as usize == ci {
                    s.add_status(ci, WINDOW, 12 * 60, true);
                }
            }
            Event::EnemyHit(ev) => {
                if ev.info.actor as usize != ci || !s.mod_active(ci, WINDOW) || s.mod_active(ci, ICD) {
                    return;
                }
                s.add_status(ci, ICD, 60, true);
                let active = s.mod_active(ci, ATK_KEY);
                let st = s.weapon_st_mut::<State>(ci);
                if !active {
                    st.stacks = 0;
                }
                st.stacks = (st.stacks + 1).min(3);
                let o = Owner::Weapon(ci as u8);
                s.add_mod(ci, Mod::owned(ATK_KEY, ModKind::Stat, o, st::NONE, false, true, -1), 6 * 60);
                s.add_mod(ci, Mod::owned(EM_KEY, ModKind::Stat, o, st::NONE, false, true, -1), 6 * 60);
                s.add_mod(ci, Mod::owned(STELLAR_KEY, ModKind::ReactBonus, o, st::NONE, false, true, -1), 6 * 60);
            }
            _ => {}
        }
    }
}
