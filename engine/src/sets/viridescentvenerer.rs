//! Viridescent Venerer, from gcsim's internal/artifacts/viridescent.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("vv-2pc");
const FOUR: ModKey = ModKey::new("vv-4pc");

/// gcsim's "vv<element>" shred keys.
fn key(e: Element) -> ModKey {
    match e {
        Element::Pyro => ModKey::new("vvpyro"),
        Element::Hydro => ModKey::new("vvhydro"),
        Element::Electro => ModKey::new("vvelectro"),
        _ => ModKey::new("vvcryo"),
    }
}

fn is_swirl(tag: AttackTag) -> bool {
    matches!(tag, AttackTag::SwirlPyro | AttackTag::SwirlHydro | AttackTag::SwirlElectro)
}

impl SetImpl for Set {
    const KEY: &'static str = "viridescentvenerer";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::ANEMOP, 0.15, false, false, -1));
        if count < 4 {
            return;
        }
        let o = Owner::Set(ci as u8, si as u8);
        s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::ReactBonus, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::Reaction, o);
        // the hook for swirl's hits on other enemies, which also refreshes
        // the shred when swirl's own hit lands
        s.subscribe(EventKind::EnemyDamage, o);
    }

    /// Swirl DMG +60%, unless amplified.
    fn react_bonus(_: &Sim, _: usize, _: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if !info.amped && is_swirl(info.tag) { 0.6 } else { 0.0 }
    }

    /// A swirl by the holder on field shreds the swirled element by 40% for
    /// 10 s.
    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let ele = match *e {
            Event::Reaction(r, ev) if ev.info.actor as usize == ci => match r {
                Reaction::SwirlPyro => Element::Pyro,
                Reaction::SwirlHydro => Element::Hydro,
                Reaction::SwirlElectro => Element::Electro,
                _ => return,
            },
            Event::EnemyDamage(ev) if ev.info.actor as usize == ci && is_swirl(ev.info.tag) => ev.info.element,
            _ => return,
        };
        if s.player.active as usize != ci {
            return;
        }
        s.enemy_add_resist(key(ele), ele, -0.4, 10 * 60);
    }
}
