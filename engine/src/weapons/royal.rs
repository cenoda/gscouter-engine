//! Shared by the Royal weapons.

use crate::sim::*;
use crate::types::*;

pub const BUFF: ModKey = ModKey::new("royal");
pub const ICD: ModKey = ModKey::new("royal-icd");

#[derive(Default)]
pub struct State {
    pub stacks: i32,
}

pub fn init(s: &mut Sim, ci: usize) {
    s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    s.chars[ci].mods.push(Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
}

pub fn on_enemy_damage(s: &mut Sim, ci: usize, ev: &AttackEvent, st: &mut State) {
    if ev.damage == 0.0 || ev.info.actor as usize != ci {
        return;
    }
    if ev.crit {
        st.stacks = 0;
    } else if !s.mod_active(ci, ICD) {
        st.stacks = (st.stacks + 1).min(5);
        s.add_status(ci, ICD, 18, true);
    }
}

pub fn mod_amount(s: &Sim, ci: usize, stacks: i32, out: &mut Stats) -> bool {
    out[st::CR] += (0.06 + 0.02 * s.chars[ci].refine as f64) * stacks as f64;
    true
}
