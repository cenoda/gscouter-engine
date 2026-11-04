//! Polar Star, from gcsim's internal/weapons/bow/polarstar.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ATK: ModKey = ModKey::new("polar-star-atk");
const DMG: ModKey = ModKey::new("polar-star-dmg");
const NORMAL: ModKey = ModKey::new("polar-star-normal");
const CHARGED: ModKey = ModKey::new("polar-star-charged");
const SKILL: ModKey = ModKey::new("polar-star-skill");
const BURST: ModKey = ModKey::new("polar-star-burst");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "polarstar";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(ATK, ModKind::Stat, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
        s.chars[ci].mods.push(Mod::owned(DMG, ModKind::Attack, o, st::NONE, false, false, -1));
    }

    /// Ashen Nightstar: ATK +10 to 20% a stack, and 6 to 12% more at four.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let count = [NORMAL, CHARGED, SKILL, BURST].iter().filter(|&&k| s.mod_active(ci, k)).count();
        let mut bonus = (0.075 + r * 0.025) * count as f64;
        if count >= 4 {
            bonus += 0.06 + r * 0.02;
        }
        out[st::ATKP] += bonus;
        true
    }

    /// Skill and burst DMG +12 to 24%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst) {
            ev.stats[st::DMGP] += 0.09 + s.chars[ci].refine as f64 * 0.03;
        }
    }

    /// A normal, charged, skill or burst hit by the holder on field gives
    /// that kind's stack for 12 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        let key = match ev.info.tag {
            AttackTag::Normal => NORMAL,
            AttackTag::Extra => CHARGED,
            AttackTag::ElementalArt | AttackTag::ElementalArtHold => SKILL,
            AttackTag::ElementalBurst => BURST,
            _ => return,
        };
        s.add_status(ci, key, 720, true);
    }
}
