//! Iron Sting, from gcsim's internal/weapons/sword/ironsting.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
    /// gcsim's buff, updated only while stacks rise below 2.
    dmg: f64,
}

const BUFF: ModKey = ModKey::new("ironsting");
const ICD: ModKey = ModKey::new("ironsting-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "ironsting";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::DMGP] += s.weapon_st::<State>(ci).dmg;
        true
    }

    /// The holder's non-physical damage on field, once a second (hitlag):
    /// DMG +6 to 12% for 6 s, up to 2 stacks.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if s.player.active as usize != ci || ev.info.actor as usize != ci || ev.info.element == Element::Physical || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 60, true);
        let active = s.mod_active(ci, BUFF);
        let r = s.chars[ci].refine as f64;
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        if st.stacks < 2 {
            st.stacks += 1;
            st.dmg = (0.045 + 0.015 * r) * st.stacks as f64;
        }
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 360);
    }
}
