//! Range Gauge, from gcsim's internal/weapons/bow/rangegauge.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
    count: i32,
}

const SYMBOL: ModKey = ModKey::new("range-gauge-symbol");
const ICD: ModKey = ModKey::new("range-gauge-icd");
const BUFF: ModKey = ModKey::new("range-gauge");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "rangegauge";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Heal, o);
        s.subscribe(EventKind::Burst, o);
        s.subscribe(EventKind::Skill, o);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let n = s.weapon_st::<State>(ci).count as f64;
        out[st::ATKP] += (0.02 + 0.01 * r) * n;
        for i in st::PYROP..=st::DENDROP {
            out[i] += (0.055 + 0.015 * r) * n;
        }
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Heal { caller, on, amount, .. } => {
                if (caller != ci && on != ci) || amount <= 0.0 {
                    return;
                }
                if !s.mod_active(ci, SYMBOL) {
                    s.weapon_st_mut::<State>(ci).stacks = 0;
                }
                let st = s.weapon_st_mut::<State>(ci);
                if st.stacks < 3 {
                    st.stacks += 1;
                }
                s.add_status(ci, SYMBOL, 30 * 60, true);
            }
            Event::Skill | Event::Burst => {
                if !s.mod_active(ci, SYMBOL) || s.mod_active(ci, ICD) || s.player.active as usize != ci {
                    return;
                }
                s.add_status(ci, ICD, 15 * 60, true);
                s.delete_mod(ci, SYMBOL);
                let st = s.weapon_st_mut::<State>(ci);
                st.count = st.stacks;
                st.stacks = 0;
                s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 10 * 60);
            }
            _ => {}
        }
    }
}
