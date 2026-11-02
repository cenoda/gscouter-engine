//! Vourukasha's Glow, from gcsim's internal/artifacts/vourukashasglow.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
#[derive(Default)]
pub struct State {
    /// The next of the five stacks, each its own 5 s mod.
    counter: usize,
}

const TWO: ModKey = ModKey::new("vg-2pc");
const FOUR: ModKey = ModKey::new("vg-4pc");
const STACKS: [ModKey; 5] = [ModKey::new("vg-4pc-1-stack"), ModKey::new("vg-4pc-2-stack"), ModKey::new("vg-4pc-3-stack"), ModKey::new("vg-4pc-4-stack"), ModKey::new("vg-4pc-5-stack")];

impl SetImpl for Set {
    const KEY: &'static str = "vourukashasglow";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::HPP, 0.20, false, false, -1));
        }
        if count >= 4 {
            let o = Owner::Set(ci as u8, si as u8);
            s.subscribe(EventKind::Drain, o);
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Attack, o, st::NONE, false, false, -1));
        }
    }

    /// Skill and burst DMG +10%, and +8% per stack.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst) {
            ev.stats[st::DMGP] += if m.key == FOUR { 0.1 } else { 0.08 };
        }
    }

    /// The wearer losing HP to something else: a stack for 5 s (hitlag).
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::Drain { on, amount, external } = *e else { return };
        if on != ci || amount <= 0.0 || !external {
            return;
        }
        let st = s.set_st_mut::<State>(ci, si);
        let k = STACKS[st.counter];
        st.counter = (st.counter + 1) % 5;
        s.add_mod(ci, Mod::owned(k, ModKind::Attack, Owner::Set(ci as u8, si as u8), st::NONE, false, true, -1), 300);
    }
}
