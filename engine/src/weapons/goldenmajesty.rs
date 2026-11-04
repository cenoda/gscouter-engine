//! Shared by the Golden Majesty series (Memory of Dust, Summit Shaper,
//! The Unforged, Vortex Vanquisher).

use crate::sim::*;
use crate::types::*;

pub const BUFF: ModKey = ModKey::new("golden-majesty");
pub const ICD: ModKey = ModKey::new("golden-majesty-icd");

#[derive(Default)]
pub struct State {
    pub stacks: i32,
}

pub fn init(s: &mut Sim, ci: usize) {
    s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
}

pub fn on_enemy_damage(s: &mut Sim, ci: usize, ev: &AttackEvent, st: &mut State) {
    if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
        return;
    }
    s.add_status(ci, ICD, 18, true);
    if !s.mod_active(ci, BUFF) {
        st.stacks = 0;
    }
    st.stacks = (st.stacks + 1).min(5);
    s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 480);
}

pub fn mod_amount(s: &Sim, ci: usize, stacks: i32, out: &mut Stats) -> bool {
    let mult = if s.is_shielded(ci) { 2.0 } else { 1.0 };
    out[st::ATKP] += (0.03 + 0.01 * s.chars[ci].refine as f64) * stacks as f64 * mult;
    true
}
