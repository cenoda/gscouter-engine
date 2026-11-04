//! White Lake Frostfeather, from gcsim's internal/weapons/sword/whitelakefrostfeather.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stack_counter: usize,
}

const STACK_ICD: ModKey = ModKey::new("whitelake-frostfeather-stack-icd");
const STELLAR_BUFF: ModKey = ModKey::new("whitelake-frostfeather-stellar");
const ENERGY_ICD: ModKey = ModKey::new("whitelake-frostfeather-energy-icd");

const ATK_1: ModKey = ModKey::new("whitelake-frostfeather-atk-1");
const ATK_2: ModKey = ModKey::new("whitelake-frostfeather-atk-2");
const ATK_3: ModKey = ModKey::new("whitelake-frostfeather-atk-3");

const ATK_MODS: [ModKey; 3] = [ATK_1, ATK_2, ATK_3];

impl WeaponImpl for Weapon {
    const KEY: &'static str = "whitelakefrostfeather";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::EnemyHit, o);
        s.subscribe(EventKind::EnemyDamage, o);
        s.subscribe(EventKind::Reaction, o);
        s.subscribe(EventKind::SpecialReactionAttack, o);
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == STELLAR_BUFF && ev.info.tag.is_stellar_direct() {
            let r = s.chars[ci].refine as f64;
            ev.stats[st::CD] += 0.35 + 0.15 * r;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyHit(ev) => {
                if s.mod_active(ci, STACK_ICD) || ev.info.actor as usize != ci {
                    return;
                }
                if !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
                    return;
                }
                let st = s.weapon_st_mut::<State>(ci);
                let idx = st.stack_counter;
                let cur_key = ATK_MODS[idx];
                let oldest_key = ATK_MODS[(idx + 1) % 3];
                st.stack_counter = (idx + 1) % 3;

                s.add_status(ci, STACK_ICD, 6, true);
                let r = s.chars[ci].refine as f64;
                let atk = 0.06 + 0.02 * r;
                s.add_mod(ci, Mod::stat(cur_key, st::ATKP, atk, false, true, -1), 8 * 60);

                if s.mod_active(ci, oldest_key) {
                    let dur = s.mod_duration(ci, oldest_key.name);
                    s.add_mod(ci, Mod::owned(STELLAR_BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), dur);
                }
            }
            Event::EnemyDamage(ev) => {
                if ev.info.tag.is_stellar() && ev.info.actor as usize == ci {
                    Self::refund_energy(s, ci);
                }
            }
            Event::Reaction(r, ev) => {
                if ev.info.actor as usize == ci && matches!(r, Reaction::StellarConduct | Reaction::StellarSwirl) {
                    Self::refund_energy(s, ci);
                }
            }
            Event::SpecialReactionAttack(ev) => {
                if s.mod_active(ci, STELLAR_BUFF) && ev.info.tag.is_stellar_react() {
                    let r = s.chars[ci].refine as f64;
                    s.special_atk.stats[st::CD] += 0.35 + 0.15 * r;
                }
            }
            _ => {}
        }
    }
}

impl Weapon {
    fn refund_energy(s: &mut Sim, ci: usize) {
        if s.mod_active(ci, ENERGY_ICD) {
            return;
        }
        let r = s.chars[ci].refine as f64;
        s.add_energy(ci, 3.5 + 0.5 * r);
        s.add_status(ci, ENERGY_ICD, 210, true);
    }
}
