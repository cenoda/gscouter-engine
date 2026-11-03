//! Primordial Jade Cutter, from gcsim's internal/weapons/sword/primordial.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const HP: ModKey = ModKey::new("jadecutter-hp");
const ATK: ModKey = ModKey::new("jadecutter-atk-buff");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "primordialjadecutter";
    type State = State;
    fn new_state() -> State {
        State
    }

    /// HP +20 to 40%, and flat ATK of 1.2 to 2.4% of max HP.
    fn init(s: &mut Sim, ci: usize) {
        let hp = 0.15 + s.chars[ci].refine as f64 * 0.05;
        s.chars[ci].mods.push(Mod::stat(HP, st::HPP, hp, false, false, -1));
        s.chars[ci].mods.push(Mod::owned(ATK, ModKind::Stat, Owner::Weapon(ci as u8), st::ATK, false, false, -1));
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::ATK] += (0.009 + s.chars[ci].refine as f64 * 0.003) * s.max_hp(ci);
        true
    }
}
