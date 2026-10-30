//! Staff of Homa, from gcsim's internal/weapons/spear/homa.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const HP: ModKey = ModKey::new("homa-hp");
const ATK: ModKey = ModKey::new("homa-atk-buff");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "staffofhoma";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(HP, st::HPP, 0.15 + r * 0.05, false, false, -1));
        s.chars[ci].mods.push(Mod::owned(ATK, ModKind::Stat, Owner::Weapon(ci as u8), st::ATK, true, false, -1));
    }

    fn mod_amount(s: &Sim, ci: usize, _on: usize, _m: &Mod, out: &mut Stats) -> bool {
        let c = &s.chars[ci];
        let maxhp = s.max_hp(ci);
        let mut per = 0.006 + c.refine as f64 * 0.002;
        if c.hp_ratio <= 0.5 {
            per += 0.008 + c.refine as f64 * 0.002;
        }
        out[st::ATK] += per * maxhp;
        true
    }
}
