//! Reliquary of Truth, from gcsim's internal/weapons/catalyst/reliquary.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const CR_PERM: ModKey = ModKey::new("reliquary-cr");
const SECRET_OF_LIES: ModKey = ModKey::new("reliquary-secretoflies");
const MOON_OF_TRUTH: ModKey = ModKey::new("reliquary-moonoftruth");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "reliquaryoftruth";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(CR_PERM, st::CR, 0.06 + 0.02 * r, false, false, -1));
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let both = s.mod_active(ci, SECRET_OF_LIES) && s.mod_active(ci, MOON_OF_TRUTH);
        let mult = if both { 1.5 } else { 1.0 };
        if m.key == SECRET_OF_LIES {
            out[st::EM] += (60.0 + 20.0 * r) * mult;
            return true;
        }
        if m.key == MOON_OF_TRUTH {
            out[st::CD] += (0.18 + 0.06 * r) * mult;
            return true;
        }
        false
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let o = Owner::Weapon(ci as u8);
        match *e {
            Event::Skill => {
                if s.player.active as usize == ci {
                    s.add_mod(ci, Mod::owned(SECRET_OF_LIES, ModKind::Stat, o, st::NONE, false, true, -1), 12 * 60);
                }
            }
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize == ci && ev.info.tag == AttackTag::DirectLunarBloom {
                    s.add_mod(ci, Mod::owned(MOON_OF_TRUTH, ModKind::Stat, o, st::NONE, false, true, -1), 4 * 60);
                }
            }
            _ => {}
        }
    }
}
