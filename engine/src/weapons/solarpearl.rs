//! Solar Pearl, from gcsim's internal/weapons/catalyst/solar.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const NA: ModKey = ModKey::new("solar-na-buff");
const SKILL: ModKey = ModKey::new("solar-skill-burst-buff");

fn skill_or_burst(t: AttackTag) -> bool {
    matches!(t, AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst)
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "solarpearl";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// DMG +20 to 40%: normal attacks under the one buff, skills and
    /// bursts under the other.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        let applies = if m.key == NA { ev.info.tag == AttackTag::Normal } else { skill_or_burst(ev.info.tag) };
        if applies {
            ev.stats[st::DMGP] += 0.15 + s.chars[ci].refine as f64 * 0.05;
        }
    }

    /// A skill or burst hit by the holder on field: the normal attack buff
    /// for 6 s (hitlag); a normal hit: the skill and burst buff.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        let key = if skill_or_burst(ev.info.tag) {
            NA
        } else if ev.info.tag == AttackTag::Normal {
            SKILL
        } else {
            return;
        };
        s.add_mod(ci, Mod::owned(key, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 6 * 60);
    }
}
