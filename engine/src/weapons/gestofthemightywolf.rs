//! Gest of the Mighty Wolf, from gcsim's internal/weapons/claymore/gestofthemightywolf.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const ATKSPD_KEY: ModKey = ModKey::new("gest-of-the-mighty-wolf-atkspd");
const STACKS_KEY: ModKey = ModKey::new("gest-of-the-mighty-wolf-stacks");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "gestofthemightywolf";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::stat(ATKSPD_KEY, st::ATKSPD, 0.10, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
        s.subscribe(EventKind::StateChange, o);
        s.subscribe(EventKind::Skill, o);
        // gcsim gives no stacks for a charged attack's later segments
        // (OnStateChange's segmented), which the engine's state change
        // does not carry; only these characters' charged attacks have them
        if matches!(s.chars[ci].kind, CharKind::Noelle | CharKind::Itto | CharKind::Mavuika) {
            s.unsupported("weapon gestofthemightywolf on a character with segmented charged attacks (noelle, itto, mavuika)");
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let r = s.chars[ci].refine as f64;
        let stacks = s.weapon_st::<State>(ci).stacks as f64;
        let bonus = (0.055 + 0.02 * r) * stacks;
        out[st::DMGP] += bonus;
        if s.hexerei_count() >= 2 {
            out[st::CD] += bonus;
        }
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize != ci || s.player.active as usize != ci || ev.info.tag != AttackTag::Normal {
                    return;
                }
                Self::add_stacks(s, ci, 1);
            }
            Event::StateChange(AnimState::Charge) => {
                if s.player.active as usize != ci {
                    return;
                }
                Self::add_stacks(s, ci, 2);
            }
            Event::Skill => {
                if s.player.active as usize != ci {
                    return;
                }
                Self::add_stacks(s, ci, 2);
            }
            _ => {}
        }
    }
}

impl Weapon {
    fn add_stacks(s: &mut Sim, ci: usize, amt: i32) {
        let active = s.mod_active(ci, STACKS_KEY);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        st.stacks = (st.stacks + amt).min(4);
        s.add_mod(ci, Mod::owned(STACKS_KEY, ModKind::Stat, Owner::Weapon(ci as u8), st::NONE, false, false, -1), 240);
    }
}
