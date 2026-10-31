//! Wandering Evenstar, from gcsim's
//! internal/weapons/catalyst/wanderingevenstar.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    /// gcsim's "wanderingevenstar-<holder>", the party's buff.
    team_key: ModKey,
}

const KEY: ModKey = ModKey::new("wanderingevenstar");

const TASK_UPDATE: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "wanderingevenstar";
    type State = State;
    fn new_state() -> State {
        State { team_key: KEY }
    }

    fn init(s: &mut Sim, ci: usize) {
        let name = format!("wanderingevenstar-{}", crate::gcsl::char_name(s.chars[ci].key));
        s.weapon_st_mut::<State>(ci).team_key = ModKey::named(name);
    }

    fn post_init(s: &mut Sim, ci: usize) {
        update(s, ci);
    }

    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        update(s, ci);
    }
}

/// Every 10 s: ATK from EM for 12 s, and 30% of it to the party.
fn update(s: &mut Sim, ci: usize) {
    let atk = (0.18 + s.chars[ci].refine as f64 * 0.06) * s.stat_ex(ci, st::EM, true);
    s.add_mod(ci, Mod::stat(KEY, st::ATK, atk, true, true, -1), 12 * 60);
    let team_key = s.weapon_st::<State>(ci).team_key;
    for i in 0..s.chars.len() {
        if i != ci {
            s.add_mod(i, Mod::stat(team_key, st::ATK, atk * 0.3, true, true, -1), 12 * 60);
        }
    }
    s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_UPDATE, arg: 0 }, 10 * 60);
}
