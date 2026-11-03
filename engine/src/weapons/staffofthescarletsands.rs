//! Staff of the Scarlet Sands, from gcsim's internal/weapons/spear/scarletsands.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
    icd_counter: i32,
}

const BASE: ModKey = ModKey::new("scarletsands");
const SKILL: ModKey = ModKey::new("scarletsands-skill");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "staffofthescarletsands";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(BASE, ModKind::Stat, o, st::ATK, true, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
    }

    /// ATK +52 to 104% of EM (not counting extra EM); each stack of the
    /// skill buff 28 to 56% more.
    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let em = s.stat_ex(ci, st::EM, true);
        out[st::ATK] += if m.key == BASE {
            (0.39 + 0.13 * r) * em
        } else {
            (0.21 + 0.07 * r) * em * s.weapon_st::<State>(ci).stacks as f64
        };
        true
    }

    /// A skill hit by the holder on field, at most three within 0.3 s: a
    /// stack (up to three, from none once the buff is gone), the buff for
    /// 10 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            return;
        }
        if s.weapon_st::<State>(ci).icd_counter >= 3 {
            return;
        }
        s.weapon_st_mut::<State>(ci).icd_counter += 1;
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: 0, arg: 0 }, 18);
        let active = s.mod_active(ci, SKILL);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        if st.stacks < 3 {
            st.stacks += 1;
        }
        s.add_mod(ci, Mod::owned(SKILL, ModKind::Stat, Owner::Weapon(ci as u8), st::ATK, true, true, -1), 10 * 60);
    }

    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        s.weapon_st_mut::<State>(ci).icd_counter -= 1;
    }
}
