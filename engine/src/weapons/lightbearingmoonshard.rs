//! Lightbearing Moonshard, from gcsim's internal/weapons/sword/lightbearingmoonshard.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const DEF: ModKey = ModKey::new("lightbearing-moonshard-def");
const BUFF: ModKey = ModKey::new("lightbearing-moonshard");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "lightbearingmoonshard";
    type State = State;
    fn new_state() -> State {
        State
    }

    /// DEF +20 to 40%.
    fn init(s: &mut Sim, ci: usize) {
        let def = 0.15 + 0.05 * s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(DEF, st::DEFP, def, false, false, -1));
        s.subscribe(EventKind::Skill, Owner::Weapon(ci as u8));
    }

    /// Lunar-Crystallize DMG (direct or not) +64 to 128%.
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        match info.tag {
            AttackTag::DirectLunarCrystallize | AttackTag::ReactionLunarCrystallize => 0.48 + 0.16 * s.chars[ci].refine as f64,
            _ => 0.0,
        }
    }

    /// A skill on field: the bonus for 5 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Skill = *e {
            if s.player.active as usize == ci {
                s.add_mod(ci, Mod::owned(BUFF, ModKind::ReactBonus, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 5 * 60);
            }
        }
    }
}
