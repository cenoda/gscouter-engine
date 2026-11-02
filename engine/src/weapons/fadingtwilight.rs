//! Fading Twilight, from gcsim's internal/weapons/bow/twilight.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    /// The state of the cycle, 0 to 2 (Evengleam, Afterglow, Dawnblaze).
    cycle: i32,
}

const BONUS: ModKey = ModKey::new("twilight-bonus-dmg");
const ICD: ModKey = ModKey::new("twilight-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "fadingtwilight";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(BONUS, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
    }

    /// DMG +6 to 12%, +10 to 20% or +14 to 28% by the state.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        let r = s.chars[ci].refine as f64;
        ev.stats[st::DMGP] += match s.weapon_st::<State>(ci).cycle {
            2 => 0.105 + r * 0.035,
            1 => 0.075 + r * 0.025,
            _ => 0.045 + r * 0.015,
        };
    }

    /// The holder's damage moves the cycle on, once every 7 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 420, true);
        let st = s.weapon_st_mut::<State>(ci);
        st.cycle = (st.cycle + 1) % 3;
    }
}
