//! Freedom-Sworn, from gcsim's internal/weapons/sword/freedom.
//!
//! The party's ATK buff has gcsim's common Millennial key, which Elegy for
//! the End's shares.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const DMG: ModKey = ModKey::new("freedom-dmg");
const ATTACK: ModKey = ModKey::new("freedomsworn");
const ICD: ModKey = ModKey::new("freedom-sworn-sigil-icd");
const CD: ModKey = ModKey::new("freedom-sworn-cooldown");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "freedomsworn";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// DMG +10 to 20%.
    fn init(s: &mut Sim, ci: usize) {
        let dmg = 0.075 + s.chars[ci].refine as f64 * 0.025;
        s.chars[ci].mods.push(Mod::stat(DMG, st::DMGP, dmg, false, false, -1));
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    /// The party's normal, charged and plunging attack DMG +16 to 32%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            ev.stats[st::DMGP] += 0.12 + 0.04 * s.chars[ci].refine as f64;
        }
    }

    /// Any reaction event by the holder (gcsim's whole reaction range, on
    /// any target), once every 0.5 s (hitlag): a sigil; at two, out of a
    /// 20 s cooldown (hitlag), every party member gets ATK +20 to 40% and
    /// the attack bonus for 12 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(_, ev) = *e else { return };
        if ev.info.actor as usize != ci || s.mod_active(ci, CD) || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 30, true);
        let st = s.weapon_st_mut::<State>(ci);
        st.stacks += 1;
        if st.stacks < 2 {
            return;
        }
        st.stacks = 0;
        s.add_status(ci, CD, 20 * 60, true);
        let atk = 0.15 + s.chars[ci].refine as f64 * 0.05;
        for j in 0..s.chars.len() {
            s.add_mod(j, Mod::stat(super::elegyfortheend::MILLENNIAL, st::ATKP, atk, false, true, -1), 12 * 60);
            s.add_mod(j, Mod::owned(ATTACK, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 12 * 60);
        }
    }
}
