//! Ballad of the Fjords, from gcsim's
//! internal/weapons/spear/balladofthefjords.

use crate::content::WeaponImpl;
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const KEY: ModKey = ModKey::new("balladofthefjords");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "balladofthefjords";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(_: &mut Sim, _: usize) {}

    /// With three elements or more in the party, EM +120 to +240.
    fn post_init(s: &mut Sim, ci: usize) {
        let mut els: Vec<Element> = s.chars.iter().map(|c| c.element).collect();
        els.sort_by_key(|e| *e as u8);
        els.dedup();
        if els.len() < 3 {
            return;
        }
        let em = 90.0 + 30.0 * s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(KEY, st::EM, em, false, false, -1));
    }
}
