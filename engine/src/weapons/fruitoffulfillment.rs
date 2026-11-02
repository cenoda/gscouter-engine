//! Fruit of Fulfillment, from gcsim's internal/weapons/catalyst/fruitoffulfillment.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
    /// gcsim's lastStackGain, which a loss check must still match.
    last_gain: i32,
}

const KEY: ModKey = ModKey::new("fruitoffulfillment");
const ICD: ModKey = ModKey::new("fruitoffulfillment-icd");

const TASK_LOSS: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "fruitoffulfillment";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Stat, o, st::NONE, false, false, -1));
        // gcsim subscribes every reaction event, which the engine's
        // Reaction event covers
        s.subscribe(EventKind::Reaction, o);
    }

    /// EM +24 to 36 and ATK -5% per stack.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let n = s.weapon_st::<State>(ci).stacks as f64;
        out[st::EM] += (21.0 + s.chars[ci].refine as f64 * 3.0) * n;
        out[st::ATKP] += -0.05 * n;
        true
    }

    /// 6 s without a gain loses a stack, and again every 6 s.
    fn task(s: &mut Sim, ci: usize, _: u8, src: i32) {
        let st = s.weapon_st_mut::<State>(ci);
        if st.last_gain != src {
            return;
        }
        st.stacks -= 1;
        if st.stacks > 0 {
            s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_LOSS, arg: src }, 360);
        }
    }

    /// A reaction by the holder (on or off field), once every 0.3 s
    /// (hitlag): a stack, up to 5.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(_, ev) = *e else { return };
        if ev.info.actor as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 18, true);
        let f = s.f;
        let st = s.weapon_st_mut::<State>(ci);
        st.stacks = (st.stacks + 1).min(5);
        st.last_gain = f;
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_LOSS, arg: f }, 360);
    }
}
