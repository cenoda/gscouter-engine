//! Splendor of Tranquil Waters, from gcsim's
//! internal/weapons/sword/splendoroftranquilwaters.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    skill_stacks: i32,
    hp_stacks: i32,
}

const SKILL: ModKey = ModKey::new("splendoroftranquilwaters-skill-buff");
const SKILL_ICD: ModKey = ModKey::new("splendoroftranquilwaters-skill-buff-icd");
const HP: ModKey = ModKey::new("splendoroftranquilwaters-hp-buff");
const HP_ICD: ModKey = ModKey::new("splendoroftranquilwaters-hp-icd");

/// The holder's HP changed, once every 0.2 s (hitlag): a stack of skill
/// DMG +8 to 16% for 6 s (hitlag), up to 3, counted again from 0 once
/// the buff has gone.
fn on_equip_change(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, SKILL_ICD) {
        return;
    }
    let active = s.mod_active(ci, SKILL);
    let st = s.weapon_st_mut::<State>(ci);
    if !active {
        st.skill_stacks = 0;
    }
    if st.skill_stacks < 3 {
        st.skill_stacks += 1;
    }
    s.add_status(ci, SKILL_ICD, 12, true);
    s.add_mod(ci, Mod::owned(SKILL, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 6 * 60);
}

/// Another character's HP changed, once every 0.2 s (hitlag): a stack of
/// HP +14 to 28% for 6 s (hitlag), up to 2.
fn on_other_change(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, HP_ICD) {
        return;
    }
    let active = s.mod_active(ci, HP);
    let st = s.weapon_st_mut::<State>(ci);
    if !active {
        st.hp_stacks = 0;
    }
    if st.hp_stacks < 2 {
        st.hp_stacks += 1;
    }
    s.add_status(ci, HP_ICD, 12, true);
    s.add_mod(ci, Mod::owned(HP, ModKind::Stat, Owner::Weapon(ci as u8), st::HPP, false, true, -1), 6 * 60);
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "splendoroftranquilwaters";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.subscribe(EventKind::Drain, o);
        s.subscribe(EventKind::Heal, o);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        out[st::HPP] += (0.105 + r * 0.035) * s.weapon_st::<State>(ci).hp_stacks as f64;
        true
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            let r = s.chars[ci].refine as f64;
            ev.stats[st::DMGP] += (0.06 + 0.02 * r) * s.weapon_st::<State>(ci).skill_stacks as f64;
        }
    }

    /// HP lost (more than 0) or healed (more than 0, not all overheal).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let on = match *e {
            Event::Drain { on, amount, .. } if amount > 0.0 => on,
            Event::Heal { on, amount, overheal, .. } if amount > 0.0 && (amount - overheal).abs() > 1e-9 => on,
            _ => return,
        };
        if on == ci {
            on_equip_change(s, ci);
        } else {
            on_other_change(s, ci);
        }
    }
}
