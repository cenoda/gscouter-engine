//! Angelos Heptades, from gcsim's internal/weapons/catalyst/angelosheptades.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    /// gcsim's buffSrc: the frame of the shield that started the update
    /// loop now running.
    src: i32,
    /// gcsim's buffAmt.
    amt: f64,
}

const ATK: ModKey = ModKey::new("angelos-heptades-atk");
const ENERGY_ICD: ModKey = ModKey::new("angelos-heptades-energy-icd");
const HOLDER: ModKey = ModKey::new("angelos-heptades-buff");
/// One key for every holder's mod, so that a second Angelos Heptades
/// replaces the first's on each character, as in gcsim.
const PARTY: ModKey = ModKey::new("pathfinders-light");

const TASK_UPDATE: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "angelosheptades";
    type State = State;
    fn new_state() -> State {
        State { src: -1, amt: 0.0 }
    }

    /// ATK +12 to 24%.
    fn init(s: &mut Sim, ci: usize) {
        let atk = 0.09 + s.chars[ci].refine as f64 * 0.03;
        s.chars[ci].mods.push(Mod::stat(ATK, st::ATKP, atk, false, false, -1));
        s.subscribe(EventKind::Shielded, Owner::Weapon(ci as u8));
    }

    /// gcsim's Weapon.Init: the party's attack mod.
    fn post_init(s: &mut Sim, ci: usize) {
        for c in 0..s.chars.len() {
            mod_add(&mut s.chars[c].mods, Mod::owned(PARTY, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
        }
    }

    /// While the holder's buff lasts: DMG + the amount for the active
    /// character, half of it for a Hexerei character off field.
    fn attack_mod(s: &Sim, ci: usize, on: usize, _: &Mod, ev: &mut AttackEvent) {
        if !s.mod_active(ci, HOLDER) {
            return;
        }
        let amt = s.weapon_st::<State>(ci).amt;
        if on == s.player.active as usize {
            ev.stats[st::DMGP] += amt;
        } else if s.chars[on].hexerei {
            ev.stats[st::DMGP] += amt * 0.5;
        }
    }

    fn task(s: &mut Sim, ci: usize, _: u8, src: i32) {
        update(s, ci, src);
    }

    /// A shield of the holder's: the buff for 20 s (hitlag), its amount
    /// updated now and every 0.5 s, and 14 to 18 energy once every 14 s
    /// (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Shielded(sh) = *e else { return };
        if sh.owner as usize != ci {
            return;
        }
        let src = s.f;
        s.weapon_st_mut::<State>(ci).src = src;
        s.add_status(ci, HOLDER, 20 * 60, true);
        update(s, ci, src);
        if s.mod_active(ci, ENERGY_ICD) {
            return;
        }
        s.add_status(ci, ENERGY_ICD, 14 * 60, true);
        let energy = 13.0 + s.chars[ci].refine as f64;
        s.add_energy(ci, energy);
    }
}

/// gcsim's updateBuff: 10 to 22% per 1,000 of the holder's ATK, at most
/// 26 to 58%, then again in 0.5 s while the buff lasts.
fn update(s: &mut Sim, ci: usize, src: i32) {
    if s.weapon_st::<State>(ci).src != src {
        return;
    }
    if !s.mod_active(ci, HOLDER) {
        s.weapon_st_mut::<State>(ci).amt = 0.0;
        return;
    }
    let r = s.chars[ci].refine as f64;
    let atk = total_atk(&s.select_stats(ci, [st::BASEATK, st::ATKP, st::ATK], true));
    s.weapon_st_mut::<State>(ci).amt = (atk / 1000.0 * (0.07 + r * 0.03)).min(0.18 + r * 0.08);
    s.add_task(Task::Weapon { c: ci as u8, id: TASK_UPDATE, arg: src }, 30);
}
