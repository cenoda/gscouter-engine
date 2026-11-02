//! Flowing Purity, from gcsim's internal/weapons/catalyst/flowingpurity.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    /// gcsim's debt: the Bond of Life counted towards the bond buff.
    debt: f64,
    /// The bond buff's elemental DMG, which a later skill changes even
    /// while the buff lasts (gcsim's shared slice).
    bond: f64,
}

const ICD: ModKey = ModKey::new("flowingpurity-icd");
const BOND: ModKey = ModKey::new("flowingpurity-bond");
const ELE: ModKey = ModKey::new("flowingpurity-eledmg-boost");
const BOND_ELE: ModKey = ModKey::new("flowingpurity-bond-eledmg-boost");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "flowingpurity";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::Heal, o);
    }

    /// Elemental DMG (pyro to dendro, not physical): the skill's 8 to 16%,
    /// or the bond's.
    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let v = if m.key == BOND_ELE { s.weapon_st::<State>(ci).bond } else { 0.06 + s.chars[ci].refine as f64 * 0.02 };
        for k in st::PYROP..=st::DENDROP {
            out[k] += v;
        }
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let r = s.chars[ci].refine as f64;
        let o = Owner::Weapon(ci as u8);
        match *e {
            // a skill on field, once every 10 s: elemental DMG for 15 s,
            // and a Bond of Life of 24% of max HP
            Event::Skill => {
                if s.player.active as usize != ci || s.mod_active(ci, ICD) {
                    return;
                }
                s.add_status(ci, ICD, 10 * 60, true);
                s.add_mod(ci, Mod::owned(ELE, ModKind::Stat, o, st::NONE, false, true, -1), 15 * 60);
                if !s.mod_active(ci, BOND) {
                    s.weapon_st_mut::<State>(ci).debt = 0.0;
                }
                s.add_status(ci, BOND, -1, true);
                s.modify_hp_debt_ratio(ci, 0.24);
                let debt = s.hp_debt(ci);
                let st = s.weapon_st_mut::<State>(ci);
                st.debt += debt;
                st.bond = ((st.debt / 1000.0) * (0.015 + r * 0.005)).min(0.09 + r * 0.03);
            }
            // healed with the Bond cleared: the bond's elemental DMG for
            // 15 s
            Event::Heal { on, .. } => {
                if on != ci || s.hp_debt(ci) > 0.0 || !s.mod_active(ci, BOND) {
                    return;
                }
                s.delete_mod(ci, BOND);
                s.add_mod(ci, Mod::owned(BOND_ELE, ModKind::Stat, o, st::NONE, false, true, -1), 15 * 60);
            }
            _ => {}
        }
    }
}
