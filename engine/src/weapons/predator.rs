//! Predator, from gcsim's internal/weapons/bow/predator.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const ATK_PERM: ModKey = ModKey::new("predator-atk");
const STACK_KEY: ModKey = ModKey::new("predator-stacks");
const DMG_BUFF: ModKey = ModKey::new("predator-dmg");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "predator";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let passive = s.chars[ci].weapon_params.iter().find(|p| p.0 == "passive").map_or(1, |p| p.1);
        if passive != 1 {
            return;
        }
        if s.chars[ci].kind == CharKind::Aloy {
            s.chars[ci].mods.push(Mod::stat(ATK_PERM, st::ATK, 66.0, false, false, -1));
        }
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == DMG_BUFF && matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            ev.stats[st::DMGP] += 0.10 * s.weapon_st::<State>(ci).stacks as f64;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        if ev.info.element != Element::Cryo || ev.damage == 0.0 {
            return;
        }
        let active = s.mod_active(ci, STACK_KEY);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        if st.stacks < 2 {
            st.stacks += 1;
        }
        s.add_status(ci, STACK_KEY, 360, true);
        s.add_mod(ci, Mod::owned(DMG_BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 360);
    }
}
