//! Mistsplitter Reforged, from gcsim's internal/weapons/sword/mistsplitter.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("mistsplitter");
const NORMAL: ModKey = ModKey::new("mistsplitter-normal");
const BURST: ModKey = ModKey::new("mistsplitter-burst");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "mistsplitterreforged";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(KEY, ModKind::Stat, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
        s.subscribe(EventKind::Burst, o);
    }

    /// Every elemental DMG +12 to 24%, and the holder's element 8 to 16%
    /// per emblem (energy not full, a normal or charged elemental hit
    /// within 5 s, a burst within 10 s), 4 to 8% more at three.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let c = &s.chars[ci];
        let r = c.refine as f64;
        let base = 0.09 + r * 0.03;
        let mut count = 0;
        if c.energy < c.energy_max || c.energy_max == 0.0 {
            count += 1;
        }
        if s.mod_active(ci, NORMAL) {
            count += 1;
        }
        if s.mod_active(ci, BURST) {
            count += 1;
        }
        let mut dmg = count as f64 * (0.06 + r * 0.02);
        if count >= 3 {
            dmg += 0.03 + r * 0.01;
        }
        let bonus = c.element.dmg_stat();
        for i in st::PYROP..=st::DENDROP {
            out[i] += if i == bonus { base + dmg } else { base };
        }
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // a normal or charged elemental hit by the holder: 5 s (hitlag)
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize == ci && matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) && ev.info.element != Element::Physical {
                    s.add_status(ci, NORMAL, 300, true);
                }
            }
            // a burst on field: 10 s (hitlag)
            Event::Burst => {
                if s.player.active as usize == ci {
                    s.add_status(ci, BURST, 600, true);
                }
            }
            _ => {}
        }
    }
}
