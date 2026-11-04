//! Symphonist of Scents, from gcsim's internal/weapons/spear/symphonist.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const SELF: ModKey = ModKey::new("symphonist-atkp");
const BUFF: ModKey = ModKey::new("symphonist-chanson-de-baies");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "symphonistofscents";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(SELF, ModKind::Stat, o, st::ATKP, false, false, -1));
        s.subscribe(EventKind::Heal, o);
    }

    /// ATK +12 to 24%, doubled while the holder is off field.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let mut atk = 0.09 + s.chars[ci].refine as f64 * 0.03;
        if s.player.active as usize != ci {
            atk *= 2.0;
        }
        out[st::ATKP] += atk;
        true
    }

    /// A heal by the holder, on or off field: Chanson de Baies, ATK +32 to
    /// 64% for 3 s (hitlag), on the holder and on the character healed
    /// (gcsim's OnHeal comes once for each character healed).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Heal { caller, on, .. } = *e else { return };
        if caller != ci {
            return;
        }
        let v = 0.24 + s.chars[ci].refine as f64 * 0.08;
        s.add_mod(ci, Mod::stat(BUFF, st::ATKP, v, false, true, -1), 180);
        if on != ci {
            s.add_mod(on, Mod::stat(BUFF, st::ATKP, v, false, true, -1), 180);
        }
    }
}
