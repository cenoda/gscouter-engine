//! Amenoma Kageuchi, from gcsim's internal/weapons/sword/amenoma.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;

pub struct Weapon;
pub struct State;

const SEEDS: [ModKey; 3] = [ModKey::new("amenoma-seed-0"), ModKey::new("amenoma-seed-1"), ModKey::new("amenoma-seed-2")];
const ICD: ModKey = ModKey::new("amenoma-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "amenomakageuchi";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::Burst, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if s.player.active as usize != ci {
            return;
        }
        match *e {
            // a skill on field, once every 5 s (hitlag): a seed for 30 s
            // (hitlag), over the one that expires first
            Event::Skill => {
                if s.mod_active(ci, ICD) {
                    return;
                }
                let mut index = 0;
                for i in 0..3 {
                    if status_expiry(s, ci, SEEDS[i]) < status_expiry(s, ci, SEEDS[index]) {
                        index = i;
                    }
                }
                s.add_status(ci, SEEDS[index], 30 * 60, true);
                s.add_status(ci, ICD, 300, true);
            }
            // a burst on field takes the seeds, and 3 s later (hitlag)
            // restores 6 to 12 energy for each one active
            Event::Burst => {
                let mut count = 0;
                for k in SEEDS {
                    if s.mod_active(ci, k) {
                        count += 1;
                    }
                    s.delete_mod(ci, k);
                }
                if count == 0 {
                    return;
                }
                s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: 0, arg: count }, 180);
            }
            _ => {}
        }
    }

    /// arg: the seeds taken.
    fn task(s: &mut Sim, ci: usize, _: u8, arg: i32) {
        let refund = 4.5 + 1.5 * s.chars[ci].refine as f64;
        s.add_energy(ci, refund * arg as f64);
    }
}

/// gcsim's StatusExpiry: the mod's expiry, 0 when there is none.
fn status_expiry(s: &Sim, ci: usize, key: ModKey) -> i32 {
    s.chars[ci].mods.iter().find(|m| m.key == key).map_or(0, |m| m.expiry())
}
