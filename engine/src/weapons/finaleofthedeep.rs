//! Finale of the Deep, from gcsim's internal/weapons/sword/finaleofthedeep.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    /// gcsim's collectedDebt: the Bond of Life granted (gcsim collects
    /// the OnHPDebt changes that raise it) since the last flat ATK bonus.
    collected: f64,
}

const ICD: ModKey = ModKey::new("finaleofthedeep-icd");
const ATK_BOOST: ModKey = ModKey::new("finaleofthedeep-atk-boost");
const BOND_BOOST: ModKey = ModKey::new("finaleofthedeep-bond-flatatk-boost");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "finaleofthedeep";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::HpDebt, o);
    }

    /// gcsim's Weapon.Init.
    fn post_init(s: &mut Sim, ci: usize) {
        s.weapon_st_mut::<State>(ci).collected = 0.0;
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let r = s.chars[ci].refine as f64;
        match *e {
            // A skill by the holder on field, once every 10 s: ATK +12 to
            // 24% for 15 s, and a Bond of Life of 25% of max HP.
            Event::Skill => {
                if s.player.active as usize != ci || s.mod_active(ci, ICD) {
                    return;
                }
                s.add_status(ci, ICD, 10 * 60, true);
                s.add_stat_mod(ci, ATK_BOOST, st::ATKP, 0.09 + r * 0.03, 15 * 60, true);
                s.modify_hp_debt_ratio(ci, 0.25);
            }
            // When the holder's Bond of Life is cleared: flat ATK of 2.4 to
            // 4.8% of the Bond cleared (up to 150 to 300) for 15 s.
            Event::HpDebt { on, change } => {
                if on != ci {
                    return;
                }
                if change < 0.0 {
                    s.weapon_st_mut::<State>(ci).collected += -change;
                }
                if s.hp_debt(ci) > 0.0 {
                    return;
                }
                let collected = s.weapon_st::<State>(ci).collected;
                if collected < 0.0001 {
                    return;
                }
                let atk = (112.5 + r * 37.5).min(collected * (0.018 + r * 0.006));
                s.weapon_st_mut::<State>(ci).collected = 0.0;
                s.add_stat_mod(ci, BOND_BOOST, st::ATK, atk, 15 * 60, true);
            }
            _ => {}
        }
    }
}
