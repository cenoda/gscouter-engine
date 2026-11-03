//! Elegy for the End, from gcsim's internal/weapons/bow/elegy.
//!
//! The party's ATK buff has gcsim's common Millennial key, which
//! Freedom-Sworn's shares.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const EM: ModKey = ModKey::new("elegy-em");
const PROC: ModKey = ModKey::new("elegy-proc");
pub const MILLENNIAL: ModKey = ModKey::new("millennial-atk%");
const ICD: ModKey = ModKey::new("elegy-sigil-icd");
const CD: ModKey = ModKey::new("elegy-cd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "elegyfortheend";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// EM +60 to 120.
    fn init(s: &mut Sim, ci: usize) {
        let em = 45.0 + s.chars[ci].refine as f64 * 15.0;
        s.chars[ci].mods.push(Mod::stat(EM, st::EM, em, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// A skill or burst hit by the holder, once every 0.2 s (hitlag): a
    /// sigil; at four, out of a 20 s cooldown (hitlag), every party member
    /// gets EM +100 to 200 and ATK +20 to 40% for 12 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst) {
            return;
        }
        if s.mod_active(ci, CD) || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 12, true);
        let st = s.weapon_st_mut::<State>(ci);
        st.stacks += 1;
        if st.stacks < 4 {
            return;
        }
        st.stacks = 0;
        s.add_status(ci, CD, 20 * 60, true);
        let r = s.chars[ci].refine as f64;
        for j in 0..s.chars.len() {
            s.add_mod(j, Mod::stat(PROC, st::EM, 75.0 + r * 25.0, false, true, -1), 12 * 60);
            s.add_mod(j, Mod::stat(MILLENNIAL, st::ATKP, 0.15 + r * 0.05, false, true, -1), 12 * 60);
        }
    }
}
