//! Crimson Moon's Semblance, from gcsim's internal/weapons/spear/crimsonmoonssemblance.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("crimsonmoonssemblance-icd");
const BONUS: ModKey = ModKey::new("crimsonmoonssemblance-bonus");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "crimsonmoonssemblance";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(BONUS, ModKind::Stat, o, st::DMGP, false, true, -1));
        s.subscribe(EventKind::EnemyHit, o);
    }

    /// DMG +12 to 24% with a Bond of Life, 24 to 48% more while it is at
    /// least 30% of max HP.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let debt = s.hp_debt(ci);
        if debt > 0.0 {
            out[st::DMGP] += 0.08 + 0.04 * r;
        }
        if debt >= 0.3 * s.max_hp(ci) {
            out[st::DMGP] += 0.16 + 0.08 * r;
        }
        true
    }

    /// A charged attack hitting while the holder is on field (gcsim checks
    /// no attacker), once every 14 s (hitlag): a Bond of Life of 25% of max
    /// HP.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if ev.info.tag != AttackTag::Extra || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 14 * 60, true);
        s.modify_hp_debt_ratio(ci, 0.25);
    }
}
