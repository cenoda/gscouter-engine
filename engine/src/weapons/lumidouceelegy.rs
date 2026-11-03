//! Lumidouce Elegy, from gcsim's internal/weapons/spear/lumidouceelegy.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const ATK: ModKey = ModKey::new("lumidouceelegy-atk-buff");
const BONUS: ModKey = ModKey::new("lumidouceelegy-bonus-buff");
const ENERGY_ICD: ModKey = ModKey::new("lumidouceelegy-energy-icd");

/// A stack (up to 2, counted again from 0 once the buff has gone); at two,
/// 12 to 16 energy once every 12 s (hitlag); the buff for 8 s (hitlag).
fn bonus_cb(s: &mut Sim, ci: usize) {
    let active = s.mod_active(ci, BONUS);
    let st = s.weapon_st_mut::<State>(ci);
    if !active {
        st.stacks = 0;
    }
    if st.stacks < 2 {
        st.stacks += 1;
    }
    if st.stacks == 2 && !s.mod_active(ci, ENERGY_ICD) {
        s.add_status(ci, ENERGY_ICD, 12 * 60, true);
        let amt = s.chars[ci].refine as f64 + 11.0;
        s.add_energy(ci, amt);
    }
    s.add_mod(ci, Mod::owned(BONUS, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 8 * 60);
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "lumidouceelegy";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// ATK +15 to 31%.
    fn init(s: &mut Sim, ci: usize) {
        let atk = 0.11 + 0.04 * s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(ATK, st::ATKP, atk, false, false, -1));
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Reaction, o);
        s.subscribe(EventKind::EnemyDamage, o);
    }

    /// DMG +18 to 38% per stack.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        let r = s.chars[ci].refine as f64;
        ev.stats[st::DMGP] += (0.05 * r + 0.13) * s.weapon_st::<State>(ci).stacks as f64;
    }

    /// Burning set off by the holder on the enemy, or the holder's dendro
    /// damage to a burning enemy: a stack.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Reaction(Reaction::Burning, ev) if ev.on_enemy() && ev.info.actor as usize == ci => bonus_cb(s, ci),
            Event::EnemyDamage(ev) if s.enemy_burning() && ev.info.element == Element::Dendro && ev.info.actor as usize == ci => bonus_cb(s, ci),
            _ => {}
        }
    }
}
