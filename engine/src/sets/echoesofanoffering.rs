//! Echoes of an Offering, from gcsim's internal/artifacts/echoes.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State {
    prob: f64,
    icd: i32,
    proc_expiry: i32,
}

const TWO: ModKey = ModKey::new("echoes-2pc");

impl SetImpl for Set {
    const KEY: &'static str = "echoesofanoffering";
    type State = State;
    fn new_state() -> State {
        State { prob: 0.36, icd: 0, proc_expiry: 0 }
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ATKP, 0.18, false, false, -1));
        }
        if count >= 4 {
            s.subscribe(EventKind::EnemyHit, Owner::Set(ci as u8, si as u8));
        }
    }

    /// The wearer's normal attack hits on field: Valley Rite, 70% of its
    /// ATK added, at 36% (+20% after each miss, once every 0.2 s) and for
    /// 0.05 s after it triggers.
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if s.player.active as usize != ci || ev.info.actor as usize != ci || ev.info.tag != AttackTag::Normal {
            return;
        }
        let atk = total_atk(&s.select_stats(ci, [st::BASEATK, st::ATKP, st::ATK], false));
        let f = s.f;
        let st = s.set_st::<State>(ci, si);
        if f < st.proc_expiry {
            s.hit_flat += atk * 0.7;
            return;
        }
        if f < st.icd {
            return;
        }
        if s.rng.float64() > s.set_st::<State>(ci, si).prob {
            let st = s.set_st_mut::<State>(ci, si);
            st.icd = f + 12;
            st.prob += 0.2;
            return;
        }
        s.hit_flat += atk * 0.7;
        let st = s.set_st_mut::<State>(ci, si);
        st.proc_expiry = f + 3;
        st.icd = f + 12;
        st.prob = 0.36;
    }
}
