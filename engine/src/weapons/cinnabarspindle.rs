//! Cinnabar Spindle, from gcsim's internal/weapons/sword/cinnabar.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("cinnabar-icd");
const ACTIVE: ModKey = ModKey::new("cinnabar-buff-active");

const TASK_ICD: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "cinnabarspindle";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyHit, Owner::Weapon(ci as u8));
    }

    /// The 1.5 s cooldown, 6 frames after the first hit (no hitlag).
    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        s.add_status(ci, ICD, 90, false);
    }

    /// The holder's skill hits deal 40 to 80% of its DEF more; the hits of
    /// the 6 frames after the first all do, then 1.5 s of cooldown.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) || s.mod_active(ci, ICD) {
            return;
        }
        if !s.mod_active(ci, ACTIVE) {
            s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_ICD, arg: 0 }, 6);
            s.add_status(ci, ACTIVE, 6, false);
        }
        let def = total_def(&s.select_stats(ci, [st::BASEDEF, st::DEFP, st::DEF], false));
        s.hit_flat += def * (0.3 + s.chars[ci].refine as f64 * 0.1);
    }
}
