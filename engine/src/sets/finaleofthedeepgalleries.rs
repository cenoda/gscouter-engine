//! Finale of the Deep Galleries, from gcsim's
//! internal/artifacts/finaleofthedeepgalleries: two pieces, 15% cryo DMG;
//! four pieces, with the holder at 0 energy, 60% DMG to normal attacks and
//! bursts, the one lost for 6 s after the other deals damage.

use crate::content::{Event, SetImpl};
use crate::sim::*;
use crate::types::*;

pub struct Set;
pub struct State;

const TWO: ModKey = ModKey::new("deep-galleries-2pc");
const FOUR: ModKey = ModKey::new("deep-galleries-4pc");
const NORMAL_DEBUFF: ModKey = ModKey::new("deep-galleries-4pc-normal-debuff");
const BURST_DEBUFF: ModKey = ModKey::new("deep-galleries-4pc-burst-debuff");

impl SetImpl for Set {
    const KEY: &'static str = "finaleofthedeepgalleries";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize, si: usize, count: i32) {
        s.chars[ci].mods.push(Mod::stat(TWO, st::CRYOP, 0.15, false, false, -1));
        if count < 4 {
            return;
        }
        let o = Owner::Set(ci as u8, si as u8);
        s.chars[ci].mods.push(Mod::owned(FOUR, ModKind::Attack, o, st::NONE, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, o);
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if s.chars[ci].energy != 0.0 {
            return;
        }
        let ok = match ev.info.tag {
            AttackTag::Normal => !s.mod_active(ci, NORMAL_DEBUFF),
            AttackTag::ElementalBurst => !s.mod_active(ci, BURST_DEBUFF),
            _ => false,
        };
        if ok {
            ev.stats[st::DMGP] += 0.6;
        }
    }

    fn event(s: &mut Sim, ci: usize, _: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci {
            return;
        }
        match ev.info.tag {
            AttackTag::Normal => s.add_status(ci, BURST_DEBUFF, 360, true),
            AttackTag::ElementalBurst => s.add_status(ci, NORMAL_DEBUFF, 360, true),
            _ => {}
        }
    }
}
