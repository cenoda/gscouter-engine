//! Brave Heart, from gcsim's internal/artifacts/braveheart.

use crate::content::SetImpl;
use crate::sim::*;
use crate::types::*;

pub struct Set;

#[derive(Default)]
pub struct State {
    enemy_max_hp: f64,
}

const TWO: ModKey = ModKey::new("braveheart-2pc");
const FOUR: ModKey = ModKey::new("braveheart-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "braveheart";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        }
        if count >= 4 && s.damage_mode {
            s.set_st_mut::<State>(ci, si).enemy_max_hp = s.enemy_hp;
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, false, -1));
        }
    }

    fn attack_mod(s: &Sim, ci: usize, si: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == FOUR {
            let max_hp = s.set_st::<State>(ci, si).enemy_max_hp;
            if max_hp > 0.0 && s.enemy_hp / max_hp > 0.5 {
                ev.stats[st::DMGP] += 0.30;
            }
        }
    }
}
