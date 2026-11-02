//! Long Night's Oath, from gcsim's internal/artifacts/longnightsoath.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
/// gcsim's stacks.MultipleRefreshNoRemove of 5: each stack keeps the frame
/// it was added on and goes 6 s later (on the wearer's queue, so hitlag
/// delays it); a sixth replaces the oldest. `serial` tells a slot's removal
/// task whether the stack it was queued for is still the one there.
pub struct State {
    src: [i32; 5],
    serial: [i32; 5],
}

const TWO: ModKey = ModKey::new("longnightsoath-2pc");
const FOUR: ModKey = ModKey::new("longnightsoath-4pc");
const ICD_PLUNGE: ModKey = ModKey::new("longnightsoath-icd-plunge");
const ICD_CHARGE: ModKey = ModKey::new("longnightsoath-icd-charge");
const ICD_SKILL: ModKey = ModKey::new("longnightsoath-icd-skill");

const TASK_EXPIRE: u8 = 0;
const NONE: i32 = i32::MIN;

impl State {
    fn count(&self) -> i32 {
        self.src.iter().filter(|&&f| f != NONE).count() as i32
    }
}

impl SetImpl for Set {
    const KEY: &'static str = "longnightsoath";
    type State = State;
    fn new_state() -> State {
        State { src: [NONE; 5], serial: [0; 5] }
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        let o = Owner::Set(ci as u8, si as u8);
        s.chars[ci].mods.push(Mod::owned(TWO, ModKind::Attack, o, st::NONE, false, false, -1));
        if count >= 4 {
            s.subscribe(EventKind::EnemyDamage, o);
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Attack, o, st::NONE, false, false, -1));
        }
    }

    /// Plunging attack DMG +25%, and +15% per stack with four pieces.
    fn attack_mod(s: &Sim, ci: usize, si: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag != AttackTag::Plunge {
            return;
        }
        if m.key == TWO {
            ev.stats[st::DMGP] += 0.25;
        } else {
            ev.stats[st::DMGP] += 0.15 * s.set_st::<State>(ci, si).count() as f64;
        }
    }

    /// The stack added in slot arg & 7 runs out, unless replaced since.
    fn task(s: &mut Sim, ci: usize, si: usize, _: u8, arg: i32) {
        let st = s.set_st_mut::<State>(ci, si);
        let idx = (arg & 7) as usize;
        if st.serial[idx] == arg >> 3 {
            st.src[idx] = NONE;
        }
    }

    /// The wearer's damage while on field: a plunging attack adds a stack,
    /// a charged attack or skill two, each kind once a second.
    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci {
            return;
        }
        let (icd, n) = match ev.info.tag {
            AttackTag::Plunge => (ICD_PLUNGE, 1),
            AttackTag::Extra => (ICD_CHARGE, 2),
            AttackTag::ElementalArt | AttackTag::ElementalArtHold => (ICD_SKILL, 2),
            _ => return,
        };
        if s.mod_active(ci, icd) {
            return;
        }
        s.add_status(ci, icd, 60, true);
        for _ in 0..n {
            let f = s.f;
            let st = s.set_st_mut::<State>(ci, si);
            let mut idx = 0;
            for i in 0..5 {
                if st.src[i] == NONE {
                    idx = i;
                    break;
                }
                if st.src[i] < st.src[idx] {
                    idx = i;
                }
            }
            st.src[idx] = f;
            st.serial[idx] += 1;
            let arg = st.serial[idx] << 3 | idx as i32;
            s.queue_char_task(ci, Task::Set { c: ci as u8, si: si as u8, id: TASK_EXPIRE, arg }, 6 * 60);
        }
    }
}
