//! Wine and Song, from gcsim's internal/weapons/catalyst/wine.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const BUFF: ModKey = ModKey::new("wineandsong");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "wineandsong";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Dash, o);
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        out[st::ATKP] += 0.15 + 0.05 * r;
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Dash => {
                if s.player.active as usize == ci {
                    s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 300);
                }
            }
            Event::EnemyDamage(ev) => {
                if s.player.active as usize != ci || ev.info.actor as usize != ci || ev.info.tag != AttackTag::Normal {
                    return;
                }
                // gcsim's AddStamPercentMod("wineandsong-<char>", 300): the
                // player's dash stamina -(0.12 + 0.02 r)
                let r = s.chars[ci].refine as f64;
                s.wine_stam[ci] = (s.f + 300, -(0.12 + 0.02 * r));
            }
            _ => {}
        }
    }
}
