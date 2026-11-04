//! Cloudforged, from gcsim's internal/weapons/bow/cloudforged.
//!
//! gcsim stacks its EM (30 + 10r per stack, 2 stacks, 18 s, hitlag
//! affected) on OnEnergyChange when the holder's energy goes down. That
//! event is AddEnergy's, not ConsumeEnergy's (a burst's drain emits
//! OnEnergyBurst), so only a negative AddEnergy triggers it: for a bow
//! user, Sethos's A1 on his charged shot and Shimenawa's Reminiscence's
//! four pieces on the skill. Without them the passive never triggers in
//! gcsim, and does nothing here. The engine emits no OnEnergyChange, so a
//! holder that could lose energy that way is an error.

use crate::content::WeaponImpl;
use crate::sim::*;

pub struct Weapon;
pub struct State;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "cloudforged";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let shimenawa4 = s.chars[ci].sets.iter().any(|x| x.kind == SetKind::Shimenawa && x.count >= 4);
        if s.chars[ci].kind == CharKind::Sethos || shimenawa4 {
            s.unsupported("weapon cloudforged's EM stacks on an energy loss (gcsim's OnEnergyChange, which the engine does not emit), on Sethos or with Shimenawa's Reminiscence 4pc");
        }
    }
}
