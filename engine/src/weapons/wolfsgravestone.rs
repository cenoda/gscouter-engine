//! Wolf's Gravestone, from gcsim's internal/weapons/claymore/wolf.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    /// The enemy's HP at the start (its max HP), in damage mode.
    enemy_max_hp: f64,
}

const FLAT: ModKey = ModKey::new("wolf-flat");
const PROC: ModKey = ModKey::new("wolf-proc");
const ICD: ModKey = ModKey::new("wolf-gravestone-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "wolfsgravestone";
    type State = State;
    fn new_state() -> State {
        State { enemy_max_hp: 0.0 }
    }

    /// ATK +20 to 40%.
    fn init(s: &mut Sim, ci: usize) {
        let v = 0.15 + 0.05 * s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(FLAT, st::ATKP, v, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
        let hp = s.enemy_hp;
        s.weapon_st_mut::<State>(ci).enemy_max_hp = hp;
    }

    /// In damage mode only: a hit by the holder on field on an enemy at
    /// 30% HP or less, once every 30 s (hitlag), gives every member ATK
    /// +40 to 80% for 12 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if !s.damage_mode {
            return;
        }
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        if s.enemy_hp / s.weapon_st::<State>(ci).enemy_max_hp > 0.3 {
            return;
        }
        s.add_status(ci, ICD, 1800, true);
        let v = 0.3 + 0.1 * s.chars[ci].refine as f64;
        for x in 0..s.chars.len() {
            s.add_mod(x, Mod::stat(PROC, st::ATKP, v, false, true, -1), 720);
        }
    }
}
