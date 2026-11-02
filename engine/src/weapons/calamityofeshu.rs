//! Calamity of Eshu, from gcsim's internal/weapons/sword/calamityofeshu.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("calamityofeshu");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "calamityofeshu";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// While the holder is shielded: normal and charged attacks DMG +20 to
    /// 40% and CRIT Rate +8 to 16%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if !s.is_shielded(ci) || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        let r = s.chars[ci].refine as f64;
        ev.stats[st::DMGP] += 0.15 + 0.05 * r;
        ev.stats[st::CR] += 0.06 + 0.02 * r;
    }
}
