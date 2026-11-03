//! Song of Broken Pines, from gcsim's internal/weapons/claymore/pines.
//!
//! The party's ATK buff has gcsim's common Millennial key, which Elegy for
//! the End's and Freedom-Sworn's share.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const ATK: ModKey = ModKey::new("pines-atk");
const PROC: ModKey = ModKey::new("pines-proc");
const ICD: ModKey = ModKey::new("songofbrokenpines-icd");
const CD: ModKey = ModKey::new("songofbrokenpines-cooldown");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "songofbrokenpines";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// ATK +16 to 32%.
    fn init(s: &mut Sim, ci: usize) {
        let atk = 0.12 + s.chars[ci].refine as f64 * 0.04;
        s.chars[ci].mods.push(Mod::stat(ATK, st::ATKP, atk, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// ATK SPD +12 to 24%, only in a normal attack's state.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if s.current_state() != AnimState::Normal {
            return false;
        }
        out[st::ATKSPD] += 0.09 + 0.03 * s.chars[ci].refine as f64;
        true
    }

    /// A normal or charged hit by the holder on field, once every 0.3 s
    /// (hitlag): a sigil; at four, out of a 20 s cooldown (hitlag), every
    /// party member gets the ATK SPD above and ATK +20 to 40% for 12 s
    /// (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        if s.mod_active(ci, CD) || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 18, true);
        let st = s.weapon_st_mut::<State>(ci);
        st.stacks += 1;
        if st.stacks != 4 {
            return;
        }
        st.stacks = 0;
        s.add_status(ci, CD, 20 * 60, true);
        let atk = 0.15 + 0.05 * s.chars[ci].refine as f64;
        let o = Owner::Weapon(ci as u8);
        for j in 0..s.chars.len() {
            s.add_mod(j, Mod::owned(PROC, ModKind::Stat, o, st::ATKSPD, false, true, -1), 12 * 60);
            s.add_mod(j, Mod::stat(super::elegyfortheend::MILLENNIAL, st::ATKP, atk, false, true, -1), 12 * 60);
        }
    }
}
