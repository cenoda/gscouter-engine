//! Sunny Morning Sleep-in, from gcsim's internal/weapons/catalyst/sunnymorning.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    last_swirl_f: i32,
}

const SWIRL_BUFF: ModKey = ModKey::new("sunny-morning-swirl");
const SKILL_BUFF: ModKey = ModKey::new("sunny-morning-skill");
const BURST_BUFF: ModKey = ModKey::new("sunny-morning-burst");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "sunnymorningsleepin";
    type State = State;
    fn new_state() -> State {
        State { last_swirl_f: -1 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Reaction, o);
        s.subscribe(EventKind::EnemyHit, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let r = s.chars[ci].refine as f64;
        let mult = 0.75 + 0.25 * r;
        match *e {
            Event::Reaction(rx, ev) => {
                if ev.info.actor as usize != ci || s.player.active as usize != ci {
                    return;
                }
                let swirl = matches!(
                    rx,
                    Reaction::SwirlElectro
                        | Reaction::SwirlCryo
                        | Reaction::SwirlHydro
                        | Reaction::SwirlPyro
                        | Reaction::StellarSwirl
                );
                if !swirl {
                    return;
                }
                let f = s.f;
                let st = s.weapon_st_mut::<State>(ci);
                if st.last_swirl_f == f {
                    return;
                }
                st.last_swirl_f = f;
                s.add_mod(ci, Mod::stat(SWIRL_BUFF, st::EM, 120.0 * mult, false, true, -1), 6 * 60);
            }
            Event::EnemyHit(ev) => {
                if ev.info.actor as usize != ci || s.player.active as usize != ci {
                    return;
                }
                if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
                    s.add_mod(ci, Mod::stat(SKILL_BUFF, st::EM, 96.0 * mult, false, true, -1), 9 * 60);
                } else if ev.info.tag == AttackTag::ElementalBurst {
                    s.add_mod(ci, Mod::stat(BURST_BUFF, st::EM, 32.0 * mult, false, false, -1), 30 * 60);
                }
            }
            _ => {}
        }
    }
}
