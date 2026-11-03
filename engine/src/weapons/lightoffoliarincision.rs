//! Light of Foliar Incision, from gcsim's internal/weapons/sword/foliar.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    procs: i32,
}

const CRIT: ModKey = ModKey::new("foliar-crit-rate");
const BUFF: ModKey = ModKey::new("foliar-whitemoon-bristle");
const ICD: ModKey = ModKey::new("foliar-icd");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "lightoffoliarincision";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    /// CRIT Rate +4 to 8%.
    fn init(s: &mut Sim, ci: usize) {
        let cr = 0.03 + s.chars[ci].refine as f64 * 0.01;
        s.chars[ci].mods.push(Mod::stat(CRIT, st::CR, cr, false, false, -1));
        s.subscribe(EventKind::EnemyHit, Owner::Weapon(ci as u8));
    }

    /// The holder's normal or skill hits on field: out of a 12 s cooldown
    /// (hitlag), an elemental normal hit starts Whitemoon Bristle for 12 s
    /// (hitlag) and 28 uses; while it lasts, each such hit after it gains
    /// 120 to 240% of EM as flat damage and uses one.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if s.player.active as usize != ci || ev.info.actor as usize != ci {
            return;
        }
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            return;
        }
        if !s.mod_active(ci, ICD) && ev.info.tag == AttackTag::Normal && ev.info.element != Element::Physical {
            s.add_status(ci, BUFF, 720, true);
            s.add_status(ci, ICD, 720, true);
            s.weapon_st_mut::<State>(ci).procs = 28;
            return;
        }
        if !s.mod_active(ci, BUFF) {
            return;
        }
        let boost = 0.9 + 0.3 * s.chars[ci].refine as f64;
        s.hit_flat += s.stat(ci, st::EM) * boost;
        let st = s.weapon_st_mut::<State>(ci);
        st.procs -= 1;
        if st.procs <= 0 {
            s.delete_mod(ci, BUFF);
        }
    }
}
