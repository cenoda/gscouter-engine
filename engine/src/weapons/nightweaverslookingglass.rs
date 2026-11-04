//! Nightweaver's Looking Glass, from gcsim's
//! internal/weapons/catalyst/nightweavers. The holder's skill dealing
//! hydro or dendro damage to the enemy gives Prayer of the Far North (EM
//! +60 to 120 for 4.5 s, hitlag); any Lunar-Bloom on the enemy gives New
//! Moon Verse (EM +60 to 120 for 10 s, hitlag). While both are active,
//! every party member's bloom (Bountiful Core's too, which gcsim tags as
//! bloom) DMG is increased by 3 times 40 to 80%, hyperbloom and burgeon by
//! twice that and direct Lunar-Bloom by that (a ReactBonusMod on each
//! member, added in gcsim's Weapon.Init; a second holder's replaces it, as
//! in gcsim).

use crate::content::{Event, Owner, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const REACT: ModKey = ModKey::new("nightweavers");
const PRAYER: ModKey = ModKey::new("prayer-of-the-far-north");
const NEW_MOON: ModKey = ModKey::new("new-moon-verse");

fn em(s: &Sim, ci: usize) -> f64 {
    45.0 + s.chars[ci].refine as f64 * 15.0
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "nightweaverslookingglass";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::EnemyDamage, o);
        s.subscribe(EventKind::Reaction, o);
    }

    fn post_init(s: &mut Sim, ci: usize) {
        for x in 0..s.chars.len() {
            s.add_mod(x, Mod::owned(REACT, ModKind::ReactBonus, Owner::Weapon(ci as u8), st::NONE, false, false, -1), -1);
        }
    }

    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if !s.mod_active(ci, PRAYER) || !s.mod_active(ci, NEW_MOON) {
            return 0.0;
        }
        let buff = 0.3 + s.chars[ci].refine as f64 * 0.1;
        match info.tag {
            AttackTag::Bloom => buff * 3.0,
            AttackTag::Hyperbloom | AttackTag::Burgeon => buff * 2.0,
            AttackTag::DirectLunarBloom => buff,
            _ => 0.0,
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyDamage(ev) => {
                if !ev.on_enemy() || ev.info.actor as usize != ci {
                    return;
                }
                if !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) || !matches!(ev.info.element, Element::Hydro | Element::Dendro) {
                    return;
                }
                let v = em(s, ci);
                s.add_mod(ci, Mod::stat(PRAYER, st::EM, v, false, true, -1), 270);
            }
            Event::Reaction(Reaction::LunarBloom, ev) => {
                if !ev.on_enemy() {
                    return;
                }
                let v = em(s, ci);
                s.add_mod(ci, Mod::stat(NEW_MOON, st::EM, v, false, true, -1), 600);
            }
            _ => {}
        }
    }
}
