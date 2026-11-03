//! Earth Shaker, from gcsim's internal/weapons/claymore/earthshaker.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("earth-shaker");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "earthshaker";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Reaction, Owner::Weapon(ci as u8));
    }

    /// Skill DMG +16 to 32%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            ev.stats[st::DMGP] += 0.12 + 0.04 * s.chars[ci].refine as f64;
        }
    }

    /// A pyro reaction by anyone (overload, vaporize, melt, pyro swirl,
    /// pyro crystallize or burning on the enemy; burgeon on any target):
    /// the buff for 8 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        match r {
            Reaction::Burgeon => {}
            Reaction::Overload | Reaction::Vaporize | Reaction::Melt | Reaction::SwirlPyro | Reaction::CrystallizePyro | Reaction::Burning => {
                if !ev.on_enemy() {
                    return;
                }
            }
            _ => return,
        }
        s.add_mod(ci, Mod::owned(KEY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 8 * 60);
    }
}
