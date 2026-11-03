//! Aquila Favonia, from gcsim's internal/weapons/sword/aquila.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ATK: ModKey = ModKey::new("aquila favonia");
const ICD: ModKey = ModKey::new("aquila-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "aquilafavonia";
    type State = State;
    fn new_state() -> State {
        State
    }

    /// ATK +20 to 40%.
    fn init(s: &mut Sim, ci: usize) {
        let atk = 0.15 + 0.05 * s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(ATK, st::ATKP, atk, false, false, -1));
        s.subscribe(EventKind::Drain, Owner::Weapon(ci as u8));
    }

    /// The holder losing HP to an external drain, once every 15 s
    /// (hitlag): 200 to 320% of ATK as physical damage around the player,
    /// and a heal of the active character for 100 to 160% of the
    /// snapshot's ATK.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Drain { on, amount, external } = *e else { return };
        if !external || amount <= 0.0 || on != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 900, true);
        let r = s.chars[ci].refine as f64;
        let mut ai = AttackInfo::new(ci, AttackTag::Weapon, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 1.7 + 0.3 * r);
        ai.abil = "Aquila Favonia";
        ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
        let ev = s.snapshot_event(ai, 0);
        s.queue_attack_with_snap(ev, 0, 1);
        let heal = total_atk(&ev.stats) * (0.85 + 0.15 * r);
        let bonus = s.stat(ci, st::HEAL);
        let active = s.player.active as usize;
        s.heal(ci, active, heal, bonus);
    }
}
