//! Fruitful Hook, from gcsim's internal/weapons/claymore/fruitfulhook.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const CR_BUFF: ModKey = ModKey::new("fruitful-hook-cr");
const DMG_BUFF: ModKey = ModKey::new("fruitful-hook-dmg%");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "fruitfulhook";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(CR_BUFF, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        let r = s.chars[ci].refine as f64;
        let bonus = 0.12 + 0.04 * r;
        if m.key == CR_BUFF && ev.info.tag == AttackTag::Plunge {
            ev.stats[st::CR] += bonus;
        } else if m.key == DMG_BUFF && matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            ev.stats[st::DMGP] += bonus;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || ev.info.tag != AttackTag::Plunge {
            return;
        }
        s.add_mod(ci, Mod::owned(DMG_BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 10 * 60);
    }
}
