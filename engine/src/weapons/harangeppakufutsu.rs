//! Haran Geppaku Futsu, from gcsim's internal/weapons/sword/haran.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    wavespike_stacks: i32,
    consumed_stacks: i32,
}

const ELE_BUFF: ModKey = ModKey::new("haran-ele-bonus");
const ICD: ModKey = ModKey::new("haran-icd");
const NA_BUFF: ModKey = ModKey::new("haran-na");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "harangeppakufutsu";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        let r = s.chars[ci].refine as f64;
        let base = 0.09 + 0.03 * r;
        for i in st::PYROP..=st::DENDROP {
            s.chars[ci].mods.push(Mod::stat(ELE_BUFF, i, base, false, false, -1));
        }
        s.subscribe(EventKind::Skill, o);
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == NA_BUFF && ev.info.tag == AttackTag::Normal {
            let r = s.chars[ci].refine as f64;
            let n = s.weapon_st::<State>(ci).consumed_stacks as f64;
            ev.stats[st::DMGP] += (0.15 + 0.05 * r) * n;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Skill = *e else { return };
        if s.player.active as usize != ci {
            if s.mod_active(ci, ICD) {
                return;
            }
            s.add_status(ci, ICD, 18, true);
            let st = s.weapon_st_mut::<State>(ci);
            if st.wavespike_stacks < 2 {
                st.wavespike_stacks += 1;
            }
        } else {
            let count = s.weapon_st::<State>(ci).wavespike_stacks;
            if count == 0 {
                return;
            }
            let st = s.weapon_st_mut::<State>(ci);
            st.consumed_stacks = count;
            st.wavespike_stacks = 0;
            s.add_mod(ci, Mod::owned(NA_BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 480);
        }
    }
}
