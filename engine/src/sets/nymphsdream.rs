//! Nymph's Dream, from gcsim's internal/artifacts/nymphsdream.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("nd-2pc");
const FOUR: ModKey = ModKey::new("nd-4pc");
const NORMAL: ModKey = ModKey::new("nd-normal");
const CHARGED: ModKey = ModKey::new("nd-charged");
const SKILL: ModKey = ModKey::new("nd-skill");
const BURST: ModKey = ModKey::new("nd-burst");
const PLUNGE: ModKey = ModKey::new("nd-plunge");

impl SetImpl for Set {
    const KEY: &'static str = "nymphsdream";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::HYDROP, 0.15, false, false, -1));
        }
        if count >= 4 {
            let o = Owner::Set(ci as u8, si as u8);
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Stat, o, st::NONE, false, false, -1));
            s.subscribe(EventKind::EnemyDamage, o);
        }
    }

    /// Mirrorlight Reflection, one per kind of attack whose status lasts
    /// (up to 3): ATK +7/16/25% and hydro DMG +4/9/15%.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let mut n = 0;
        for k in [NORMAL, CHARGED, PLUNGE, SKILL, BURST] {
            if n < 3 && s.mod_active(ci, k) {
                n += 1;
            }
        }
        if n > 0 {
            out[st::ATKP] += 0.09 * n as f64 - 0.02;
            out[st::HYDROP] += [0.04, 0.09, 0.15][n - 1];
        }
        true
    }

    /// The wearer's damage on field: its kind's status for 8 s (hitlag).
    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        let k = match ev.info.tag {
            AttackTag::Normal => NORMAL,
            AttackTag::Extra => CHARGED,
            AttackTag::Plunge => PLUNGE,
            AttackTag::ElementalArt | AttackTag::ElementalArtHold => SKILL,
            AttackTag::ElementalBurst => BURST,
            _ => return,
        };
        s.add_status(ci, k, 480, true);
    }
}
