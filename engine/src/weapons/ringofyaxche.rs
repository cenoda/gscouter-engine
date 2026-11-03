//! Ring of Yaxche, from gcsim's internal/weapons/catalyst/ringofyaxche.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    amt: f64,
}

const KEY: ModKey = ModKey::new("ring-of-yaxche");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "ringofyaxche";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    /// Normal attack DMG by the amount taken at the skill.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::Normal {
            ev.stats[st::DMGP] += s.weapon_st::<State>(ci).amt;
        }
    }

    /// A skill by the holder on field: for 10 s (no hitlag), normal attack
    /// DMG +0.6 to 1% per 1,000 max HP, up to 16 to 32%.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Skill = *e else { return };
        if s.player.active as usize != ci {
            return;
        }
        let r = s.chars[ci].refine as f64;
        let amt = (s.max_hp(ci) * 0.001 * (0.005 + 0.001 * r)).min(0.12 + 0.04 * r);
        s.weapon_st_mut::<State>(ci).amt = amt;
        s.add_mod(ci, Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1), 10 * 60);
    }
}
