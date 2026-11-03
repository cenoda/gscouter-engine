//! Luxurious Sea-Lord, from gcsim's internal/weapons/claymore/sealord.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("luxurious-sea-lord");
const ICD: ModKey = ModKey::new("sealord-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "luxurioussealord";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
    }

    /// Burst DMG +12 to 24%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::ElementalBurst {
            ev.stats[st::DMGP] += 0.09 + 0.03 * s.chars[ci].refine as f64;
        }
    }

    /// A burst hit by the holder on field, once every 15 s (hitlag): a
    /// tuna for 100 to 200% of ATK as physical damage on the target.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) || ev.info.tag != AttackTag::ElementalBurst {
            return;
        }
        s.add_status(ci, ICD, 900, true);
        let mult = 0.75 + 0.25 * s.chars[ci].refine as f64;
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, mult);
        ai.abil = "Luxurious Sea-Lord Proc";
        ai.pattern = s.circle_on_target(3.0);
        s.queue_attack(ai, 0, 0, 1);
    }
}
