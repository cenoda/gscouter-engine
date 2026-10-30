//! Wolf-Fang, from gcsim's internal/weapons/sword/wolffang.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    /// The CRIT Rate stacks of the skill's buff and of the burst's.
    stacks: [i32; 2],
}

const DMG: ModKey = ModKey::new("wolf-fang");
const BUFF: [ModKey; 2] = [ModKey::new("wolf-fang-skill"), ModKey::new("wolf-fang-burst")];
const ICD: [ModKey; 2] = [ModKey::new("wolf-fang-skill-icd"), ModKey::new("wolf-fang-burst-icd")];

/// 0 for the skill's buff, 1 for the burst's.
fn which(tag: AttackTag) -> Option<usize> {
    match tag {
        AttackTag::ElementalArt | AttackTag::ElementalArtHold => Some(0),
        AttackTag::ElementalBurst => Some(1),
        _ => None,
    }
}

impl WeaponImpl for Weapon {
    const KEY: &'static str = "wolffang";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].mods.push(Mod::owned(DMG, ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, false, -1));
        // gcsim subscribes the skill's then the burst's; one hook does both
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        let Some(w) = which(ev.info.tag) else { return };
        let r = s.chars[ci].refine as f64;
        if m.key == DMG {
            ev.stats[st::DMGP] += 0.12 + 0.04 * r;
        } else if m.key == BUFF[w] {
            ev.stats[st::CR] += (0.015 + 0.005 * r) * s.weapon_st::<State>(ci).stacks[w] as f64;
        }
    }

    /// A skill or burst hit by the holder, on field: a CRIT Rate stack for
    /// 10 s, up to 4, once every 0.1 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        let Some(w) = which(ev.info.tag) else { return };
        if ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, ICD[w]) {
            return;
        }
        s.add_status(ci, ICD[w], 6, true);
        let active = s.mod_active(ci, BUFF[w]);
        let st = s.weapon_st_mut::<State>(ci);
        if !active {
            st.stacks[w] = 0;
        }
        st.stacks[w] = (st.stacks[w] + 1).min(4);
        s.add_mod(ci, Mod::owned(BUFF[w], ModKind::Attack, Owner::Weapon(ci as u8), st::NONE, false, true, -1), 600);
    }
}
