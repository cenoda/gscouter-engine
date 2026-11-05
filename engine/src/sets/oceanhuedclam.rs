//! Ocean-Hued Clam, from gcsim's internal/artifacts/oceanhuedclam.
//!
//! gcsim's Flags.Custom["OHCActiveChar"] is shared by every wearer: the
//! wearer whose foam is up, or none. The engine keeps it as that wearer's
//! index plus one, 0 for none.

use crate::content::{Event, SetImpl};
use crate::gamedata::icd;
use crate::sim::*;
use crate::types::*;

pub struct Set;
#[derive(Default)]
pub struct State {
    /// gcsim's bubbleHealStacks: the healing gathered, up to 30,000. It
    /// gathers also while the foam is on cooldown and none is up, and that
    /// goes into the next foam.
    stacks: f64,
    /// gcsim's bubbleICDExpiry: a new foam only after this frame.
    icd_expiry: i32,
}

const TWO: ModKey = ModKey::new("ohc-2pc");
const ACTIVE: ModKey = ModKey::new("OHCActiveChar");

const TASK_POP: u8 = 0;

impl SetImpl for Set {
    const KEY: &'static str = "oceanhuedclam";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::HEAL, 0.15, false, false, -1));
        if count >= 4 {
            s.subscribe(EventKind::Heal, Owner::Set(ci as u8, si as u8));
        }
    }

    /// gcsim's Set.Init: no foam is up.
    fn post_init(s: &mut Sim, _: usize, _: usize) {
        s.set_flag(ACTIVE, 0.0);
    }

    /// The foam bursts 3 s after it appeared: physical damage of 90% of
    /// the healing gathered, which ignores the wearer's stats and the
    /// enemy's DEF, in a circle of radius 6 on the player.
    fn task(s: &mut Sim, ci: usize, si: usize, _: u8, _: i32) {
        let stacks = s.set_st::<State>(ci, si).stacks;
        let mut ai = AttackInfo::new(ci, AttackTag::NoneStat, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 0.0, 0.0);
        ai.abil = "Sea-Dyed Foam";
        ai.ignore_def = 1.0;
        ai.flat_dmg = stacks * 0.9;
        ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
        s.queue_attack(ai, 0, -1, 1);
        s.set_flag(ACTIVE, 0.0);
        s.set_st_mut::<State>(ci, si).stacks = 0.0;
    }

    /// A heal by the wearer, also off field, gathers into the foam (the
    /// heal before the Bond of Life takes its part, overflow included),
    /// unless another wearer's foam is up; a foam appears at most once
    /// every 3.5 s.
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::Heal { caller, before_debt, .. } = *e else { return };
        if caller != ci {
            return;
        }
        let active = s.flag(ACTIVE);
        if active != 0.0 && active != (ci + 1) as f64 {
            return;
        }
        let f = s.f;
        let st = s.set_st_mut::<State>(ci, si);
        st.stacks = (st.stacks + before_debt).min(30000.0);
        if st.icd_expiry < f {
            st.icd_expiry = f + 210;
            s.set_flag(ACTIVE, (ci + 1) as f64);
            s.add_task(Task::Set { c: ci as u8, si: si as u8, id: TASK_POP, arg: 0 }, 3 * 60);
        }
    }
}
