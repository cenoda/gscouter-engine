//! Desert Pavilion Chronicle, from gcsim's
//! internal/artifacts/desertpavilionchronicle.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("desert-2pc");
const SPD: ModKey = ModKey::new("desert-4pc-spd");
const DMG: ModKey = ModKey::new("desert-4pc-dmg");

impl SetImpl for Set {
    const KEY: &'static str = "desertpavilionchronicle";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        if count >= 2 {
            s.chars[ci].mods.push(Mod::stat(TWO, st::ANEMOP, 0.15, false, false, -1));
        }
        if count >= 4 {
            s.subscribe(EventKind::EnemyDamage, Owner::Set(ci as u8, si as u8));
        }
    }

    /// Normal attack SPD +10% while the player is in a normal attack (as
    /// a normal attack's speed is read when it starts, that is during the
    /// one before it).
    fn mod_amount(s: &Sim, _: usize, _: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if s.current_state() != AnimState::Normal {
            return false;
        }
        out[st::ATKSPD] += 0.1;
        true
    }

    /// Normal, charged and plunging attack DMG +40%.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            ev.stats[st::DMGP] += 0.4;
        }
    }

    /// The wearer's charged attack damage: both for 15 s (hitlag).
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || ev.info.tag != AttackTag::Extra {
            return;
        }
        let o = Owner::Set(ci as u8, si as u8);
        s.add_mod(ci, Mod::owned(SPD, ModKind::Stat, o, st::NONE, false, true, -1), 15 * 60);
        s.add_mod(ci, Mod::owned(DMG, ModKind::Attack, o, st::NONE, false, true, -1), 15 * 60);
    }
}
