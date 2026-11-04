//! Mitternachts Waltz, from gcsim's internal/weapons/bow/mitternachtswaltz.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    /// gcsim's buffIcd: no trigger until the frame after this one.
    buff_icd: i32,
}

const SKILL_BUFF: ModKey = ModKey::new("mitternachtswaltz-ele");
const NA_BUFF: ModKey = ModKey::new("mitternachtswaltz-na");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "mitternachtswaltz";
    type State = State;
    fn new_state() -> State {
        State { buff_icd: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        let buff = 0.15 + 0.05 * s.chars[ci].refine as f64;
        if m.key == SKILL_BUFF && matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            ev.stats[st::DMGP] += buff;
        } else if m.key == NA_BUFF && ev.info.tag == AttackTag::Normal {
            ev.stats[st::DMGP] += buff;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        // any hit of the holder's starts the ICD, whatever its tag
        let f = s.f;
        let st = s.weapon_st_mut::<State>(ci);
        if f <= st.buff_icd {
            return;
        }
        st.buff_icd = f + 1;
        if ev.info.tag == AttackTag::Normal {
            s.add_mod(ci, Mod::owned(SKILL_BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 300);
        } else if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            s.add_mod(ci, Mod::owned(NA_BUFF, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 300);
        }
    }
}
