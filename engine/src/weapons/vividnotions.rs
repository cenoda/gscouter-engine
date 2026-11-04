//! Vivid Notions, from gcsim's internal/weapons/catalyst/vividnotions: ATK
//! +28 to 56%. Entering the plunge state (any character's, as gcsim
//! subscribes OnStateChange without checking who) gives the holder Dawn's
//! First Hue for 15 s (hitlag); any character's skill or burst gives it
//! Twilight's Splendor for 15 s (hitlag). The holder's plunges crit for
//! +28 to 56% CRIT DMG with the first and +40 to 80% with the second. A
//! plunge hit by the holder with durability removes, 0.1 s later (on its
//! queue), each of them that has not been renewed or extended since.

use crate::content::{Event, Owner, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    /// The two statuses' expiries at each pending removal, a ring the
    /// task's arg indexes.
    pending: [(i32, i32); 8],
    next: usize,
}

const ATK: ModKey = ModKey::new("vividnotions-atk");
const CD: ModKey = ModKey::new("vividnotions-cd");
const PLUNGE: ModKey = ModKey::new("dawns-first-hue");
const SKILL_BURST: ModKey = ModKey::new("twilights-splendor");

/// gcsim's StatusExpiry: the expiry of the status, 0 when it is absent.
fn status_expiry(s: &Sim, ci: usize, key: ModKey) -> i32 {
    let c = &s.chars[ci];
    c.mods.iter().find(|m| m.key == key && !c.gone(m)).map_or(0, |m| m.expiry())
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "vividnotions";
    type State = State;
    fn new_state() -> State {
        State { pending: [(0, 0); 8], next: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(ATK, st::ATKP, 0.21 + r * 0.07, false, false, -1));
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(CD, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::StateChange, o);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::Burst, o);
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag != AttackTag::Plunge {
            return;
        }
        let r = s.chars[ci].refine as f64;
        if s.mod_active(ci, PLUNGE) {
            ev.stats[st::CD] += 0.21 + r * 0.07;
        }
        if s.mod_active(ci, SKILL_BURST) {
            ev.stats[st::CD] += 0.3 + r * 0.1;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::StateChange(AnimState::Plunge) => s.add_status(ci, PLUNGE, 15 * 60, true),
            Event::Skill | Event::Burst => s.add_status(ci, SKILL_BURST, 15 * 60, true),
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize != ci || ev.info.tag != AttackTag::Plunge || ev.info.durability == 0.0 {
                    return;
                }
                let exp = (status_expiry(s, ci, PLUNGE), status_expiry(s, ci, SKILL_BURST));
                let st = s.weapon_st_mut::<State>(ci);
                let i = st.next;
                st.pending[i] = exp;
                st.next = (i + 1) % st.pending.len();
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: 0, arg: i as i32 }, 6);
            }
            _ => {}
        }
    }

    fn task(s: &mut Sim, ci: usize, _: u8, arg: i32) {
        let (plunge_f, skill_burst_f) = s.weapon_st::<State>(ci).pending[arg as usize];
        if plunge_f == status_expiry(s, ci, PLUNGE) {
            s.delete_mod(ci, PLUNGE);
        }
        if skill_burst_f == status_expiry(s, ci, SKILL_BURST) {
            s.delete_mod(ci, SKILL_BURST);
        }
    }
}
