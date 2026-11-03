//! Redhorn Stonethresher, from gcsim's internal/weapons/claymore/redhorn.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const DEF: ModKey = ModKey::new("redhorn-stonethrasher-def-boost");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "redhornstonethresher";
    type State = State;
    fn new_state() -> State {
        State
    }

    /// DEF +21 to 49%, always; then the OnEnemyHit subscription.
    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        s.add_stat_mod(ci, DEF, st::DEFP, 0.21 + 0.07 * r, -1, false);
        s.subscribe(EventKind::EnemyHit, Owner::Weapon(ci as u8));
    }

    /// The holder's normal and charged attacks deal 40 to 80% of its DEF
    /// more (TotalDef, extra mods included, when the hit lands).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        let def = total_def(&s.select_stats(ci, [st::BASEDEF, st::DEFP, st::DEF], false));
        s.hit_flat += def * (0.3 + 0.1 * s.chars[ci].refine as f64);
    }
}
