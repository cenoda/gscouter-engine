//! Silken Moon's Serenade, from gcsim's internal/artifacts/silkenmoonsserenade.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State {
    /// Gleaming Moon: Devotion's EM, by the party's Moonsign level.
    em: f64,
}

const TWO: ModKey = ModKey::new("silken-moon-2pc");
const DEVOTION_EM: ModKey = ModKey::new("gleaming-moon-devotion-em");
const DEVOTION_REACT: ModKey = ModKey::new("gleaming-moon-devotion-reaction");

impl SetImpl for Set {
    const KEY: &'static str = "silkenmoonsserenade";
    type State = State;
    fn new_state() -> State {
        State { em: 0.0 }
    }

    fn init(_: &mut Sim, _: usize, _: usize, _: i32) {}

    /// gcsim adds all of it in Set.Init, after every character is set up:
    /// ER +20%; with four pieces and the party Moonsign, the wearer's
    /// elemental damage gives the whole party EM +60 (+120 at Ascendant
    /// Gleam's level) for 8 s, and while anyone has it, lunar reactions
    /// take +10%.
    fn post_init(s: &mut Sim, ci: usize, si: usize) {
        let count = s.chars[ci].sets[si].count;
        s.chars[ci].mods.push(Mod::stat(TWO, st::ER, 0.2, false, false, -1));
        if count < 4 {
            return;
        }
        let level: i32 = s.chars.iter().map(|c| c.moonsign).sum();
        let em = match level {
            0 => return,
            1 => 60.0,
            _ => 120.0,
        };
        s.set_st_mut::<State>(ci, si).em = em;
        let o = Owner::Set(ci as u8, si as u8);
        s.subscribe(EventKind::EnemyDamage, o);
        for c in 0..s.chars.len() {
            s.add_mod(c, Mod::owned(DEVOTION_REACT, ModKind::ReactBonus, o, st::NONE, false, false, -1), -1);
        }
    }

    fn react_bonus(s: &Sim, _: usize, _: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if !info.tag.is_lunar() {
            return 0.0;
        }
        if (0..s.chars.len()).any(|c| s.mod_active(c, DEVOTION_EM)) {
            0.1
        } else {
            0.0
        }
    }

    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if !matches!(ev.info.element, Element::Pyro | Element::Hydro | Element::Electro | Element::Cryo | Element::Anemo | Element::Geo | Element::Dendro) {
            return;
        }
        if ev.info.actor as usize != ci {
            return;
        }
        let em = s.set_st::<State>(ci, si).em;
        for c in 0..s.chars.len() {
            s.add_mod(c, Mod::stat(DEVOTION_EM, st::EM, em, false, false, -1), 8 * 60);
        }
    }
}
