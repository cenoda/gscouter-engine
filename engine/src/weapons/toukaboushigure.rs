//! Toukabou Shigure, from gcsim's internal/weapons/sword/toukaboushigure.
//!
//! The Cursed Parasol is a status on the enemy under one key, which every
//! holder shares, as in gcsim.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("toukaboushigure");
const ICD: ModKey = ModKey::new("toukaboushigure-icd");
const DEBUFF: ModKey = ModKey::new("toukaboushigure-cursed-parasol");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "toukaboushigure";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
        s.subscribe(EventKind::TargetDied, o);
    }

    /// DMG +16 to 32% against an enemy under the Cursed Parasol.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if s.enemy_status_active(DEBUFF) {
            ev.stats[st::DMGP] += 0.12 + 0.04 * s.chars[ci].refine as f64;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // the holder's damage, once every 15 s (hitlag): the Cursed
            // Parasol on the enemy for 10 s
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize != ci || s.mod_active(ci, ICD) {
                    return;
                }
                s.add_status(ci, ICD, 15 * 60, true);
                s.enemy_add_status(DEBUFF, 10 * 60, true);
            }
            // the enemy dying under it ends the cooldown (in damage mode a
            // kill ends the fight)
            Event::TargetDied(_) => {
                if s.enemy_status_active(DEBUFF) && s.mod_active(ci, ICD) {
                    s.delete_mod(ci, ICD);
                }
            }
            _ => {}
        }
    }
}
