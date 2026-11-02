//! Disenchantment in Deep Shadow, from gcsim's
//! internal/artifacts/disenchantmentindeepshadow.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("disenchantment-2pc");
const DMG: ModKey = ModKey::new("disenchantment-4pc-dmg");
const CR: ModKey = ModKey::new("disenchantment-4pc-cr");
const SUPERCONDUCT_SHRED: ModKey = ModKey::new("superconduct-phys-shred");
const STELLAR_SHRED: ModKey = ModKey::new("polestar-field-phys-shred");

/// The enemy's physical shred from superconduct or Stellar-Conduct.
fn shredded(s: &Sim) -> bool {
    s.enemy_status_active(SUPERCONDUCT_SHRED) || s.enemy_status_active(STELLAR_SHRED)
}

impl SetImpl for Set {
    const KEY: &'static str = "disenchantmentindeepshadow";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        }
        if count >= 4 {
            let o = Owner::Set(ci as u8, si as u8);
            s.chars[ci].mods.push(Mod::owned(DMG, ModKind::ReactBonus, o, st::NONE, false, false, -1));
            s.chars[ci].mods.push(Mod::owned(CR, ModKind::Attack, o, st::NONE, false, false, -1));
            s.subscribe(EventKind::SpecialReactionAttack, o);
        }
    }

    /// Superconduct DMG +80%, Stellar-Conduct DMG +40%.
    fn react_bonus(_: &Sim, _: usize, _: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        match info.tag {
            AttackTag::SuperconductDamage => 0.8,
            AttackTag::DirectStellarConduct => 0.4,
            _ => 0.0,
        }
    }

    /// CRIT Rate +16% against an enemy with either shred.
    fn attack_mod(s: &Sim, _: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if shredded(s) {
            ev.stats[st::CR] += 0.16;
        }
    }

    /// The same on the wearer's own lunar contributions.
    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let Event::SpecialReactionAttack(ev) = *e else { return };
        if ev.info.actor as usize == ci && shredded(s) {
            s.special_atk.stats[st::CR] += 0.16;
        }
    }
}
