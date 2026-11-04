//! Clash of Kings, from gcsim's internal/weapons/catalyst/clashofkings.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("clash-of-kings");
const BUFF_ICD: ModKey = ModKey::new("clash-of-kings-icd");
const CHARGE_EXT: ModKey = ModKey::new("clash-of-kings-ca-extension");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "clashofkings";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::EnemyHit, o);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        out[st::ATKP] += 0.15 + 0.05 * r;
        out[st::EM] += 75.0 + 25.0 * r;
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Skill => {
                if s.player.active as usize != ci || s.mod_active(ci, BUFF_ICD) {
                    return;
                }
                s.add_status(ci, BUFF_ICD, 12 * 60, true);
                s.add_status(ci, CHARGE_EXT, 12 * 60, true);
                s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 6 * 60);
            }
            Event::EnemyHit(ev) => {
                if ev.info.actor as usize != ci || s.player.active as usize != ci || ev.info.tag != AttackTag::Extra {
                    return;
                }
                if !s.mod_active(ci, BUFF) || !s.mod_active(ci, CHARGE_EXT) {
                    return;
                }
                s.delete_mod(ci, CHARGE_EXT);
                s.extend_mod(ci, BUFF, 6 * 60);
            }
            _ => {}
        }
    }
}
