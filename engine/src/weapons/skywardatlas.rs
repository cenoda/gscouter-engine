//! Skyward Atlas, from gcsim's internal/weapons/catalyst/skyward.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
/// The snapshot of the last proc, which its six cloud hits use (the 30 s
/// ICD outlasts the 6 x 147 frames of a proc's hits).
pub struct State {
    snap: Option<AttackEvent>,
}

const TASK_HIT: u8 = 0;

const ELE_BUFF: ModKey = ModKey::new("skyward-atlas");
const ICD: ModKey = ModKey::new("skyward-atlas-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "skywardatlas";
    type State = State;
    fn new_state() -> State {
        State { snap: None }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        let r = s.chars[ci].refine as f64;
        let base = 0.09 + 0.03 * r;
        for i in st::PYROP..=st::DENDROP {
            s.chars[ci].mods.push(Mod::stat(ELE_BUFF, i, base, false, false, -1));
        }
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || ev.info.tag != AttackTag::Normal || s.mod_active(ci, ICD) {
            return;
        }
        if s.rng.float64() < 0.5 {
            return;
        }
        let r = s.chars[ci].refine as f64;
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 1.2 + 0.4 * r);
        ai.abil = "Skyward Atlas Proc";
        let snap = s.snapshot_event(ai, 0);
        s.weapon_st_mut::<State>(ci).snap = Some(snap);
        for i in 1..=6 {
            s.add_task(Task::Weapon { c: ci as u8, id: TASK_HIT, arg: 0 }, i * 147);
        }
        s.add_status(ci, ICD, 1800, true);
    }

    /// gcsim's task: the closest enemy within 15 of the player takes the
    /// hit, `travel` frames later.
    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        if id != TASK_HIT || !s.enemy_alive || !s.enemy_within(s.circle_on_player(0.0, 0.0, 15.0)) {
            return;
        }
        let Some(mut snap) = s.weapon_st::<State>(ci).snap else { return };
        snap.info.pattern = s.circle_on_target(1.2);
        let travel = s.chars[ci].weapon_params.iter().find(|p| p.0 == "travel").map_or(10, |p| p.1 as i32);
        s.queue_attack_with_snap(snap, 0, travel);
    }
}
