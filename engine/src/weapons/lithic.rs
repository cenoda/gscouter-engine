//! gcsim's internal/weapons/common/lithic, which Lithic Spear and Lithic
//! Blade are: on OnInitialize, CRIT Rate +3 to 7% and ATK +7 to 11% for
//! each Liyue member (Zibai's zone counting), the holder included.

use crate::sim::*;
use crate::types::*;

const KEY: ModKey = ModKey::new("lithic");
/// gcsim's ZoneLiyue and ZoneNodKraiZibai.
const ZONE_LIYUE: u8 = 2;
const ZONE_NODKRAI_ZIBAI: u8 = 9;

pub fn init(s: &mut Sim, ci: usize) {
    s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
}

/// The number of stacks.
pub fn post_init(s: &Sim) -> i32 {
    s.chars
        .iter()
        .filter(|c| crate::data::CHARS.iter().any(|d| d.key == crate::gcsl::char_name(c.key) && matches!(d.zone, ZONE_LIYUE | ZONE_NODKRAI_ZIBAI)))
        .count() as i32
}

pub fn mod_amount(s: &Sim, ci: usize, stacks: i32, out: &mut Stats) {
    let r = s.chars[ci].refine as f64;
    out[st::CR] += (0.02 + r * 0.01) * stacks as f64;
    out[st::ATKP] += (0.06 + r * 0.01) * stacks as f64;
}
