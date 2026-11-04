//! The Bell, from gcsim's internal/weapons/claymore/bell.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("bell");
const ICD: ModKey = ModKey::new("bell-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "thebell";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Drain, Owner::Weapon(ci as u8));
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// Damage from outside (to any member: gcsim does not check whom),
    /// once every 45 s (hitlag): a shield of 20 to 32% of the holder's max
    /// HP on the holder for 10 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Drain { amount, external, .. } = *e else { return };
        if !external || amount <= 0.0 || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 2700, true);
        let hp = (0.17 + s.chars[ci].refine as f64 * 0.03) * s.max_hp(ci);
        let f = s.f;
        s.add_shield(Shield { owner: ci as u8, key: KEY, typ: ShieldType::Bell, target: ci as i8, ele: Element::NoElement, hp, expiry: f + 600, em_bonus: 0.0 });
    }

    /// DMG +12 to 24% while the holder is shielded.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if !s.is_shielded(ci) {
            return false;
        }
        out[st::DMGP] += 0.09 + s.chars[ci].refine as f64 * 0.03;
        true
    }
}
