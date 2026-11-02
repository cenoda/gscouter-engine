//! Key of Khaj-Nisut, from gcsim's internal/weapons/sword/keyofkhajnisut.
//!
//! gcsim subscribes its hit hook under one key for every holder, so a
//! second holder's would replace the first's: two holders are an error.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    stacks: i32,
    /// The EM of the holder's buff and of the party's, as gcsim computes
    /// them when it adds them.
    em: f64,
    em_team: f64,
}

const HPP: ModKey = ModKey::new("khaj-nisut");
const BUFF: ModKey = ModKey::new("khaj-nisut-buff");
const TEAM: ModKey = ModKey::new("khaj-nisut-team-buff");
const ICD: ModKey = ModKey::new("khaj-nisut-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "keyofkhajnisut";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// HP +20 to 40%.
    fn init(s: &mut Sim, ci: usize) {
        let hp = 0.15 + 0.05 * s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(HPP, st::HPP, hp, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn post_init(s: &mut Sim, ci: usize) {
        let kind = s.chars[ci].weapon;
        if s.chars.iter().filter(|c| c.weapon == kind).count() > 1 {
            s.unsupported("weapon keyofkhajnisut on two characters");
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let st = s.weapon_st::<State>(ci);
        out[st::EM] += if m.key == BUFF { st.em } else { st.em_team };
        true
    }

    /// A skill hit by the holder on field, once every 0.3 s (hitlag): a
    /// stack (up to 3, counted again from 0 once the buff has gone) of EM
    /// +0.12 to 0.24% of max HP for 20 s (hitlag); at three, every party
    /// member gets EM +0.2 to 0.4% of the holder's max HP for 20 s
    /// (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if s.player.active as usize != ci || s.mod_active(ci, ICD) || ev.info.actor as usize != ci {
            return;
        }
        if !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            return;
        }
        let active = s.mod_active(ci, BUFF);
        let r = s.chars[ci].refine as f64;
        let max_hp = s.max_hp(ci);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks = 0;
        }
        if st.stacks < 3 {
            st.stacks += 1;
        }
        st.em = max_hp * (0.0009 + 0.0003 * r) * st.stacks as f64;
        let full = st.stacks == 3;
        let o = Owner::Weapon(ci as u8);
        s.add_mod(ci, Mod::owned(BUFF, ModKind::Stat, o, st::EM, true, true, -1), 1200);
        if full {
            s.weapon_st_mut::<State>(ci).em_team = max_hp * (0.0015 + 0.0005 * r);
            for j in 0..s.chars.len() {
                s.add_mod(j, Mod::owned(TEAM, ModKind::Stat, o, st::EM, true, true, -1), 1200);
            }
        }
        s.add_status(ci, ICD, 18, true);
    }
}
