//! Breezeborne Refrain, from gcsim's internal/weapons/bow/breezebornerefrain.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const ER_BUFF: ModKey = ModKey::new("breezeborne-refrain-er");
const VIPER_KEY: ModKey = ModKey::new("thus-lied-the-viper");
const STACK_ICD: ModKey = ModKey::new("hymn-of-the-pure-stack-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "breezebornerefrain";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(ER_BUFF, st::ER, 0.15 + 0.05 * r, false, false, -1));
        s.subscribe(EventKind::EnemyHit, Owner::Weapon(ci as u8));
    }

    fn react_bonus(s: &Sim, ci: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if m.key == VIPER_KEY && matches!(info.tag, AttackTag::DirectStellarConduct | AttackTag::DirectStellarSwirl | AttackTag::ReactionStellarSwirl) {
            0.18 + 0.06 * s.chars[ci].refine as f64
        } else {
            0.0
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst) {
            return;
        }
        if s.mod_active(ci, VIPER_KEY) || s.mod_active(ci, STACK_ICD) {
            return;
        }
        let stacks = {
            let st = s.weapon_st_mut::<State>(ci);
            st.stacks += 1;
            st.stacks
        };
        s.add_status(ci, STACK_ICD, 2, true);
        if stacks < 3 {
            return;
        }
        s.weapon_st_mut::<State>(ci).stacks = 0;
        s.add_status(ci, VIPER_KEY, 12 * 60, true);
        for j in 0..s.chars.len() {
            s.add_mod(j, Mod::owned(VIPER_KEY, ModKind::ReactBonus, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 12 * 60);
        }
    }
}
