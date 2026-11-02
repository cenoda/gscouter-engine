//! Mailed Flower, from gcsim's internal/weapons/claymore/mailedflower.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("mailedflower");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "mailedflower";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::EnemyDamage, o);
        s.subscribe(EventKind::Reaction, o);
    }

    /// ATK +12 to 24% and EM +48 to 96.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        out[st::ATKP] += 0.09 + r * 0.03;
        out[st::EM] += 36.0 + r * 12.0;
        true
    }

    /// The holder's skill hitting, or any reaction event by the holder
    /// (gcsim's whole reaction range, shatter included, on any target),
    /// with the holder on field: the buff for 8 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let ev = match *e {
            Event::EnemyDamage(ev) if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) => ev,
            Event::Reaction(_, ev) => ev,
            _ => return,
        };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        s.add_mod(ci, Mod::owned(KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 8 * 60);
    }
}
