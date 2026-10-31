//! Flower of Paradise Lost, from gcsim's internal/artifacts/flowerofparadiselost.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const TWO: ModKey = ModKey::new("flower-2pc");
const FOUR: ModKey = ModKey::new("flower-4pc");
const BUFF: ModKey = ModKey::new("flower-4pc-buff");
const ICD: ModKey = ModKey::new("flower-4pc-icd");

impl SetImpl for Set {
    const KEY: &'static str = "flowerofparadiselost";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::EM, 80.0, false, false, -1));
        if count < 4 {
            return;
        }
        let o = Owner::Set(ci as u8, si as u8);
        s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::ReactBonus, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::Reaction, o);
    }

    /// Bloom, hyperbloom and burgeon DMG +40%, and a quarter of that more
    /// per stack while the stacks' mod lasts. (gcsim's 10% for direct
    /// Lunar-Bloom is not here: the engine has no lunar reactions.)
    fn react_bonus(s: &Sim, ci: usize, si: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if !matches!(info.tag, AttackTag::Bloom | AttackTag::Hyperbloom | AttackTag::Burgeon) {
            return 0.0;
        }
        if m.key == BUFF { 0.4 * s.set_st::<State>(ci, si).stacks as f64 * 0.25 } else { 0.4 }
    }

    /// A bloom on an enemy, or a hyperbloom or burgeon, by the holder, on
    /// or off field: a stack, up to four, the stacks' mod for 10 s (with
    /// hitlag), once a second.
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        match r {
            // gcsim's OnBloom subscription skips targets other than enemies
            Reaction::Bloom if !ev.on_player => {}
            Reaction::Hyperbloom | Reaction::Burgeon => {}
            _ => return,
        }
        if ev.info.actor as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 60, true);
        if !s.mod_active(ci, BUFF) {
            s.set_st_mut::<State>(ci, si).stacks = 0;
        }
        let st = s.set_st_mut::<State>(ci, si);
        if st.stacks < 4 {
            st.stacks += 1;
        }
        s.add_mod(ci, Mod::owned(BUFF, ModKind::ReactBonus, Owner::Set(ci as u8, si as u8), st::NONE, false, true, -1), 10 * 60);
    }
}
