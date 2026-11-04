//! Ferrous Shadow, from gcsim's internal/weapons/claymore/ferrousshadow.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("ferrousshadow");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "ferrousshadow";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        let r = s.chars[ci].refine as f64;
        let hp_check = 0.65 + r * 0.05;
        if ev.info.tag == AttackTag::Extra && s.current_hp(ci) / s.max_hp(ci) <= hp_check {
            ev.stats[st::DMGP] += 0.25 + r * 0.05;
        }
    }
}
