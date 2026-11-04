//! Primordial Jade Winged-Spear, from gcsim's
//! internal/weapons/spear/primordial.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    stacks: i32,
    /// gcsim's w.buff: ATK% and DMG%, set as the stacks grow.
    atk: f64,
    dmg: f64,
}

const BUFF: ModKey = ModKey::new("primordial");
const ICD: ModKey = ModKey::new("primordial-jade-spear-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "primordialjadewingedspear";
    type State = State;
    fn new_state() -> State {
        State { stacks: 0, atk: 0.0, dmg: 0.0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// A hit by the holder on field, once every 0.3 s (hitlag): a stack of
    /// ATK +3.2 to 6%, up to seven, and DMG +12 to 24% at seven, for 6 s
    /// (hitlag); the stacks start over once the buff has ended.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        let active = s.mod_active(ci, BUFF);
        let r = s.chars[ci].refine as f64;
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
            st.dmg = 0.0;
        }
        s.add_status(ci, ICD, 18, true);
        let st = s.weapon_st_mut::<State>(ci);
        if st.stacks < 7 {
            st.stacks += 1;
            if st.stacks == 7 {
                st.dmg = 0.09 + r * 0.03;
            }
            st.atk = st.stacks as f64 * (r * 0.007 + 0.025);
        }
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 360);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let st = s.weapon_st::<State>(ci);
        out[st::ATKP] += st.atk;
        out[st::DMGP] += st.dmg;
        true
    }
}
