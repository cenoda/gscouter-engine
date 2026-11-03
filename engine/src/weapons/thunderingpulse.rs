//! Thundering Pulse, from gcsim's internal/weapons/bow/thundering.
//!
//! Its ATK bonus is part of an attack mod, as in gcsim: it counts in the
//! holder's attacks only.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("thundering-pulse");
const NORMAL: ModKey = ModKey::new("thundering-pulse-normal");
const SKILL: ModKey = ModKey::new("thundering-pulse-skill");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "thunderingpulse";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
        s.subscribe(EventKind::Skill, o);
    }

    /// ATK +20 to 40%; normal attack DMG +12 to 24% per emblem (energy not
    /// full, a damaging normal attack within 5 s, a skill within 10 s),
    /// 40 to 80% at three.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        let c = &s.chars[ci];
        let r = c.refine as f64;
        ev.stats[st::ATKP] += 0.15 + r * 0.05;
        if ev.info.tag != AttackTag::Normal {
            return;
        }
        let mut count = 0;
        if c.energy < c.energy_max {
            count += 1;
        }
        if s.mod_active(ci, NORMAL) {
            count += 1;
        }
        if s.mod_active(ci, SKILL) {
            count += 1;
        }
        ev.stats[st::DMGP] += if count >= 3 { 0.3 + r * 0.1 } else { count as f64 * (0.09 + r * 0.03) };
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize == ci && ev.info.tag == AttackTag::Normal && ev.damage != 0.0 {
                    s.add_status(ci, NORMAL, 300, true);
                }
            }
            Event::Skill => {
                if s.player.active as usize == ci {
                    s.add_status(ci, SKILL, 600, true);
                }
            }
            _ => {}
        }
    }
}
