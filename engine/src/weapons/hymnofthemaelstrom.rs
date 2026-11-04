//! Hymn of the Maelstrom, from gcsim's internal/weapons/catalyst/hymnofthemaelstrom.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State;

const HEAL_PERM: ModKey = ModKey::new("hymn-of-the-maelstrom-heal");
const HP_BUFF: ModKey = ModKey::new("hymn-of-the-maelstrom-hp");
const ATK_BUFF: ModKey = ModKey::new("hymn-of-the-maelstrom-atk");
const STRONG_KEY: ModKey = ModKey::new("hymn-of-the-maelstrom-strong");

const STACK_0: ModKey = ModKey::new("hymn-of-the-maelstrom-stack-0");
const STACK_1: ModKey = ModKey::new("hymn-of-the-maelstrom-stack-1");
const STACK_2: ModKey = ModKey::new("hymn-of-the-maelstrom-stack-2");

impl WeaponImpl for Weapon {
    const KEY: &'static str = "hymnofthemaelstrom";
    type State = State;
    fn new_state() -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(HEAL_PERM, st::HEAL, 0.03 + 0.01 * r, false, false, -1));
        s.chars[ci].mods.push(Mod::owned(HP_BUFF, ModKind::Stat, o, st::NONE, false, false, -1));
        for j in 0..s.chars.len() {
            s.chars[j].mods.push(Mod::owned(ATK_BUFF, ModKind::Stat, o, st::NONE, false, false, -1));
        }
        s.subscribe(EventKind::Heal, o);
        s.subscribe(EventKind::Reaction, o);
    }

    fn mod_amount(s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool {
        let m_factor = Self::mul(s, ci);
        if m_factor == 0.0 {
            return false;
        }
        let r = s.chars[ci].refine as f64;
        if m.key == HP_BUFF {
            if on == ci {
                out[st::HPP] += (0.03 + 0.01 * r) * m_factor;
                return true;
            }
        } else if m.key == ATK_BUFF {
            if s.player.active as usize == on {
                let max_hp = s.max_hp(ci);
                let atk_per_kilo = 0.003 + 0.001 * r;
                let atk_max = 0.06 + 0.02 * r;
                let per = ((max_hp - 40000.0) * atk_per_kilo / 1000.0).clamp(0.0, atk_max);
                if per > 0.0 {
                    out[st::ATKP] += per * m_factor;
                    return true;
                }
            }
        }
        false
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Heal { caller, .. } => {
                if caller != ci {
                    return;
                }
                let stacks = [STACK_0, STACK_1, STACK_2];
                let free = stacks.iter().position(|&k| !s.mod_active(ci, k));
                if let Some(idx) = free {
                    s.add_status(ci, stacks[idx], 10 * 60, true);
                } else {
                    for &k in &stacks {
                        s.add_status(ci, k, 10 * 60, true);
                    }
                }
            }
            Event::Reaction(Reaction::Frozen | Reaction::StellarSwirl, _) => {
                s.add_status(ci, STRONG_KEY, 5 * 60, true);
            }
            _ => {}
        }
    }
}

impl Weapon {
    fn stacks(s: &Sim, ci: usize) -> usize {
        let mut n = 0;
        if s.mod_active(ci, STACK_0) {
            n += 1;
        }
        if s.mod_active(ci, STACK_1) {
            n += 1;
        }
        if s.mod_active(ci, STACK_2) {
            n += 1;
        }
        n
    }

    fn mul(s: &Sim, ci: usize) -> f64 {
        let mut m = Self::stacks(s, ci) as f64;
        if s.mod_active(ci, STRONG_KEY) {
            m *= 1.75;
        }
        m
    }
}
