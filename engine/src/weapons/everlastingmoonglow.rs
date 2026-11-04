//! Everlasting Moonglow, from gcsim's internal/weapons/catalyst/moonglow:
//! Healing Bonus +10 to 20%; the holder's normal attacks deal 1 to 3% of
//! max HP more (on OnEnemyHit, as flat damage); for 12 s (hitlag) after a
//! burst on field, the holder's normal attacks that hit restore 0.6 energy,
//! once every 0.1 s (hitlag).

use crate::content::{Event, Owner, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const HEAL: ModKey = ModKey::new("moonglow-heal-bonus");
const BUFF: ModKey = ModKey::new("moonglow-postburst");
const ICD: ModKey = ModKey::new("moonglow-energy-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "everlastingmoonglow";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(HEAL, st::HEAL, 0.075 + r * 0.025, false, false, -1));
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::EnemyHit, o);
        s.subscribe(EventKind::Burst, o);
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyHit(ev) => {
                if ev.info.actor as usize != ci || ev.info.tag != AttackTag::Normal {
                    return;
                }
                let nabuff = 0.005 + s.chars[ci].refine as f64 * 0.005;
                s.hit_flat += s.max_hp(ci) * nabuff;
            }
            Event::Burst => {
                if s.player.active as usize == ci {
                    s.add_status(ci, BUFF, 720, true);
                }
            }
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize != ci || ev.info.tag != AttackTag::Normal {
                    return;
                }
                if !s.mod_active(ci, BUFF) || s.mod_active(ci, ICD) {
                    return;
                }
                s.add_energy(ci, 0.6);
                s.add_status(ci, ICD, 6, true);
            }
            _ => {}
        }
    }
}
