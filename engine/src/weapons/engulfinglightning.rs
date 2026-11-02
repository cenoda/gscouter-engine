//! Engulfing Lightning, from gcsim's internal/weapons/spear/engulfing.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("engulfing-lightning");
const ER: ModKey = ModKey::new("engulfing-er");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "engulfinglightning";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Stat, o, st::ATKP, false, false, -1));
        s.subscribe(EventKind::Burst, o);
    }

    /// ATK +28 to 56% of the ER above 100% (non-extra), at most 80 to 120%.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let er = (s.stat_ex(ci, st::ER, true) - 1.0).max(0.0);
        out[st::ATKP] += ((0.21 + 0.07 * r) * er).min(0.7 + 0.1 * r);
        true
    }

    /// A burst on field: ER +30 to 50% for 12 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Burst = *e {
            if s.player.active as usize == ci {
                let er = 0.25 + 0.05 * s.chars[ci].refine as f64;
                s.add_mod(ci, Mod::stat(ER, st::ER, er, false, true, -1), 720);
            }
        }
    }
}
