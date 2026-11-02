//! Prototype Crescent, from gcsim's internal/weapons/bow/prototype.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("prototype-crescent");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "prototypecrescent";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// The holder hitting a weak spot while on field: ATK +36 to 72% for
    /// 10 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || !ev.info.weak_point {
            return;
        }
        let v = 0.27 + s.chars[ci].refine as f64 * 0.09;
        s.add_stat_mod(ci, BUFF, st::ATKP, v, 10 * 60, true);
    }
}
