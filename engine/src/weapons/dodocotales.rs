//! Dodoco Tales, from gcsim's internal/weapons/catalyst/dodoco.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const CA_BUFF: ModKey = ModKey::new("dodoco-ca");
const ATK_BUFF: ModKey = ModKey::new("dodoco-atk");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "dodocotales";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == CA_BUFF && ev.info.tag == AttackTag::Extra {
            ev.stats[st::DMGP] += 0.12 + 0.04 * s.chars[ci].refine as f64;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        let r = s.chars[ci].refine as f64;
        match ev.info.tag {
            AttackTag::Normal => {
                s.add_mod(ci, Mod::owned(CA_BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 360);
            }
            AttackTag::Extra => {
                s.add_mod(ci, Mod::stat(ATK_BUFF, st::ATKP, 0.06 + 0.02 * r, false, true, -1), 360);
            }
            _ => {}
        }
    }
}
