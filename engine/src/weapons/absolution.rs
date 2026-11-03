//! Absolution, from gcsim's internal/weapons/sword/absolution.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const CD: ModKey = ModKey::new("absolution-crit-dmg");
const BONUS: ModKey = ModKey::new("absolution-dmg-bonus");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "absolution";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// CRIT DMG +20 to 40%.
    fn init(s: &mut Sim, ci: usize) {
        let cd = 0.15 + 0.05 * s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(CD, st::CD, cd, false, false, -1));
        s.subscribe(EventKind::HpDebt, Owner::Weapon(ci as u8));
    }

    /// DMG +16 to 32% per stack.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        ev.stats[st::DMGP] += (0.12 + 0.04 * s.chars[ci].refine as f64) * s.weapon_st::<State>(ci).stacks as f64;
    }

    /// The holder's Bond of Life growing: a stack (up to three, from none
    /// once the buff is gone), the buff for 6 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::HpDebt { on, change } = *e else { return };
        if on != ci || change >= 0.0 {
            return;
        }
        if !s.mod_active(ci, BONUS) {
            s.weapon_st_mut::<State>(ci).stacks = 0;
        }
        let st = s.weapon_st_mut::<State>(ci);
        if st.stacks < 3 {
            st.stacks += 1;
        }
        s.add_mod(ci, Mod::owned(BONUS, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 6 * 60);
    }
}
