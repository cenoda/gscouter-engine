//! Hunter's Path, from gcsim's internal/weapons/bow/hunterspath.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    /// Charged attack hits left to Tireless Hunt.
    proc_count: i32,
}

const BONUS: ModKey = ModKey::new("hunterspath-dmg-bonus");
const BUFF: ModKey = ModKey::new("hunterspath-tireless-hunt");
const ICD: ModKey = ModKey::new("hunterspath-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "hunterspath";
    type State = State;
    fn new_state() -> State {
        State { proc_count: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(BONUS, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyHit, Owner::Weapon(ci as u8));
    }

    /// Elemental DMG bonus +12 to 24% (gcsim's PyroP to DendroP).
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let dmg = 0.09 + 0.03 * s.chars[ci].refine as f64;
        for stat in [st::PYROP, st::HYDROP, st::CRYOP, st::ELECTROP, st::ANEMOP, st::GEOP, st::DENDROP] {
            out[stat] += dmg;
        }
        true
    }

    /// A charged attack by the holder hitting outside the 12 s cooldown
    /// (hitlag) starts Tireless Hunt for 10 s (hitlag), without a bonus of
    /// its own; while it lasts, each charged attack hit adds 160 to 320% of
    /// EM as flat damage, for 12 hits.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if ev.info.actor as usize != ci || ev.info.tag != AttackTag::Extra {
            return;
        }
        if !s.mod_active(ci, ICD) {
            s.add_status(ci, BUFF, 600, true);
            s.add_status(ci, ICD, 720, true);
            s.weapon_st_mut::<State>(ci).proc_count = 12;
            return;
        }
        if !s.mod_active(ci, BUFF) {
            return;
        }
        let boost = 1.2 + 0.4 * s.chars[ci].refine as f64;
        s.hit_flat += s.stat(ci, st::EM) * boost;
        let st = s.weapon_st_mut::<State>(ci);
        st.proc_count -= 1;
        if st.proc_count <= 0 {
            s.delete_mod(ci, BUFF);
        }
    }
}
