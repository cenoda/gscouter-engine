//! Kagura's Verity, from gcsim's internal/weapons/catalyst/kagura.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
}

const STACKS: ModKey = ModKey::new("kaguras-verity-stacks");
const BUFF: ModKey = ModKey::new("kaguras-verity");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "kagurasverity";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    /// The holder's attacks: skill DMG +12 to 24% per stack, and at three
    /// stacks every elemental and physical DMG +12 to 24%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.actor as usize != ci {
            return;
        }
        let dmg = 0.12 + 0.03 * (s.chars[ci].refine - 1) as f64;
        let stacks = s.weapon_st::<State>(ci).stacks;
        if stacks == 3 {
            for i in st::PYROP..=st::PHYP {
                ev.stats[i] += dmg;
            }
        }
        if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            ev.stats[st::DMGP] += dmg * stacks as f64;
        }
    }

    /// A skill by the holder on field: a stack (up to three, from none
    /// once 16 s passed since the last), the buff for 16 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Skill = *e else { return };
        if s.player.active as usize != ci {
            return;
        }
        if !s.mod_active(ci, STACKS) {
            s.weapon_st_mut::<State>(ci).stacks = 0;
        }
        s.add_status(ci, STACKS, 960, true);
        let st = s.weapon_st_mut::<State>(ci);
        if st.stacks < 3 {
            st.stacks += 1;
        }
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 960);
    }
}
