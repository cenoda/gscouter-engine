//! The Black Sword, from gcsim's internal/weapons/sword/blacksword.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("blacksword");
const ICD: ModKey = ModKey::new("black-sword-icd");

fn normal_or_charged(t: AttackTag) -> bool {
    matches!(t, AttackTag::Normal | AttackTag::Extra)
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "theblacksword";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
    }

    /// Normal and charged attack DMG +20 to 40%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if normal_or_charged(ev.info.tag) {
            ev.stats[st::DMGP] += 0.15 + 0.05 * s.chars[ci].refine as f64;
        }
    }

    /// A crit by the holder's normal or charged attack on field heals it
    /// for 60 to 100% of the attack's ATK, once every 5 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || !normal_or_charged(ev.info.tag) || s.player.active as usize != ci || s.mod_active(ci, ICD) || !ev.crit {
            return;
        }
        let heal = (0.5 + 0.1 * s.chars[ci].refine as f64) * total_atk(&ev.stats);
        let bonus = s.stat(ci, st::HEAL);
        s.heal(ci, ci, heal, bonus);
        s.add_status(ci, ICD, 300, true);
    }
}
