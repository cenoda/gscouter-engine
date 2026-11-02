//! Thundering Fury, from gcsim's internal/artifacts/thunderingfury.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("tf-2pc");
const FOUR: ModKey = ModKey::new("tf-4pc");
const ICD: ModKey = ModKey::new("tf-4pc-icd");

impl SetImpl for Set {
    const KEY: &'static str = "thunderingfury";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::ELECTROP, 0.15, false, false, -1));
        if count < 4 {
            return;
        }
        let o = Owner::Set(ci as u8, si as u8);
        s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::ReactBonus, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::Reaction, o);
    }

    /// Overloaded, electro-charged, superconduct and hyperbloom DMG +40%;
    /// aggravate's bonus +20%; Lunar-Charged, direct or not, and direct
    /// Stellar-Conduct DMG +20%.
    fn react_bonus(_: &Sim, _: usize, _: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if info.catalyzed == Some(Reaction::Aggravate) {
            return 0.2;
        }
        match info.tag {
            AttackTag::OverloadDamage | AttackTag::ECDamage | AttackTag::SuperconductDamage | AttackTag::Hyperbloom => 0.4,
            AttackTag::DirectStellarConduct | AttackTag::DirectLunarCharged | AttackTag::ReactionLunarCharge => 0.2,
            _ => 0.0,
        }
    }

    /// Overloaded, electro-charged, Lunar-Charged, superconduct,
    /// Stellar-Conduct, quicken and aggravate on an enemy, and hyperbloom
    /// (from a dendro core, which gcsim does not check), triggered by the
    /// holder while on field: skill cooldown −1 s, once every 0.8 s.
    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        // every reaction the engine emits, so that a new one must be placed
        let counts = match r {
            Reaction::Overload | Reaction::ElectroCharged | Reaction::LunarCharged | Reaction::Superconduct | Reaction::StellarConduct | Reaction::Quicken | Reaction::Aggravate => ev.on_enemy(),
            Reaction::Hyperbloom => true,
            Reaction::Vaporize
            | Reaction::Melt
            | Reaction::Frozen
            | Reaction::Shatter
            | Reaction::CrystallizeElectro
            | Reaction::CrystallizeHydro
            | Reaction::CrystallizePyro
            | Reaction::CrystallizeCryo
            | Reaction::SwirlPyro
            | Reaction::SwirlHydro
            | Reaction::SwirlElectro
            | Reaction::SwirlCryo
            | Reaction::Spread
            | Reaction::Bloom
            | Reaction::Burgeon
            | Reaction::Burning
            | Reaction::LunarBloom
            | Reaction::LunarCrystallize => false,
        };
        if !counts || ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD) {
            return;
        }
        s.add_status(ci, ICD, 48, true);
        s.reduce_cd(ci, Action::Skill, 60);
    }
}
