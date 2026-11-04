//! Sword of Narzissenkreuz, from gcsim's internal/weapons/sword/swordofnarzissenkreuz.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ICD: ModKey = ModKey::new("swordofnarzissenkreuz-icd");
const TASK_PROC: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "swordofnarzissenkreuz";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        // gcsim's HasArkhe, which these characters' NewChar set (it reads
        // the param arkhe only to log it)
        if matches!(
            s.chars[ci].kind,
            CharKind::Navia
                | CharKind::Charlotte
                | CharKind::Emilie
                | CharKind::Freminet
                | CharKind::Clorinde
                | CharKind::Lynette
                | CharKind::Lyney
                | CharKind::Chevreuse
                | CharKind::Wriothesley
                | CharKind::Furina
                | CharKind::Sigewinne
                | CharKind::Neuvillette
                | CharKind::LumineHydro
                | CharKind::AetherHydro
        ) {
            return;
        }
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            return;
        }
        s.add_status(ci, ICD, 12 * 60, true);
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_PROC, arg: 0 }, 6);
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        if id == TASK_PROC {
            let r = s.chars[ci].refine as f64;
            let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 1.2 + 0.4 * r);
            ai.abil = "Sword of Narzissenkreuz";
            ai.pattern = s.circle_on_target(3.0);
            s.queue_attack(ai, 0, 0, 0);
        }
    }
}
