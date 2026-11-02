//! Obsidian Codex, from gcsim's internal/artifacts/obsidiancodex: two
//! pieces, 15% DMG while the holder is on field in Nightsoul's Blessing;
//! four pieces, once the holder on field has used 1 Nightsoul point
//! (counted over consumptions, once every 1 s), 40% CRIT Rate for 6 s.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
#[derive(Default)]
pub struct State {
    consume_count: f64,
}

const TWO: ModKey = ModKey::new("obsidiancodex-2pc");
const FOUR: ModKey = ModKey::new("obsidiancodex-4pc");
const FOUR_ICD: ModKey = ModKey::new("obsidiancodex-4pc-icd");

impl SetImpl for Set {
    const KEY: &'static str = "obsidiancodex";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        let o = Owner::Set(ci as u8, si as u8);
        s.chars[ci].mods.push(Mod::owned(TWO, ModKind::Stat, o, st::DMGP, false, false, -1));
        if count >= 4 {
            s.subscribe(EventKind::NightsoulConsume, o);
        }
    }

    /// Two pieces: on field in Nightsoul's Blessing.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if !s.ns_has_blessing(ci) || s.player.active as usize != ci {
            return false;
        }
        out[st::DMGP] += 0.15;
        true
    }

    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::NightsoulConsume { on, amount } = *e else { return };
        if on != ci || s.player.active as usize != ci || s.mod_active(ci, FOUR_ICD) {
            return;
        }
        let st = s.set_st_mut::<State>(ci, si);
        st.consume_count += amount;
        if st.consume_count >= 1.0 {
            st.consume_count = 0.0;
            s.add_status(ci, FOUR_ICD, 60, true);
            s.add_stat_mod(ci, FOUR, st::CR, 0.4, 6 * 60, true);
        }
    }
}
