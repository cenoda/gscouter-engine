//! Hakushin Ring, from gcsim's internal/weapons/catalyst/hakushin.
//!
//! While the holder is on field, an electro reaction their attack triggers
//! gives every party member of electro or of the reaction's other element
//! that element's DMG bonus for 6 s (10% at R1, +2.5% per refinement). Each
//! element has its own 1 s ICD, on the holder, so one proc refreshes every
//! member of an element and then locks that element. Crystallize, swirl,
//! electro-charged, overload, superconduct, quicken and aggravate count
//! only on an enemy; hyperbloom counts only from a dendro core, which is
//! the only place the engine emits it. gcsim also listens to lunar-charged
//! (hydro, like electro-charged) and stellar-conduct (cryo, like
//! superconduct). The engine never emits those reactions; whoever adds
//! them should buff them here too.

use crate::content::{Event, EventKind, Owner, WeaponImpl};
use crate::gcsimdata::weapons::catalyst_hakushin::{BUFF_KEY, ICD_KEY};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const fn same(a: &str, b: &str) -> bool {
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    if ab.len() != bb.len() {
        return false;
    }
    let mut i = 0;
    while i < ab.len() {
        if ab[i] != bb[i] {
            return false;
        }
        i += 1;
    }
    true
}

// BUFF_KEY and ICD_KEY are gcsim's formats; the keys below fill in the
// element's name (`attributes.Element.String`: pyro, hydro, ...).
const _: () = assert!(same(BUFF_KEY, "hakushin-%v-buff") && same(ICD_KEY, "hakushin-%v-icd"));

const BUFF_PYRO: ModKey = ModKey::new("hakushin-pyro-buff");
const BUFF_HYDRO: ModKey = ModKey::new("hakushin-hydro-buff");
const BUFF_DENDRO: ModKey = ModKey::new("hakushin-dendro-buff");
const BUFF_ELECTRO: ModKey = ModKey::new("hakushin-electro-buff");
const BUFF_CRYO: ModKey = ModKey::new("hakushin-cryo-buff");
const BUFF_ANEMO: ModKey = ModKey::new("hakushin-anemo-buff");
const BUFF_GEO: ModKey = ModKey::new("hakushin-geo-buff");

const ICD_PYRO: ModKey = ModKey::new("hakushin-pyro-icd");
const ICD_HYDRO: ModKey = ModKey::new("hakushin-hydro-icd");
const ICD_DENDRO: ModKey = ModKey::new("hakushin-dendro-icd");
const ICD_ELECTRO: ModKey = ModKey::new("hakushin-electro-icd");
const ICD_CRYO: ModKey = ModKey::new("hakushin-cryo-icd");
const ICD_ANEMO: ModKey = ModKey::new("hakushin-anemo-icd");
const ICD_GEO: ModKey = ModKey::new("hakushin-geo-icd");

fn buff_key(e: Element) -> ModKey {
    match e {
        Element::Pyro => BUFF_PYRO,
        Element::Hydro => BUFF_HYDRO,
        Element::Dendro => BUFF_DENDRO,
        Element::Electro => BUFF_ELECTRO,
        Element::Cryo => BUFF_CRYO,
        Element::Anemo => BUFF_ANEMO,
        Element::Geo => BUFF_GEO,
        Element::NoElement | Element::Physical | Element::Frozen | Element::Quicken => unreachable!(),
    }
}

fn icd_key(e: Element) -> ModKey {
    match e {
        Element::Pyro => ICD_PYRO,
        Element::Hydro => ICD_HYDRO,
        Element::Dendro => ICD_DENDRO,
        Element::Electro => ICD_ELECTRO,
        Element::Cryo => ICD_CRYO,
        Element::Anemo => ICD_ANEMO,
        Element::Geo => ICD_GEO,
        Element::NoElement | Element::Physical | Element::Frozen | Element::Quicken => unreachable!(),
    }
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "hakushinring";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        // every reaction the engine emits, so a new one must be placed
        let (other, gadget) = match r {
            Reaction::CrystallizeElectro => (Element::Geo, false),
            Reaction::SwirlElectro => (Element::Anemo, false),
            Reaction::ElectroCharged => (Element::Hydro, false),
            Reaction::Overload => (Element::Pyro, false),
            Reaction::Superconduct => (Element::Cryo, false),
            Reaction::Quicken | Reaction::Aggravate => (Element::Dendro, false),
            // hyperbloom's event is the attack that hit the dendro core
            Reaction::Hyperbloom => (Element::Dendro, true),
            Reaction::Vaporize
            | Reaction::Melt
            | Reaction::Frozen
            | Reaction::Shatter
            | Reaction::CrystallizeHydro
            | Reaction::CrystallizePyro
            | Reaction::CrystallizeCryo
            | Reaction::SwirlPyro
            | Reaction::SwirlHydro
            | Reaction::SwirlCryo
            | Reaction::Spread
            | Reaction::Bloom
            | Reaction::Burgeon
            | Reaction::Burning => return,
        };
        // gcsim's target check: an enemy, or a gadget for hyperbloom
        if !gadget && ev.on_player {
            return;
        }
        if s.player.active as usize != ci || ev.info.actor as usize != ci {
            return;
        }
        let dmg = 0.075 + s.chars[ci].refine as f64 * 0.025;
        // the ICD is set after the loop, so every member of an element is
        // refreshed together
        let mut did_electro = false;
        let mut did_other = false;
        let n = s.chars.len();
        for i in 0..n {
            let ele = s.chars[i].element;
            if ele != Element::Electro && ele != other {
                continue;
            }
            if s.mod_active(ci, icd_key(ele)) {
                continue;
            }
            if ele == Element::Electro {
                did_electro = true;
            } else {
                did_other = true;
            }
            s.add_stat_mod(i, buff_key(ele), ele.dmg_stat(), dmg, 6 * 60, true);
        }
        if did_electro {
            s.add_status(ci, icd_key(Element::Electro), 60, true);
        }
        if did_other {
            s.add_status(ci, icd_key(other), 60, true);
        }
    }
}
