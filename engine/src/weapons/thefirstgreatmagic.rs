//! The First Great Magic, from gcsim's internal/weapons/bow/firstgreatmagic.
//! Theatrics (movement speed) is not implemented in gcsim either.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const DMG: ModKey = ModKey::new("thefirstgreatmagic-dmg%");
const ATK: ModKey = ModKey::new("thefirstgreatmagic-atk");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "thefirstgreatmagic";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(DMG, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// Charged attack DMG +16 to 32%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::Extra {
            ev.stats[st::DMGP] += 0.12 + s.chars[ci].refine as f64 * 0.04;
        }
    }

    /// Gimmick: ATK +16 to 32% for each member of the holder's element
    /// (the holder too), up to three.
    fn post_init(s: &mut Sim, ci: usize) {
        let el = s.chars[ci].element;
        let same = s.chars.iter().filter(|c| c.element == el).count().min(3);
        let v = (0.12 + s.chars[ci].refine as f64 * 0.04) * same as f64;
        s.chars[ci].mods.push(Mod::stat(ATK, st::ATKP, v, false, false, -1));
    }
}
