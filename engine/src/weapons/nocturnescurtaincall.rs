//! Nocturne's Curtain Call, from gcsim's internal/weapons/catalyst/nocturnes.
//!
//! gcsim's OnSpecialReactionAttack subscription adds CRIT DMG to the
//! holder's lunar contributions while "nocturnes-curtain-call-buff" is
//! active, a status nothing adds (the buffs are "...-buff-hp" and
//! "...-buff-cd"), so it never does anything and is not ported.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const HP: ModKey = ModKey::new("nocturnes-curtain-call-hp");
const ICD: ModKey = ModKey::new("nocturnes-curtain-call-icd");
const BUFF_HP: ModKey = ModKey::new("nocturnes-curtain-call-buff-hp");
const BUFF_CD: ModKey = ModKey::new("nocturnes-curtain-call-buff-cd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "nocturnescurtaincall";
    type State = State;
    fn new_state() -> State {
        State
    }

    /// HP +10 to 18%.
    fn init(s: &mut Sim, ci: usize) {
        let hp = 0.08 + 0.02 * s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(HP, st::HPP, hp, false, false, -1));
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::EnemyDamage, o);
        s.subscribe(EventKind::Reaction, o);
    }

    /// Lunar DMG's CRIT DMG +60 to 120%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag.is_lunar() {
            ev.stats[st::CD] += 0.4 + 0.2 * s.chars[ci].refine as f64;
        }
    }

    /// Lunar damage by the holder, or a Lunar-Charged, Lunar-Bloom or
    /// Lunar-Crystallize it triggers: the buffs.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let ev = match *e {
            Event::EnemyDamage(ev) if ev.info.tag.is_lunar() => ev,
            Event::Reaction(Reaction::LunarCharged | Reaction::LunarBloom | Reaction::LunarCrystallize, ev) => ev,
            _ => return,
        };
        if ev.info.actor as usize != ci {
            return;
        }
        let r = s.chars[ci].refine as f64;
        // 14 to 18 energy, once every 18 s (hitlag)
        if !s.mod_active(ci, ICD) {
            s.add_energy(ci, 13.0 + r);
            s.add_status(ci, ICD, 18 * 60, true);
        }
        // HP +14 to 22% and the lunar CRIT DMG for 12 s (hitlag)
        s.add_mod(ci, Mod::stat(BUFF_HP, st::HPP, 0.12 + 0.02 * r, false, true, -1), 12 * 60);
        s.add_mod(ci, Mod::owned(BUFF_CD, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 12 * 60);
    }
}
