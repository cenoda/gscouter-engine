//! Deepwood Memories, from gcsim's internal/artifacts/deepwood.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("dm-2pc");
const FOUR: ModKey = ModKey::new("dm-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "deepwoodmemories";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::DENDROP, 0.15, false, false, -1));
        if count >= 4 {
            s.subscribe(EventKind::EnemyDamage, Owner::Set(ci as u8, si as u8));
        }
    }

    /// The holder's skill or burst damage: dendro RES -30% for 8 s.
    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst) {
            return;
        }
        s.enemy_add_resist(FOUR, Element::Dendro, -0.3, 8 * 60);
    }
}
