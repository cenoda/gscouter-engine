//! Skyward Pride, from gcsim's internal/weapons/claymore/skyward.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    /// Vacuum blades since the last burst.
    counter: i32,
}

const DMG: ModKey = ModKey::new("skyward pride");
const ACTIVE: ModKey = ModKey::new("skyward-pride-active");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "skywardpride";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// DMG +8 to 16%.
    fn init(s: &mut Sim, ci: usize) {
        let dmg = 0.06 + s.chars[ci].refine as f64 * 0.02;
        s.chars[ci].mods.push(Mod::stat(DMG, st::DMGP, dmg, false, false, -1));
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Burst, o);
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // a burst on field: vacuum blades for 20 s (hitlag)
            Event::Burst => {
                if s.player.active as usize == ci {
                    s.add_status(ci, ACTIVE, 1200, true);
                    s.weapon_st_mut::<State>(ci).counter = 0;
                }
            }
            // normal and charged damage by the holder: a blade of 80 to
            // 160% of ATK on the target, up to 8
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) || !s.mod_active(ci, ACTIVE) {
                    return;
                }
                if s.weapon_st::<State>(ci).counter >= 8 {
                    return;
                }
                s.weapon_st_mut::<State>(ci).counter += 1;
                let mult = 0.6 + s.chars[ci].refine as f64 * 0.2;
                let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, mult);
                ai.abil = "Skyward Pride Proc";
                ai.pattern = s.box_on_enemy(0.0, 0.0, 0.1, 0.1);
                s.queue_attack(ai, 0, 0, 1);
            }
            _ => {}
        }
    }
}
