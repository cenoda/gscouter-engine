//! Retracing Bolide, from gcsim's internal/artifacts/bolide.

use crate::content::SetImpl;
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("bolide-2pc");
const FOUR: ModKey = ModKey::new("bolide-4pc");

impl SetImpl for Set {
    const KEY: &'static str = "retracingbolide";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        let o = Owner::Set(ci as u8, si as u8);
        if count >= 2 {
            s.shield_bonus_mods.push(ShieldBonusMod { key: TWO, owner: o, expiry: -1 });
        }
        if count >= 4 {
            s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Attack, o, st::NONE, false, false, -1));
        }
    }

    /// Shield strength +35%, for the party's shields.
    fn shield_bonus(_: &Sim, _: usize, _: usize, _: ModKey) -> (f64, bool) {
        (0.35, false)
    }

    /// Normal and charged attack DMG +40% while shielded.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) && s.is_shielded(ci) {
            ev.stats[st::DMGP] += 0.4;
        }
    }
}
