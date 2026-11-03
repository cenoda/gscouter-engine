//! Jade Vista, from gcsim's internal/weapons/bow/jadevista: on Init, EM
//! +64 to 128 for each other member of the holder's element and ATK +12
//! to 24% for each other of another, up to three each.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    em: f64,
    atk: f64,
}

const KEY: ModKey = ModKey::new("jade-vista");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "jadevista";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(_s: &mut Sim, _ci: usize) {}

    fn post_init(s: &mut Sim, ci: usize) {
        let ele = s.chars[ci].element;
        let (mut same, mut different) = (0, 0);
        for (j, c) in s.chars.iter().enumerate() {
            if j == ci {
                continue;
            }
            if c.element == ele {
                same += 1;
            } else {
                different += 1;
            }
        }
        let r = s.chars[ci].refine as f64;
        let st = s.weapon_st_mut::<State>(ci);
        st.em = (48.0 + r * 16.0) * same.min(3) as f64;
        st.atk = (0.09 + r * 0.03) * different.min(3) as f64;
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let st = s.weapon_st::<State>(ci);
        out[st::EM] += st.em;
        out[st::ATKP] += st.atk;
        true
    }
}
