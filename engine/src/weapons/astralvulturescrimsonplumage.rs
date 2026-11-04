//! Astral Vulture's Crimson Plumage, from gcsim's
//! internal/weapons/bow/astralvulturescrimsonplumage.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    /// The burst bonus from the members of another element, set by Init
    /// (0 when none); charged attacks get twice it.
    dmg: f64,
}

const DMG: ModKey = ModKey::new("astralvulturescrimsonplumage-dmg");
const ATK: ModKey = ModKey::new("astralvulturescrimsonplumage-atkp");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "astralvulturescrimsonplumage";
    type State = State;
    fn new_state() -> State {
        State { dmg: 0.0 }
    }

    /// gcsim's OnSwirlHydro to OnSwirlPyro (not Stellar Swirl).
    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    /// Charged attack DMG +2x and burst DMG +x, with x 10 to 20% for one
    /// member of another element, 2.4 times that for two or more.
    fn post_init(s: &mut Sim, ci: usize) {
        let el = s.chars[ci].element;
        let counter = s.chars.iter().filter(|c| c.element != el).count();
        if counter == 0 {
            return;
        }
        let mut dmg = 0.025 * s.chars[ci].refine as f64 + 0.075;
        if counter >= 2 {
            dmg *= 2.4;
        }
        s.weapon_st_mut::<State>(ci).dmg = dmg;
        s.chars[ci].mods.push(Mod::owned(DMG, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        let dmg = s.weapon_st::<State>(ci).dmg;
        match ev.info.tag {
            AttackTag::Extra => ev.stats[st::DMGP] += dmg * 2.0,
            AttackTag::ElementalBurst => ev.stats[st::DMGP] += dmg,
            _ => {}
        }
    }

    /// A swirl by the holder on field (on any target, as gcsim does not
    /// check): ATK +24 to 48% for 12 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(Reaction::SwirlHydro | Reaction::SwirlCryo | Reaction::SwirlElectro | Reaction::SwirlPyro, ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        let v = 0.06 * s.chars[ci].refine as f64 + 0.18;
        s.add_mod(ci, Mod::stat(ATK, st::ATKP, v, false, true, -1), 12 * 60);
    }
}
