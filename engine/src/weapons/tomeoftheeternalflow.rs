//! Tome of the Eternal Flow, from gcsim's internal/weapons/catalyst/eternalflow.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const HPP: ModKey = ModKey::new("eternalflow-hpp");
const BUFF: ModKey = ModKey::new("eternalflow-buff");
const BUFF_ICD: ModKey = ModKey::new("eternalflow-icd");
const ENERGY_ICD: ModKey = ModKey::new("eternalflow-energy-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "tomeoftheeternalflow";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// HP +16 to 32%.
    fn init(s: &mut Sim, ci: usize) {
        let hpp = 0.12 + s.chars[ci].refine as f64 * 0.04;
        s.chars[ci].mods.push(Mod::stat(HPP, st::HPP, hpp, false, false, -1));
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Drain, o);
        s.subscribe(EventKind::Heal, o);
    }

    /// Charged attack DMG +14 to 30% per stack.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::Extra {
            let r = s.chars[ci].refine as f64;
            ev.stats[st::DMGP] += (0.10 + 0.04 * r) * s.weapon_st::<State>(ci).stacks as f64;
        }
    }

    /// The holder's HP changing on field (lost, or healed not all as
    /// overheal), once every 0.3 s (hitlag): a stack for 4 s (hitlag), up
    /// to 3, counted again from 0 once the buff has gone; at 3 stacks, 8
    /// to 12 energy, once every 12 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let on = match *e {
            Event::Drain { on, amount, .. } if amount > 0.0 => on,
            Event::Heal { on, amount, overheal, .. } if amount > 0.0 && (amount - overheal).abs() > 1e-9 => on,
            _ => return,
        };
        if s.player.active as usize != ci || on != ci || s.mod_active(ci, BUFF_ICD) {
            return;
        }
        let active = s.mod_active(ci, BUFF);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        if st.stacks < 3 {
            st.stacks += 1;
        }
        let stacks = st.stacks;
        s.add_status(ci, BUFF_ICD, 18, true);
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 4 * 60);
        if stacks == 3 && !s.mod_active(ci, ENERGY_ICD) {
            let amt = 7.0 + s.chars[ci].refine as f64;
            s.add_energy(ci, amt);
            s.add_status(ci, ENERGY_ICD, 12 * 60, true);
        }
    }
}
