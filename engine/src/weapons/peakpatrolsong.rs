//! Peak Patrol Song, from gcsim's internal/weapons/sword/peakpatrolsong.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
    /// The party buff's elemental DMG, set at 2 stacks.
    team: f64,
}

const BUFF: ModKey = ModKey::new("peakpatrolsong-buff");
const TEAM: ModKey = ModKey::new("peakpatrolsong-team-buff");
const ICD: ModKey = ModKey::new("peakpatrolsong-buff-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "peakpatrolsong";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// The holder's buff: DEF +8 to 16% and elemental DMG +10 to 20% per
    /// stack; the party's: elemental DMG from the holder's DEF.
    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let st = s.weapon_st::<State>(ci);
        let r = s.chars[ci].refine as f64;
        let v = if m.key == TEAM {
            st.team
        } else {
            let n = st.stacks as f64;
            out[st::DEFP] += (0.06 + 0.02 * r) * n;
            (0.075 + 0.025 * r) * n
        };
        for k in st::PYROP..=st::DENDROP {
            out[k] += v;
        }
        true
    }

    /// The holder's normal or plunging attack damage, once every 0.1 s
    /// (hitlag): a stack for 6 s, up to 2; at 2, the party gets 8 to 16%
    /// elemental DMG per 1,000 of the holder's DEF (up to 25.6 to 51.2%)
    /// for 15 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Plunge) || s.mod_active(ci, ICD) {
            return;
        }
        let active = s.mod_active(ci, BUFF);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        st.stacks = (st.stacks + 1).min(2);
        let stacks = st.stacks;
        let o = Owner::Weapon(ci as u8);
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, o, st::NONE, false, true, -1), 6 * 60);
        if stacks == 2 {
            let r = s.chars[ci].refine as f64;
            let def = total_def(&s.select_stats(ci, [st::BASEDEF, st::DEFP, st::DEF], true));
            s.weapon_st_mut::<State>(ci).team = ((0.06 + 0.02 * r) * def / 1000.0).min(0.192 + 0.064 * r);
            for c in 0..s.chars.len() {
                s.add_mod(c, Mod::owned(TEAM, ModKind::Stat, o, st::NONE, false, true, -1), 15 * 60);
            }
        }
        s.add_status(ci, ICD, 6, true);
    }
}
