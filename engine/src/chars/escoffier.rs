//! Escoffier, from gcsim's internal/characters/escoffier, at C0: normal
//! attacks, the skill and its ticks (with its Surging Blade), the burst
//! with its heals, A1 and A4. No charged attacks or plunges.

use crate::content::Character;
use crate::gcsimdata::chars::escoffier as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    skill_src: i32,
    skill_travel: i32,
    a1_src: i32,
    /// Hydro and cryo members, for A4's shred.
    hydro_cryo: usize,
}

const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const ALIGNED_ICD: ModKey = ModKey::new(g::SKILL_ALIGNED_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new(g::A1KEY);
const A4_CRYO: ModKey = ModKey::new("escoffier-a4-shred-cryo");
const A4_HYDRO: ModKey = ModKey::new("escoffier-a4-shred-hydro");

const CB_PARTICLE: u16 = 1 << 0;
const CB_A4: u16 = 1 << 1;

// arg: the normal's index * 8 + the hit's
const TASK_NORMAL: u8 = 0;
// arg: the skill's frame
const TASK_SKILL_TICK: u8 = 1;
const TASK_BLADE: u8 = 2;
const TASK_BURST_HIT: u8 = 3;
const TASK_BURST_HEAL: u8 = 4;
// arg: the burst's frame
const TASK_A1: u8 = 5;

fn ceil(x: f64) -> i32 {
    x.ceil() as i32
}

impl Character for Char {
    const KEY: &'static str = "escoffier";
    // C1, C2, C4 and C6 are not ported
    const MAX_CONS: i32 = 0;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let n = s.chars.iter().filter(|c| matches!(c.element, Element::Hydro | Element::Cryo)).count();
        s.st_mut::<State>(ci).hydro_cryo = n;
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let hitmarks = g::ATTACK_HITMARKS[n];
        for (i, &h) in hitmarks.iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: (n * 8 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hitmarks[hitmarks.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(5, |t| t as i32);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_ESCOFFIER_SKILL, Element::Cryo, 25.0, g::SKILL_INITAL[lvl]);
        ai.abil = "Low-Temperature Cooking";
        let a4 = a4_cb(s, ci);
        s.queue_attack(ai, CB_PARTICLE | a4, g::SKILL_INIT_HITMARK, g::SKILL_INIT_HITMARK);
        // the ticks are not affected by hitlag
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        st.skill_travel = travel;
        st.skill_src = f;
        for i in 0..g::SKILL_TICKS {
            s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_TICK, arg: f }, g::SKILL_FIRST_TICK_DELAY + ceil(g::SKILL_INTERVAL * i as f64));
        }
        s.add_status(ci, SKILL, g::SKILL_FIRST_TICK_DELAY + ceil((g::SKILL_TICKS - 1) as f64 * g::SKILL_INTERVAL), false);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BLADE, arg: 0 }, g::SKILL_ALIGNED_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD, 22);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_HIT, arg: 0 }, g::HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_HEAL, arg: 0 }, g::INITIAL_HEAL);
        // A1: heals over time
        if s.chars[ci].ascension >= 1 {
            let f = s.f;
            s.st_mut::<State>(ci).a1_src = f;
            let ticks = g::A1_COUNT;
            for i in 0..ticks {
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1, arg: f }, g::A1_FIRST_TICK + ceil(g::A1_INTERVAL * i as f64));
            }
            s.add_status(ci, A1, g::A1_FIRST_TICK + ceil((ticks - 1) as f64 * g::A1_INTERVAL), true);
        }
        s.set_cd(ci, Action::Burst, g::BURST_CD * 60);
        s.consume_energy(ci, 5);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = ((arg / 8) as usize, (arg % 8) as usize);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = 0.06 * 60.0;
                ai.def_halt = true;
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_SKILL_TICK => {
                if arg != s.st::<State>(ci).skill_src {
                    return;
                }
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_ESCOFFIER_SKILL, Element::Cryo, 25.0, g::SKILL_DOT[lvl]);
                ai.abil = "Frosty Parfait";
                let travel = s.st::<State>(ci).skill_travel;
                let a4 = a4_cb(s, ci);
                s.queue_attack(ai, a4, 0, travel);
            }
            TASK_BLADE => {
                if s.mod_active(ci, ALIGNED_ICD) {
                    return;
                }
                s.add_status(ci, ALIGNED_ICD, g::SKILL_ALIGNED_ICD, true);
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 0.0, g::ARKHE[lvl]);
                ai.abil = "Surging Blade (escoffier)";
                let a4 = a4_cb(s, ci);
                s.queue_attack(ai, a4, 0, 0);
            }
            TASK_BURST_HIT => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 50.0, g::BURST[lvl]);
                ai.abil = "Scoring Cuts";
                let a4 = a4_cb(s, ci);
                s.queue_attack(ai, a4, 0, 0);
            }
            TASK_BURST_HEAL => {
                let lvl = s.talent(ci, 2);
                let heal = g::BURST_HEAL_FLAT[lvl] + g::BURST_HEAL_PER[lvl] * total_atk(&s.stats(ci));
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, None, false, heal, bonus);
            }
            TASK_A1 => {
                if arg != s.st::<State>(ci).a1_src {
                    return;
                }
                let heal = g::A1_SCALING * total_atk(&s.stats(ci));
                let bonus = s.stat(ci, st::HEAL);
                let active = s.player.active as usize;
                s.heal(ci, active, heal, bonus);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 12, false);
            s.queue_particle(4.0, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_A4 != 0 {
            let shred = g::A4_SHRED[s.st::<State>(ci).hydro_cryo];
            s.enemy_add_resist(A4_CRYO, Element::Cryo, -shred, g::A4_DUR);
            s.enemy_add_resist(A4_HYDRO, Element::Hydro, -shred, g::A4_DUR);
        }
    }
}

fn a4_cb(s: &Sim, ci: usize) -> u16 {
    if s.chars[ci].ascension >= 4 { CB_A4 } else { 0 }
}
