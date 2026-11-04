//! theunforged, from gcsim's internal/weapons/common/goldenmajesty.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State(pub super::goldenmajesty::State);

impl WeaponImpl for Weapon {
    const KEY: &'static str = "theunforged";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        super::goldenmajesty::init(s, ci);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let ev = match *e {
            Event::EnemyDamage(ev) => ev,
            _ => return,
        };
        let mut inner = std::mem::take(&mut s.weapon_st_mut::<State>(ci).0);
        super::goldenmajesty::on_enemy_damage(s, ci, &ev, &mut inner);
        s.weapon_st_mut::<State>(ci).0 = inner;
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let stacks = s.weapon_st::<State>(ci).0.stacks;
        super::goldenmajesty::mod_amount(s, ci, stacks, out)
    }

    fn shield_bonus(s: &Sim, ci: usize, _: ModKey) -> (f64, bool) {
        super::goldenmajesty::shield_bonus(s, ci)
    }
}
