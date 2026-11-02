//! Ballad of the Boundless Blue, from gcsim's
//! internal/weapons/catalyst/balladoftheboundlessblue.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const BUFF: ModKey = ModKey::new("ballad-of-the-boundless-blue-dmgp");
const ICD: ModKey = ModKey::new("ballad-of-the-boundless-blue-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "balladoftheboundlessblue";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// Per stack, normal attack DMG +8 to 16% and charged attack DMG +6 to
    /// 12%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        let r = s.chars[ci].refine as f64;
        let stacks = s.weapon_st::<State>(ci).stacks as f64;
        match ev.info.tag {
            AttackTag::Normal => ev.stats[st::DMGP] += (0.06 + 0.02 * r) * stacks,
            AttackTag::Extra => ev.stats[st::DMGP] += (0.045 + 0.015 * r) * stacks,
            _ => {}
        }
    }

    /// Any hit by the holder on field, once every 0.3 s (hitlag): a stack,
    /// up to 3, counted again from 0 once the buff has gone; the buff (6 s,
    /// hitlag) is added or refreshed only by a normal or charged hit, as in
    /// gcsim.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        let active = s.mod_active(ci, BUFF);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        st.stacks = (st.stacks + 1).min(3);
        s.add_status(ci, ICD, 18, true);
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            s.add_mod(ci, Mod::owned(BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 6 * 60);
        }
    }
}
