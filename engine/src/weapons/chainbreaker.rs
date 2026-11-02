//! Chain Breaker, from gcsim's internal/weapons/bow/chainbreaker: ATK
//! +3.6% + 1.2% per refinement for each member from Natlan or of another
//! element than the wielder's (the wielder counts when from Natlan), and EM
//! 18 + 6 per refinement with three such members or more.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const ATK: ModKey = ModKey::new("chain-breaker-atk");
const EM: ModKey = ModKey::new("chain-breaker-em");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "chainbreaker";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(_: &mut Sim, _: usize) {}

    /// gcsim's Weapon.Init: the members counted once, as the fight starts.
    fn post_init(s: &mut Sim, ci: usize) {
        let ele = s.chars[ci].element;
        let stacks = s
            .chars
            .iter()
            .filter(|c| c.element != ele || crate::data::CHARS.iter().any(|d| d.key == crate::gcsl::char_name(c.key) && d.zone == crate::data::ZONE_NATLAN))
            .count();
        let r = s.chars[ci].refine as f64;
        let atk_stack = 0.036 + r * 0.012;
        s.add_mod(ci, Mod::stat(ATK, st::ATKP, atk_stack * stacks as f64, false, false, -1), -1);
        if stacks >= 3 {
            s.add_mod(ci, Mod::stat(EM, st::EM, 18.0 + r * 6.0, false, false, -1), -1);
        }
    }
}
