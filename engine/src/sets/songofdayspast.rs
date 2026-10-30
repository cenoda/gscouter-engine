//! Song of Days Past, from gcsim's internal/artifacts/songofdayspast. Its
//! values are gcsim's Flags.Custom and Status, shared by every wearer.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("sodp-2pc");
const YEARNING: ModKey = ModKey::new("yearning-effect");
const HEAL_STACKS: ModKey = ModKey::new("sodp-heal-stacks");
const HEAL_SNAP: ModKey = ModKey::new("sodp-heal-snap");
const WAVES: ModKey = ModKey::new("waves-of-days-past");

const TASK_WAVES: u8 = 0;

impl SetImpl for Set {
    const KEY: &'static str = "songofdayspast";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::HEAL, 0.15, false, false, -1));
        if count >= 4 {
            let o = Owner::Set(ci as u8, si as u8);
            s.subscribe(EventKind::Heal, o);
            s.subscribe(EventKind::EnemyHit, o);
        }
    }

    /// Six seconds after the wearer's first heal, Waves of Days Past: the
    /// next five hits by the active character deal 8% of what the wearer
    /// healed meanwhile (up to 15,000) as extra damage, within 10 s.
    fn task(s: &mut Sim, _: usize, _: usize, _: u8, _: i32) {
        s.set_flag(HEAL_SNAP, s.flag(HEAL_STACKS));
        s.set_flag(HEAL_STACKS, 0.0);
        s.add_core_status(WAVES, 10 * 60);
        s.set_flag(WAVES, 5.0);
    }

    fn event(s: &mut Sim, ci: usize, si: usize, e: &Event) {
        match *e {
            Event::Heal { caller, amount, .. } => {
                if caller != ci {
                    return;
                }
                let stacks = (s.flag(HEAL_STACKS) + amount).min(15000.0);
                s.set_flag(HEAL_STACKS, stacks);
                if s.core_status("yearning-effect") == 0 {
                    s.add_core_status(YEARNING, 6 * 60);
                    s.add_task(Task::Set { c: ci as u8, si: si as u8, id: TASK_WAVES, arg: 0 }, 6 * 60);
                }
            }
            Event::EnemyHit(ev) => {
                if !matches!(ev.info.tag, AttackTag::ElementalBurst | AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
                    return;
                }
                if s.core_status("waves-of-days-past") == 0 || ev.info.actor != s.player.active {
                    return;
                }
                let left = s.flag(WAVES);
                if left > 0.0 {
                    s.set_flag(WAVES, left - 1.0);
                    s.hit_flat += s.flag(HEAL_SNAP) * 0.08;
                }
                if s.flag(WAVES) == 0.0 {
                    s.delete_core_status(WAVES);
                }
            }
            _ => {}
        }
    }
}
