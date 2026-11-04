//! Fractured Halo, from gcsim's internal/weapons/spear/fracturedhalo.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ATK: ModKey = ModKey::new("fracturedhalo-atkp");
const LC: ModKey = ModKey::new("fracturedhalo-lc-buff");
/// One key for every holder's mod, as in gcsim: gcsim's NewWeapon adds it
/// to the characters added so far (the holder and those before it), and a
/// later holder's replaces an earlier one's.
const LC_DMG: ModKey = ModKey::new("fracturedhalo-lc-dmg");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "fracturedhalo";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        for c in 0..=ci {
            mod_add(&mut s.chars[c].mods, Mod::owned(LC_DMG, ModKind::ReactBonus, o, st::NONE, false, false, -1));
        }
        s.subscribe(EventKind::Burst, o);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::Shielded, o);
    }

    /// While Electrifying Edict lasts: the party's Lunar-Charged DMG
    /// (direct or not) +40 to 80%.
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if !matches!(info.tag, AttackTag::DirectLunarCharged | AttackTag::ReactionLunarCharge) || !s.mod_active(ci, LC) {
            return 0.0;
        }
        0.30 + 0.10 * s.chars[ci].refine as f64
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // a skill or burst on field: ATK +24 to 48% for 20 s (hitlag)
            Event::Skill | Event::Burst => {
                if s.player.active as usize == ci {
                    let atk = 0.18 + 0.06 * s.chars[ci].refine as f64;
                    s.add_mod(ci, Mod::stat(ATK, st::ATKP, atk, false, true, -1), 20 * 60);
                }
            }
            // a shield of the holder's while it lasts: Electrifying Edict
            // for 20 s (hitlag)
            Event::Shielded(sh) => {
                if sh.owner as usize == ci && s.mod_active(ci, ATK) {
                    s.add_status(ci, LC, 20 * 60, true);
                }
            }
            _ => {}
        }
    }
}
