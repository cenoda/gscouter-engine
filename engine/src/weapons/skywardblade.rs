//! Skyward Blade, from gcsim's internal/weapons/sword/skyward.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const CRIT: ModKey = ModKey::new("skyward-blade-crit");
const BUFF: ModKey = ModKey::new("skyward-blade");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "skywardblade";
    type State = State;
    fn new_state() -> State {
        State
    }

    /// CRIT Rate +4 to 8%.
    fn init(s: &mut Sim, ci: usize) {
        let cr = 0.03 + s.chars[ci].refine as f64 * 0.01;
        s.chars[ci].mods.push(Mod::stat(CRIT, st::CR, cr, false, false, -1));
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Burst, o);
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // a burst on field: ATK SPD +10% for 12 s (hitlag)
            Event::Burst => {
                if s.player.active as usize == ci {
                    s.add_mod(ci, Mod::stat(BUFF, st::ATKSPD, 0.1, false, true, -1), 720);
                }
            }
            // while it lasts, each damaging normal or charged hit by the
            // holder adds 20 to 40% of ATK as physical damage on the target
            // hit, a frame later
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) || !s.mod_active(ci, BUFF) || ev.damage == 0.0 {
                    return;
                }
                let mult = 0.15 + 0.05 * s.chars[ci].refine as f64;
                let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, mult);
                ai.abil = "Skyward Blade Proc";
                ai.pattern = Pattern::Single;
                s.queue_attack(ai, 0, 0, 1);
            }
            _ => {}
        }
    }
}
