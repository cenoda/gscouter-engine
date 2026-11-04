//! What gcsim's Travelers share (internal/characters/traveler/common):
//! TravelerStoryBuffs, the stat mod from the story quests, which every
//! element's NewTraveler adds, each buff on unless its param is given and
//! not 1.

use crate::gcsimdata::chars::traveler_common as g;
use crate::sim::*;
use crate::types::*;

/// gcsim's "traveler-story-quest-buffs".
pub const STORY_KEY: ModKey = ModKey::new("traveler-story-quest-buffs");

/// The params TravelerStoryBuffs reads.
pub const PARAMS: [&str; 3] = ["base_atk_buff", "skirk_story_buff", g::TRUE_MOON_PARAM];

/// The story buffs' amounts, from the character's params.
pub struct Story {
    pub amounts: Stats,
}

impl Story {
    pub fn new(cs: &CharSetup) -> Story {
        let on = |k: &str| cs.params.iter().find(|p| p.0 == k).is_none_or(|p| p.1 == 1);
        let mut m = [0.0; N_STATS];
        // Chapter III: Act I (Sumeru)
        if on("base_atk_buff") {
            m[st::BASEATK] += 3.0;
        }
        // Crystallina Chapter: Act I (Skirk)
        if on("skirk_story_buff") {
            m[st::BASEATK] += 7.0;
            m[st::EM] += 15.0;
            m[st::BASEHP] += 50.0;
        }
        // Song of the Welkin Moon: Act VIII (Nod-Krai)
        if on(g::TRUE_MOON_PARAM) {
            m[st::CR] += 0.1;
            m[st::DEFP] += 0.2;
            m[st::ER] += 0.2;
            m[st::EM] += 60.0;
            m[st::HPP] += 0.2;
            m[st::ATKP] += 0.2;
            m[st::CD] += 0.2;
        }
        Story { amounts: m }
    }
}

/// Adds the story buffs' stat mod, owned by the Traveler in slot ci.
pub fn story_buffs(s: &mut Sim, ci: usize) {
    s.add_mod(ci, Mod::owned(STORY_KEY, ModKind::Stat, Owner::Char(ci as u8), st::NONE, false, false, -1), -1);
}

/// The story buffs' amount, when `m` is their mod.
#[inline]
pub fn story_amount(story: &Story, m: &Mod, out: &mut Stats) -> bool {
    if m.key != STORY_KEY {
        return false;
    }
    for (o, a) in out.iter_mut().zip(story.amounts.iter()) {
        *o += a;
    }
    true
}
