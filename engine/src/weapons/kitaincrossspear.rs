//! Kitain Cross Spear, from gcsim's internal/weapons/spear/kitain.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("kitain-skill-dmg-buff");
const ICD: ModKey = ModKey::new("kitain-icd");

const TASK_REGEN: u8 = 0;

fn is_skill(t: AttackTag) -> bool {
    matches!(t, AttackTag::ElementalArt | AttackTag::ElementalArtHold)
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "kitaincrossspear";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(BUFF, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
    }

    /// Skill DMG +6 to 12%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if is_skill(ev.info.tag) {
            ev.stats[st::DMGP] += 0.045 + s.chars[ci].refine as f64 * 0.015;
        }
    }

    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        let regen = 2.5 + s.chars[ci].refine as f64 * 0.5;
        s.add_energy(ci, regen);
    }

    /// The holder's skill damage, once every 10 s (hitlag): 3 energy lost,
    /// and 3 to 5 regained three times, every 2 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || !is_skill(ev.info.tag) || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 600, true);
        s.add_energy(ci, -3.0);
        for i in [120, 240, 360] {
            s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_REGEN, arg: 0 }, i);
        }
    }
}
