//! Shared by the Blackcliff weapons.

use crate::sim::*;
use crate::types::*;

pub const BUFF: ModKey = ModKey::new("blackcliff");
pub const STACKS: [ModKey; 3] = [
    ModKey::new("blackcliff-stack-1"),
    ModKey::new("blackcliff-stack-2"),
    ModKey::new("blackcliff-stack-3"),
];

#[derive(Default)]
pub struct State {
    pub index: usize,
}

pub fn init(s: &mut Sim, ci: usize) {
    s.subscribe(EventKind::TargetDied, Owner::Weapon(ci as u8));
}

pub fn on_target_died(s: &mut Sim, ci: usize, ev: &AttackEvent, st: &mut State) {
    if ev.info.actor as usize != ci || s.player.active as usize != ci {
        return;
    }
    s.add_status(ci, STACKS[st.index], 1800, true);
    s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::ATKP, false, true, -1), 1800);
    st.index = (st.index + 1) % 3;
}

pub fn mod_amount(s: &Sim, ci: usize, out: &mut Stats) -> bool {
    let count = STACKS.iter().filter(|&&k| s.mod_active(ci, k)).count() as f64;
    out[st::ATKP] += (0.09 + 0.03 * s.chars[ci].refine as f64) * count;
    true
}
