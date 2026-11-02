//! Verdict, from gcsim's internal/weapons/claymore/verdict.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const ATK: ModKey = ModKey::new("verdict-atk");
const BUFF: ModKey = ModKey::new("verdict-skill-dmg");
const WINDOW: ModKey = ModKey::new("verdict-dmg-window");
const LCR_ICD: ModKey = ModKey::new("verdict-lcr-icd");

/// A Seal: up to 2, counted again from 0 once the buff has gone; the buff
/// for 15 s (hitlag).
fn gain_seal(s: &mut Sim, ci: usize) {
    let active = s.mod_active(ci, BUFF);
    let st = s.weapon_st_mut::<State>(ci);
    if !active {
        st.stacks = 0;
    }
    if st.stacks < 2 {
        st.stacks += 1;
    }
    s.add_status(ci, BUFF, 15 * 60, true);
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "verdict";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// ATK +20 to 40%.
    fn init(s: &mut Sim, ci: usize) {
        let atk = 0.15 + s.chars[ci].refine as f64 * 0.05;
        s.chars[ci].mods.push(Mod::stat(ATK, st::ATKP, atk, false, false, -1));
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Shielded, o);
        s.subscribe(EventKind::Reaction, o);
        s.subscribe(EventKind::EnemyHit, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // any crystallize shield made (by anyone): a Seal
            Event::Shielded(sh) => {
                if sh.typ == ShieldType::Crystallize {
                    gain_seal(s, ci);
                }
            }
            // Lunar-Crystallize (by anyone), once every second (hitlag): a
            // Seal
            Event::Reaction(Reaction::LunarCrystallize, _) => {
                if s.mod_active(ci, LCR_ICD) {
                    return;
                }
                s.add_status(ci, LCR_ICD, 60, true);
                gain_seal(s, ci);
            }
            // a skill hit by the holder while the buff lasts: skill DMG
            // +18 to 36% per Seal; the first opens a 0.2 s window (hitlag)
            // after which the Seals are gone
            Event::EnemyHit(ev) => {
                if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) || !s.mod_active(ci, BUFF) {
                    return;
                }
                if !s.mod_active(ci, WINDOW) {
                    s.add_status(ci, WINDOW, 12, true);
                    s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: 0, arg: 0 }, 12);
                }
                let r = s.chars[ci].refine as f64;
                s.hit_dmgp += (0.135 + r * 0.045) * s.weapon_st::<State>(ci).stacks as f64;
            }
            _ => {}
        }
    }

    fn task(s: &mut Sim, ci: usize, _: u8, _: i32) {
        s.delete_mod(ci, BUFF);
        s.weapon_st_mut::<State>(ci).stacks = 0;
    }
}
