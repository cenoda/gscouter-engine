//! Winter's Heavy Heart, from gcsim's internal/weapons/catalyst/wintersheavyheart.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    cryo: i32,
    electro: i32,
}

const EM_KEY: ModKey = ModKey::new("winters-heavy-heart-em");
const ATK_KEY: ModKey = ModKey::new("winters-heavy-heart-atk");
const STELLAR_KEY: ModKey = ModKey::new("winters-heavy-heart-stellar");

fn radiance(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, crate::stellar::POLESTAR_FIELD) || s.mod_active(ci, crate::stellar::RADIANCE_SWIRL)
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "wintersheavyheart";
    type State = State;
    fn new_state() -> State {
        State { cryo: 0, electro: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let mut cryo = 0;
        let mut electro = 0;
        for c in &s.chars {
            match c.element {
                Element::Cryo => cryo += 1,
                Element::Electro => electro += 1,
                _ => {}
            }
        }
        cryo = cryo.min(4);
        electro = electro.min(4 - cryo);
        if cryo + electro == 0 {
            return;
        }
        let st = s.weapon_st_mut::<State>(ci);
        st.cryo = cryo;
        st.electro = electro;

        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(EM_KEY, ModKind::Stat, o, st::NONE, false, false, -1));
        if electro > 0 {
            s.chars[ci].mods.push(Mod::owned(ATK_KEY, ModKind::Stat, o, st::NONE, false, false, -1));
        }
        s.chars[ci].mods.push(Mod::owned(STELLAR_KEY, ModKind::ReactBonus, o, st::NONE, false, false, -1));
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let st = s.weapon_st::<State>(ci);
        let stacks = (st.cryo + st.electro) as f64;
        let rad = radiance(s, ci);
        if m.key == EM_KEY {
            let em = if rad {
                (15.0 + 5.0 * r) * stacks
            } else {
                (18.0 + 6.0 * r) * st.cryo as f64
            };
            if em > 0.0 {
                out[st::EM] += em;
                return true;
            }
        } else if m.key == ATK_KEY {
            if !rad {
                let atk = (0.036 + 0.012 * r) * st.electro as f64;
                out[st::ATKP] += atk;
                return true;
            }
        }
        false
    }

    fn react_bonus(s: &Sim, ci: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if m.key == STELLAR_KEY && info.tag.is_stellar() && radiance(s, ci) {
            let r = s.chars[ci].refine as f64;
            let stacks = (s.weapon_st::<State>(ci).cryo + s.weapon_st::<State>(ci).electro) as f64;
            (0.045 + 0.015 * r) * stacks
        } else {
            0.0
        }
    }
}
