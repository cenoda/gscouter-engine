//! Celestial Gift, from gcsim's internal/artifacts/celestialgift.
//!
//! With four pieces on a Hexerei character (gcsim's IsHexerei, which the
//! engine has for the Hexerei characters it implements): its skill on field
//! gives the party DMG +20% of its element for 20 s (+40% with Hexerei:
//! Secret Rite, two Hexerei characters), and under Secret Rite a swap gives
//! the party the incoming character's element for what is left of it.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State {
    /// The 4-piece's DMG bonus.
    strength: f64,
}

const TWO: ModKey = ModKey::new("celestialgift-2pc");
const HOLDER: ModKey = ModKey::new("celestialgift-4pc-holder");
const ACTIVE: ModKey = ModKey::new("celestialgift-4pc-active");
const ACTIVE_BUFF: ModKey = ModKey::new("celestialgift-4pc-active-buff");
const DURATION: i32 = 20 * 60;

/// gcsim's set4KeyHolder + "-" + the element's name.
fn holder_buff(e: Element) -> ModKey {
    match e {
        Element::Pyro => ModKey::new("celestialgift-4pc-holder-pyro"),
        Element::Hydro => ModKey::new("celestialgift-4pc-holder-hydro"),
        Element::Electro => ModKey::new("celestialgift-4pc-holder-electro"),
        Element::Cryo => ModKey::new("celestialgift-4pc-holder-cryo"),
        Element::Anemo => ModKey::new("celestialgift-4pc-holder-anemo"),
        Element::Geo => ModKey::new("celestialgift-4pc-holder-geo"),
        Element::Dendro => ModKey::new("celestialgift-4pc-holder-dendro"),
        _ => unreachable!("a character of element {e:?}"),
    }
}

fn qualifies(s: &Sim, ci: usize, si: usize) -> bool {
    s.chars[ci].sets[si].count >= 4 && s.chars[ci].hexerei
}

impl SetImpl for Set {
    const KEY: &'static str = "celestialgift";
    type State = State;
    fn new_state() -> State {
        State { strength: 0.0 }
    }

    fn init(s: &mut Sim, ci: usize, _: usize, _: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::ER, 0.2, false, false, -1));
    }

    /// gcsim's Set.Init. Every wearer subscribes OnCharacterSwap under the
    /// same key, so gcsim keeps one hook, at the first one's place (all
    /// alike): only the first qualifying wearer subscribes Swap here.
    fn post_init(s: &mut Sim, ci: usize, si: usize) {
        if !qualifies(s, ci, si) {
            return;
        }
        let rite = s.hexerei_count() >= 2;
        *s.set_st_mut::<State>(ci, si) = State { strength: if rite { 0.4 } else { 0.2 } };
        let o = Owner::Set(ci as u8, si as u8);
        s.subscribe(EventKind::Skill, o);
        if !rite {
            return;
        }
        let first = (0..s.chars.len()).flat_map(|c| (0..s.chars[c].sets.len()).map(move |k| (c, k))).find(|&(c, k)| s.chars[c].sets[k].kind == s.chars[ci].sets[si].kind && qualifies(s, c, k));
        if first == Some((ci, si)) {
            s.subscribe(EventKind::Swap, o);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, si: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        // the swap buff counts only while a wearer's skill status lasts
        if m.key == ACTIVE_BUFF && !(0..s.chars.len()).any(|c| s.mod_active(c, ACTIVE)) {
            return false;
        }
        out[m.affected] += s.set_st::<State>(ci, si).strength;
        true
    }

    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let n = s.chars.len();
        let o = Owner::Set(ci as u8, si as u8);
        match *e {
            Event::Skill => {
                if s.player.active as usize != ci {
                    return;
                }
                s.add_status(ci, ACTIVE, DURATION, true);
                let ele = s.chars[ci].element;
                // another member of its element already holds the buff
                if (0..n).any(|c| c != ci && s.mod_active(c, HOLDER) && s.chars[c].element == ele) {
                    return;
                }
                s.add_status(ci, HOLDER, DURATION, true);
                for c in 0..n {
                    s.add_mod(c, Mod::owned(holder_buff(ele), ModKind::Stat, o, ele.dmg_stat(), false, true, -1), DURATION);
                }
                for c in 0..n {
                    s.delete_mod(c, ACTIVE_BUFF);
                }
            }
            Event::Swap { .. } => {
                for c in 0..n {
                    s.delete_mod(c, ACTIVE_BUFF);
                }
                let left = (0..n).map(|c| s.mod_duration(c, "celestialgift-4pc-active")).max().unwrap_or(0);
                if left == 0 {
                    return;
                }
                let ele = s.chars[s.player.active as usize].element;
                // the swap buff does not stack with a holder's buff
                if (0..n).any(|c| s.mod_active(c, HOLDER) && s.chars[c].element == ele) {
                    return;
                }
                for c in 0..n {
                    s.add_mod(c, Mod::owned(ACTIVE_BUFF, ModKind::Stat, o, ele.dmg_stat(), false, true, -1), left);
                }
            }
            _ => {}
        }
    }
}
