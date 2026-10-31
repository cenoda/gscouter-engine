//! The Alley Flash, from gcsim's internal/weapons/sword/alley: DMG bonus,
//! locked out for 5 s when the wielder loses HP to damage from outside
//! the party (bloom's self damage, for one).

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("alleyflash");
const LOCKOUT: ModKey = ModKey::new("alley-flash-lockout");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "thealleyflash";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Drain, o);
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Stat, o, st::DMGP, false, false, -1));
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if s.mod_active(ci, LOCKOUT) {
            return false;
        }
        out[st::DMGP] += 0.09 + 0.03 * s.chars[ci].refine as f64;
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Drain { on, amount, external } = *e {
            if on == ci && amount > 0.0 && external {
                s.add_status(ci, LOCKOUT, 300, true);
            }
        }
    }
}
