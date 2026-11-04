//! Beacon of the Reed Sea, from gcsim's internal/weapons/claymore/beacon.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const HP_BUFF: ModKey = ModKey::new("beacon-of-the-reed-sea-hp");
const SKILL_BUFF: ModKey = ModKey::new("beacon-of-the-reed-sea-skill");
const DAMAGED_BUFF: ModKey = ModKey::new("beacon-of-the-reed-sea-damaged");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "beaconofthereedsea";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(HP_BUFF, ModKind::Stat, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
        s.subscribe(EventKind::Drain, o);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        if m.key == HP_BUFF {
            if !s.is_shielded(ci) {
                out[st::HPP] += 0.24 + 0.08 * r;
                return true;
            }
        } else if m.key == SKILL_BUFF || m.key == DAMAGED_BUFF {
            out[st::ATKP] += 0.15 + 0.05 * r;
            return true;
        }
        false
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
                    return;
                }
                s.add_mod(ci, Mod::owned(SKILL_BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 480);
            }
            Event::Drain { on, amount, external } => {
                if on != ci || amount <= 0.0 || !external {
                    return;
                }
                s.add_mod(ci, Mod::owned(DAMAGED_BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 480);
            }
            _ => {}
        }
    }
}
