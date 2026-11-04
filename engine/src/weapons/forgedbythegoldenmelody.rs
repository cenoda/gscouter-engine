//! Forged by the Golden Melody, from gcsim's internal/weapons/claymore/forgedbythegoldenmelody.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    cur_buff: usize,
}

const ATK_NORM: ModKey = ModKey::new("golden-melody-atk");
const ATK_CONTRA: ModKey = ModKey::new("golden-melody-contrapuntal-atk");
const EM_NORM: ModKey = ModKey::new("golden-melody-em");
const EM_CONTRA: ModKey = ModKey::new("golden-melody-contrapuntal-em");
const STELLAR_NORM: ModKey = ModKey::new("golden-melody-stellar");
const STELLAR_CONTRA: ModKey = ModKey::new("golden-melody-contrapuntal-stellar");

const CONTRA_ICD: ModKey = ModKey::new("golden-melody-contrapuntal-icd");
const TASK_SWITCH: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "forgedbythegoldenmelody";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        // gcsim takes buff_type % 3, and panics on a negative one
        let buff_type = s.chars[ci].weapon_param("buff_type");
        if buff_type < 0 {
            s.unsupported("param forgedbythegoldenmelody buff_type < 0");
        }
        s.weapon_st_mut::<State>(ci).cur_buff = (buff_type.rem_euclid(3)) as usize;
        Self::apply_buff(s, ci, false, 600);
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_SWITCH, arg: 0 }, 600);
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    fn react_bonus(s: &Sim, ci: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if (m.key == STELLAR_NORM || m.key == STELLAR_CONTRA) && info.tag.is_stellar() {
            let r = s.chars[ci].refine as f64;
            0.21 + 0.07 * r
        } else {
            0.0
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        if ev.info.actor as usize != ci || !matches!(r, Reaction::StellarConduct | Reaction::StellarSwirl) {
            return;
        }
        if s.mod_active(ci, CONTRA_ICD) {
            return;
        }
        s.add_status(ci, CONTRA_ICD, 12 * 60, true);
        Self::apply_buff(s, ci, true, 12 * 60);
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        if id == TASK_SWITCH {
            let st = s.weapon_st_mut::<State>(ci);
            st.cur_buff = (st.cur_buff + 1) % 3;
            Self::apply_buff(s, ci, false, 600);
            s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_SWITCH, arg: 0 }, 600);
        }
    }
}

impl Weapon {
    fn apply_buff(s: &mut Sim, ci: usize, contrapuntal: bool, dur: i32) {
        let r = s.chars[ci].refine as f64;
        let cur = s.weapon_st::<State>(ci).cur_buff;
        let o = Owner::Weapon(ci as u8);
        match cur {
            0 => {
                let key = if contrapuntal { ATK_CONTRA } else { ATK_NORM };
                let atk = 0.135 + 0.045 * r;
                s.add_mod(ci, Mod::stat(key, st::ATKP, atk, false, true, -1), dur);
            }
            1 => {
                let key = if contrapuntal { EM_CONTRA } else { EM_NORM };
                let em = 90.0 + 30.0 * r;
                s.add_mod(ci, Mod::stat(key, st::EM, em, false, true, -1), dur);
            }
            2 => {
                let key = if contrapuntal { STELLAR_CONTRA } else { STELLAR_NORM };
                s.add_mod(ci, Mod::owned(key, ModKind::ReactBonus, o, st::NONE, false, true, -1), dur);
            }
            _ => {}
        }
    }
}
