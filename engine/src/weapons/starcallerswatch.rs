//! Starcaller's Watch, from gcsim's internal/weapons/catalyst/starcallerswatch.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    tick_src: i32,
}

const EM: ModKey = ModKey::new("starcallerswatch-em");
const BUFF: ModKey = ModKey::new("starcallerswatch-buff");
const ICD: ModKey = ModKey::new("starcallerswatch-icd");
const STATUS: ModKey = ModKey::new("starcallerswatch");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "starcallerswatch";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// EM +100 to 200.
    fn init(s: &mut Sim, ci: usize) {
        let em = 75.0 + 25.0 * s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(EM, st::EM, em, false, false, -1));
        s.subscribe(EventKind::Shielded, Owner::Weapon(ci as u8));
    }

    /// DMG +28 to 56% for the character carrying the mod while it is the
    /// active one.
    fn attack_mod(s: &Sim, ci: usize, on: usize, _: &Mod, ev: &mut AttackEvent) {
        if s.player.active as usize == on {
            ev.stats[st::DMGP] += 0.21 + 0.07 * s.chars[ci].refine as f64;
        }
    }

    /// A shield of the holder's made with the holder on field, once every
    /// 14 s (hitlag): every party member gets the buff, which a task 15 s
    /// later (hitlag) removes unless a later proc replaced it.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Shielded(sh) = *e else { return };
        if sh.owner as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 14 * 60, true);
        s.add_status(ci, STATUS, 15 * 60, true);
        let src = s.f;
        s.weapon_st_mut::<State>(ci).tick_src = src;
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: 0, arg: src }, 15 * 60);
        for j in 0..s.chars.len() {
            s.add_mod(j, Mod::owned(BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1), -1);
        }
    }

    fn task(s: &mut Sim, ci: usize, _: u8, src: i32) {
        if src != s.weapon_st::<State>(ci).tick_src {
            return;
        }
        for j in 0..s.chars.len() {
            s.delete_mod(j, BUFF);
        }
    }
}
