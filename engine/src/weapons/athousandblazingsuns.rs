//! A Thousand Blazing Suns, from gcsim's internal/weapons/claymore/athousandblazingsuns.

use crate::content::{Event, WeaponImpl};
use crate::nightsoul::NIGHTSOUL_BLESSING;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    extended: i32,
    tick_src: i32,
}

const BUFF_ICD: ModKey = ModKey::new("athousandblazingsuns-buff-icd");
const EXTEND_ICD: ModKey = ModKey::new("athousandblazingsuns-extend-icd");
const BUFF: ModKey = ModKey::new("athousandblazingsuns-buff");

const TICK: i32 = 18;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "athousandblazingsuns";
    type State = State;
    fn new_state() -> State {
        State { extended: 0, tick_src: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Skill, o);
        s.subscribe(EventKind::Burst, o);
        s.subscribe(EventKind::EnemyDamage, o);
        s.subscribe(EventKind::Swap, o);
    }

    /// Scorching Brilliance: ATK +28 to 56% and CRIT DMG +20 to 40%, 75%
    /// more in the Nightsoul's Blessing.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let mut atk = 0.21 + 0.07 * r;
        let mut cd = 0.15 + 0.05 * r;
        if s.mod_active(ci, NIGHTSOUL_BLESSING) {
            atk *= 1.75;
            cd *= 1.75;
        }
        out[st::ATKP] += atk;
        out[st::CD] += cd;
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // a skill or burst by the holder on field, once every 10 s
            // (hitlag): the buff for 6 s (hitlag)
            Event::Skill | Event::Burst => {
                if s.player.active as usize != ci || s.mod_active(ci, BUFF_ICD) {
                    return;
                }
                s.add_status(ci, BUFF_ICD, 10 * 60, true);
                s.weapon_st_mut::<State>(ci).extended = 0;
                s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 6 * 60);
            }
            // an elemental normal or charged hit by the holder on field,
            // once a second (hitlag), three times a buff: 2 s more
            Event::EnemyDamage(ev) => {
                if s.player.active as usize != ci || ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
                    return;
                }
                if matches!(ev.info.element, Element::Physical | Element::NoElement) {
                    return;
                }
                if s.weapon_st::<State>(ci).extended >= 3 || !s.mod_active(ci, BUFF) || s.mod_active(ci, EXTEND_ICD) {
                    return;
                }
                s.weapon_st_mut::<State>(ci).extended += 1;
                s.add_status(ci, EXTEND_ICD, 60, true);
                s.extend_mod(ci, BUFF, 2 * 60);
            }
            // off field in the Nightsoul's Blessing, the buff does not run
            // down: every 0.3 s it is extended by 0.3 s
            Event::Swap { prev, next } => {
                if prev == ci && s.mod_active(ci, BUFF) {
                    let src = s.f;
                    s.weapon_st_mut::<State>(ci).tick_src = src;
                    tick(s, ci, src);
                } else if next == ci {
                    s.weapon_st_mut::<State>(ci).tick_src = -1;
                }
            }
            _ => {}
        }
    }

    fn task(s: &mut Sim, ci: usize, _: u8, src: i32) {
        tick(s, ci, src);
    }
}

fn tick(s: &mut Sim, ci: usize, src: i32) {
    if src != s.weapon_st::<State>(ci).tick_src {
        return;
    }
    if s.mod_active(ci, NIGHTSOUL_BLESSING) {
        if !s.mod_active(ci, BUFF) {
            return;
        }
        s.extend_mod(ci, BUFF, TICK);
    }
    s.add_task(Task::Weapon { c: ci as u8, id: 0, arg: src }, TICK);
}
