//! Mountain-Bracing Bolt, from gcsim's internal/weapons/spear/mountainbracingbolt.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BASE_BUFF: ModKey = ModKey::new("mountain-bracing-bolt-base");
const OTHER_BUFF: ModKey = ModKey::new("mountain-bracing-bolt-other");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "mountainbracingbolt";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(BASE_BUFF, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::Skill, o);
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if (m.key == BASE_BUFF || m.key == OTHER_BUFF) && matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            ev.stats[st::DMGP] += 0.09 + 0.03 * s.chars[ci].refine as f64;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Skill = *e else { return };
        if s.player.active as usize == ci {
            return;
        }
        s.add_mod(ci, Mod::owned(OTHER_BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 8 * 60);
    }
}
