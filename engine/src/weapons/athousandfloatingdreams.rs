//! A Thousand Floating Dreams, from gcsim's internal/weapons/catalyst/athousandfloatingdreams.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    em: f64,
    dmg: f64,
}

const KEY: ModKey = ModKey::new("a-thousand-floating-dreams");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "athousandfloatingdreams";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
    }

    /// For each other party member, EM +32 to 64 if of the holder's
    /// element, else the holder's elemental DMG +10 to 26%; every other
    /// member EM +40 to 48 (keyed by the holder).
    fn post_init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        let el = s.chars[ci].element;
        let key = ModKey::named(format!("a-thousand-floating-dreams-party-{}", crate::gcsl::char_name(s.chars[ci].key)));
        let (mut same, mut diff) = (0, 0);
        for j in 0..s.chars.len() {
            if j == ci {
                continue;
            }
            if s.chars[j].element == el {
                same += 1;
            } else {
                diff += 1;
            }
            s.chars[j].mods.push(Mod::stat(key, st::EM, 38.0 + r * 2.0, false, false, -1));
        }
        let st = s.weapon_st_mut::<State>(ci);
        st.em = (24.0 + r * 8.0) * same as f64;
        st.dmg = (0.06 + r * 0.04) * diff as f64;
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let st = s.weapon_st::<State>(ci);
        out[st::EM] += st.em;
        out[s.chars[ci].element.dmg_stat()] += st.dmg;
        true
    }
}
