//! gcsim's internal/weapons/common/wavebreaker, which Mouun's Moon,
//! Wavebreaker's Fin and Akuoumaru are.

use crate::sim::*;
use crate::types::*;

const KEY: ModKey = ModKey::new("wavebreaker");

/// On OnInitialize: burst DMG +0.12 to 0.24% per point of the party's
/// total max energy, at most 40 to 80%.
pub fn post_init(s: &mut Sim, ci: usize) -> f64 {
    let r = s.chars[ci].refine as f64;
    let per = 0.09 + 0.03 * r;
    let max_bonus = 0.3 + 0.1 * r;
    let energy: f64 = s.chars.iter().map(|c| c.energy_max).sum();
    let amt = (energy * per / 100.0).min(max_bonus);
    s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    amt
}

pub fn attack_mod(amt: f64, ev: &mut AttackEvent) {
    if ev.info.tag == AttackTag::ElementalBurst {
        ev.stats[st::DMGP] += amt;
    }
}
