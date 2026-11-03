//! Uraku Misugiri, from gcsim's internal/weapons/sword/urakumisugiri.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("urakumisugiri-increase-buff");
const NA: ModKey = ModKey::new("urakumisugiri-na");
const SKILL: ModKey = ModKey::new("urakumisugiri-skill");
const DEF: ModKey = ModKey::new("urakumisugiri-def");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "urakumisugiri";
    type State = State;
    fn new_state() -> State {
        State
    }

    /// DEF +20 to 40%.
    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        let o = Owner::Weapon(ci as u8);
        let c = &mut s.chars[ci];
        c.mods.push(Mod::owned(NA, ModKind::Attack, o, st::NONE, false, false, -1));
        c.mods.push(Mod::owned(SKILL, ModKind::Attack, o, st::NONE, false, false, -1));
        c.mods.push(Mod::stat(DEF, st::DEFP, 0.15 + 0.05 * r, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
    }

    /// Normal attack DMG +16 to 32% and skill DMG +24 to 48%, doubled for
    /// 15 s (hitlag) after geo damage.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        let r = s.chars[ci].refine as f64;
        let v = if m.key == NA {
            if ev.info.tag != AttackTag::Normal {
                return;
            }
            0.12 + 0.04 * r
        } else {
            if !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
                return;
            }
            0.18 + 0.06 * r
        };
        ev.stats[st::DMGP] += if s.mod_active(ci, BUFF) { v * 2.0 } else { v };
    }

    /// Geo damage by anyone (gcsim checks no attacker).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.element == Element::Geo {
            s.add_status(ci, BUFF, 900, true);
        }
    }
}
