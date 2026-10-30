//! Marechaussee Hunter, from gcsim's internal/artifacts/marechausseehunter.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const TWO: ModKey = ModKey::new("mh-2pc");
const FOUR: ModKey = ModKey::new("mh-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "marechausseehunter";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        let o = Owner::Set(ci as u8, si as u8);
        s.chars[ci].mods.push(Mod::owned(TWO, ModKind::Attack, o, st::NONE, false, false, -1));
        if count >= 4 {
            s.subscribe(EventKind::Drain, o);
            s.subscribe(EventKind::Heal, o);
        }
    }

    /// Normal and charged attack DMG +15%.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            ev.stats[st::DMGP] += 0.15;
        }
    }

    fn mod_amount(s: &Sim, ci: usize, si: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::CR] += 0.12 * s.set_st::<State>(ci, si).stacks as f64;
        true
    }

    /// The holder's HP changing while on field: CRIT Rate +12% for 5 s, up
    /// to 3 stacks. A heal at full HP does not count.
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        if s.player.active as usize != ci {
            return;
        }
        match *e {
            Event::Drain { on, amount, .. } if on == ci && amount > 0.0 => {}
            Event::Heal { on, amount, overheal, .. } if on == ci && amount > 0.0 && (amount - overheal).abs() > 1e-9 => {}
            _ => return,
        }
        let active = s.mod_active(ci, FOUR);
        let st = s.set_st_mut::<State>(ci, si);
        if !active {
            st.stacks = 0;
        }
        st.stacks = (st.stacks + 1).min(3);
        s.add_mod(ci, Mod::owned(FOUR, ModKind::Stat, Owner::Set(ci as u8, si as u8), st::CR, false, true, -1), 5 * 60);
    }
}
