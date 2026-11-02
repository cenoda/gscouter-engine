//! Pale Flame, from gcsim's internal/artifacts/paleflame.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const TWO: ModKey = ModKey::new("pf-2pc");
const FOUR: ModKey = ModKey::new("pf-4pc");
const ICD: ModKey = ModKey::new("pf-4pc-icd");

impl SetImpl for Set {
    const KEY: &'static str = "paleflame";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::PHYP, 0.25, false, false, -1));
        }
        if count >= 4 {
            s.subscribe(EventKind::EnemyDamage, Owner::Set(ci as u8, si as u8));
        }
    }

    /// ATK +9% per stack, and physical DMG +25% at 2.
    fn mod_amount(s: &Sim, ci: usize, si: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let n = s.set_st::<State>(ci, si).stacks;
        if n == 2 {
            out[st::PHYP] += 0.25;
        }
        out[st::ATKP] += 0.09 * n as f64;
        true
    }

    /// The wearer's skill damage on field, once every 0.3 s (hitlag): a
    /// stack for 7 s, up to 2.
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) || s.mod_active(ci, ICD) {
            return;
        }
        let active = s.mod_active(ci, FOUR);
        let st = s.set_st_mut::<State>(ci, si);
        if !active {
            st.stacks = 0;
        }
        st.stacks = (st.stacks + 1).min(2);
        s.add_status(ci, ICD, 18, true);
        s.add_mod(ci, Mod::owned(FOUR, ModKind::Stat, Owner::Set(ci as u8, si as u8), st::NONE, false, true, -1), 420);
    }
}
