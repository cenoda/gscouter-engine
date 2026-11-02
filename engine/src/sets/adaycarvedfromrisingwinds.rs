//! A Day Carved From Rising Winds, from gcsim's
//! internal/artifacts/adaycarvedfromrisingwinds.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("a-day-carved-from-rising-winds-2pc");
const BLESSING: ModKey = ModKey::new("blessing-of-pastoral-winds");
const RESOLVE: ModKey = ModKey::new("resolve-of-pastoral-winds");

impl SetImpl for Set {
    const KEY: &'static str = "adaycarvedfromrisingwinds";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        }
        if count >= 4 {
            s.subscribe(EventKind::EnemyDamage, Owner::Set(ci as u8, si as u8));
        }
    }

    /// The wearer's normal, charged, skill or burst damage: ATK +25% for
    /// 6 s (hitlag), and on a Hexerei character CRIT Rate +20%.
    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst) {
            return;
        }
        s.add_stat_mod(ci, BLESSING, st::ATKP, 0.25, 6 * 60, true);
        if s.chars[ci].hexerei {
            s.add_stat_mod(ci, RESOLVE, st::CR, 0.2, 6 * 60, true);
        }
    }
}
