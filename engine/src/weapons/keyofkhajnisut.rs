//! Key of Khaj-Nisut, from gcsim's internal/weapons/sword/keyofkhajnisut.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    stacks: i32,
}

const HP: ModKey = ModKey::new("khaj-nisut");
const BUFF: ModKey = ModKey::new("khaj-nisut-buff");
const TEAM_BUFF: ModKey = ModKey::new("khaj-nisut-team-buff");
const ICD: ModKey = ModKey::new("khaj-nisut-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "keyofkhajnisut";
    type State = State;
    fn new_state() -> State {
        State { stacks: 0 }
    }

    /// HP +20 to 40%.
    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(HP, st::HPP, 0.15 + 0.05 * r, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    /// The holder's skill damaging the enemy with the holder on field, once
    /// in 0.3 s: a Grand Hymn stack (up to 3) for 20 s, EM of 0.12 to 0.24%
    /// of max HP each, and at 3 stacks the party's EM +0.2 to 0.4% of the
    /// holder's max HP for 20 s. Both are fixed as they are added.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if s.player.active as usize != ci || s.mod_active(ci, ICD) || ev.info.actor as usize != ci {
            return;
        }
        if !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            return;
        }
        if !s.mod_active(ci, BUFF) {
            s.weapon_st_mut::<State>(ci).stacks = 0;
        }
        let st = s.weapon_st_mut::<State>(ci);
        if st.stacks < 3 {
            st.stacks += 1;
        }
        let stacks = st.stacks;
        let r = s.chars[ci].refine as f64;
        let hp = s.max_hp(ci);
        let em = hp * (0.0009 + 0.0003 * r) * stacks as f64;
        s.add_mod(ci, Mod::stat(BUFF, st::EM, em, true, true, -1), 1200);
        if stacks == 3 {
            let team = hp * (0.0015 + 0.0005 * r);
            for i in 0..s.chars.len() {
                s.add_mod(i, Mod::stat(TEAM_BUFF, st::EM, team, true, true, -1), 1200);
            }
        }
        s.add_status(ci, ICD, 18, true);
    }
}
