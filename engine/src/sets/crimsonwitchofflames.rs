//! Crimson Witch of Flames, from gcsim's internal/artifacts/crimson.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const TWO: ModKey = ModKey::new("crimson-2pc");
const FOUR: ModKey = ModKey::new("crimson-4pc");
const STACKS: ModKey = ModKey::new("crimson-4pc-stacks");

impl SetImpl for Set {
    const KEY: &'static str = "crimsonwitchofflames";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::PYROP, 0.15, false, false, -1));
        if count < 4 {
            return;
        }
        let o = Owner::Set(ci as u8, si as u8);
        s.subscribe(EventKind::Skill, o);
        s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::ReactBonus, o, st::NONE, false, false, -1));
    }

    /// Overloaded, burning and burgeon DMG +40%, amplifying reactions +15%.
    fn react_bonus(_: &Sim, _: usize, _: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        match info.tag {
            AttackTag::OverloadDamage | AttackTag::BurningDamage | AttackTag::Burgeon => 0.4,
            _ if info.amped => 0.15,
            _ => 0.0,
        }
    }

    /// A skill by the holder on field adds a stack of half the 2-piece
    /// bonus for 10 s, up to three.
    fn mod_amount(s: &Sim, ci: usize, si: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::PYROP] += 0.15 * 0.5 * s.set_st::<State>(ci, si).stacks as f64;
        true
    }

    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        if !matches!(e, Event::Skill) || s.player.active as usize != ci {
            return;
        }
        if !s.mod_active(ci, STACKS) {
            s.set_st_mut::<State>(ci, si).stacks = 0;
        }
        let st = s.set_st_mut::<State>(ci, si);
        if st.stacks < 3 {
            st.stacks += 1;
        }
        s.add_mod(ci, Mod::owned(STACKS, ModKind::Stat, Owner::Set(ci as u8, si as u8), st::PYROP, false, true, -1), 10 * 60);
    }
}
