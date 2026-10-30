//! Tenacity of the Millelith, from gcsim's internal/artifacts/tenacity.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("tom-2pc");
const FOUR: ModKey = ModKey::new("tom-4pc");
const FOUR_ICD: ModKey = ModKey::new("tom-4pc-icd");

impl SetImpl for Set {
    const KEY: &'static str = "tenacityofthemillelith";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::HPP, 0.20, false, false, -1));
        if count >= 4 {
            s.subscribe(EventKind::EnemyDamage, Owner::Set(ci as u8, si as u8));
        }
    }

    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) || s.mod_active(ci, FOUR_ICD) {
            return;
        }
        s.add_status(ci, FOUR_ICD, 30, true);
        for i in 0..s.chars.len() {
            s.add_stat_mod(i, FOUR, st::ATKP, 0.2, 180, true);
        }
    }
}
