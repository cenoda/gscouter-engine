//! Fleuve Cendre Ferryman, from gcsim's
//! internal/weapons/sword/fleuvecendreferryman.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const CR: ModKey = ModKey::new("fleuvecendreferryman-cr");
const ER: ModKey = ModKey::new("fleuvecendreferryman-er");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "fleuvecendreferryman";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(CR, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::Skill, o);
    }

    /// Skill CRIT Rate +8 to 16%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            ev.stats[st::CR] += 0.06 + 0.02 * s.chars[ci].refine as f64;
        }
    }

    /// A skill on field: ER +16 to 32% for 5 s (hitlag).
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Skill = *e {
            if s.player.active as usize == ci {
                let er = 0.12 + 0.04 * s.chars[ci].refine as f64;
                s.add_mod(ci, Mod::stat(ER, st::ER, er, false, true, -1), 5 * 60);
            }
        }
    }
}
