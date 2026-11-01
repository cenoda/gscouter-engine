//! Aqua Simulacra, from gcsim's internal/weapons/bow/aquasimulacra: its
//! DMG bonus while the enemy is within 8 of the player.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const HP: ModKey = ModKey::new("aquasimulacra-hp");
const DMG: ModKey = ModKey::new("aquasimulacra-dmg");

const TASK_CHECK: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "aquasimulacra";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(HP, st::HPP, 0.12 + r * 0.04, false, false, -1));
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_CHECK, arg: 0 }, 30);
    }

    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        // the enemy within 8 of the player (EnemiesWithinArea)
        if s.enemy_alive && s.enemy_within(s.circle_on_player(0.0, 0.0, 8.0)) {
            let v = 0.15 + s.chars[ci].refine as f64 * 0.05;
            let mut m = Mod::new(DMG, ModKind::Attack, st::DMGP, false, true, -1);
            m.value = v;
            s.add_mod(ci, m, 72);
        }
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_CHECK, arg: 0 }, 30);
    }
}
