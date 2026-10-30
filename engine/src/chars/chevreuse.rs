//! Chevreuse, from gcsim's internal/characters/chevreuse: normal attacks,
//! the skill pressed and held (the overcharged ball after an overload),
//! its heals and Arkhe hit, the burst and its mines, A1, A4 and every
//! constellation. No charged attacks.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::chevreuse as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// A1: every member is pyro or electro, with at least one of each.
    only_pyro_electro: bool,
    overcharged_ball: bool,
    c4_shots_left: i32,
    c6_heal_queued: bool,
    c6_stacks: [usize; 4],
}

const SKILL_HEAL: ModKey = ModKey::new(g::SKILL_HEAL_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const ARKHE_ICD: ModKey = ModKey::new(g::ARKHE_ICD_KEY);
const A1_PYRO: ModKey = ModKey::new("chev-a1-pyro");
const A1_ELECTRO: ModKey = ModKey::new("chev-a1-electro");
const A4: ModKey = ModKey::new("chev-a4");
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C2_ICD: ModKey = ModKey::new(g::C2_ICD_KEY);
const C4_STATUS: ModKey = ModKey::new(g::C4_STATUS_KEY);
const C6: [ModKey; 3] = [ModKey::new("chev-c6-1-stack"), ModKey::new("chev-c6-2-stack"), ModKey::new("chev-c6-3-stack")];

const CB_PARTICLE: u16 = 1 << 0;
const CB_C2: u16 = 1 << 1;
const CB_ARKHE_PRESS: u16 = 1 << 2;
const CB_ARKHE_HOLD: u16 = 1 << 3;

const TASK_SKILL_HEAL: u8 = 0;
const TASK_HEALING: u8 = 1;
const TASK_C6_TEAM_HEAL: u8 = 2;
const TASK_A4: u8 = 3;

const FRAMES_HOLD: u8 = 0;

impl Character for Char {
    const KEY: &'static str = "chevreuse";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        // the overcharged ball
        s.subscribe(EventKind::Reaction, Owner::Char(ci as u8));
        s.utility_dash = Some(ci as u8);
        if s.chars[ci].ascension >= 1 {
            let n = s.chars.len();
            let pyro = s.chars.iter().filter(|c| c.element == Element::Pyro).count();
            let electro = s.chars.iter().filter(|c| c.element == Element::Electro).count();
            let only = pyro > 0 && electro > 0 && pyro + electro == n;
            s.st_mut::<State>(ci).only_pyro_electro = only;
            if only {
                s.subscribe(EventKind::EnemyDamage, Owner::Char(ci as u8));
            }
        }
        // C1 reads the overload events the subscription above already gets
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        (fields[0] == "overcharged-ball").then(|| Ok(Num::from_bool(s.st::<State>(ci).overcharged_ball)))
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let h = g::ATTACK_HITMARKS[n][i];
            s.queue_attack(ai, 0, h, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(0);
        let lvl = s.talent(ci, 1);
        if hold == 0 {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_PRESS[lvl]);
            ai.abil = "Short-Range Rapid Interdiction Fire";
            s.queue_attack(ai, CB_PARTICLE | CB_ARKHE_PRESS, g::SKILL_PRESS_HITMARK, g::SKILL_PRESS_HITMARK);
            skill_heal(s, ci, g::SKILL_PRESS_CD_START);
            s.set_cd_with_delay(ci, Action::Skill, 15 * 60, g::SKILL_PRESS_CD_START);
            let t = &g::SKILL_PRESS_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill);
        }
        // hold=1 is the earliest hitmark, hold=301 the latest
        let hold = (hold.clamp(1, 301) - 1) as i32;
        let hitmark = hold + g::SKILL_HOLD_HITMARK;
        let cd_start = hold + g::SKILL_HOLD_CD_START;
        let ai = if s.st::<State>(ci).overcharged_ball {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_OVERCHARGED[lvl]);
            ai.abil = "Short-Range Rapid Interdiction Fire [Overcharged]";
            ai.blunt = true;
            ai.poise = 125.0;
            s.st_mut::<State>(ci).overcharged_ball = false;
            if s.chars[ci].ascension >= 4 {
                s.add_task(Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, cd_start);
            }
            ai
        } else {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_HOLD[lvl]);
            ai.abil = "Short-Range Rapid Interdiction Fire [Hold]";
            ai
        };
        if s.mod_active(ci, C4_STATUS) {
            let st = s.st_mut::<State>(ci);
            st.c4_shots_left -= 1;
            if st.c4_shots_left == 0 {
                s.delete_mod(ci, C4_STATUS);
            }
        } else {
            s.set_cd_with_delay(ci, Action::Skill, 15 * 60, cd_start);
        }
        let c2 = if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
        s.queue_attack(ai, CB_PARTICLE | c2 | CB_ARKHE_HOLD, hitmark, hitmark);
        skill_heal(s, ci, cd_start);
        let t = &g::SKILL_HOLD_FRAMES;
        ActInfo::new(Frames::Char { id: FRAMES_HOLD, arg: hold as i16 }, hold + t[0], hold + t[Action::Dash as usize], AnimState::Skill)
    }

    fn frames(_: u8, hold: i16, next: Action) -> i32 {
        hold as i32 + g::SKILL_HOLD_FRAMES[next as usize]
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 50.0, g::BURST[lvl]);
        ai.abil = "Explosive Grenade";
        ai.blunt = true;
        ai.poise = 100.0;
        s.queue_attack(ai, 0, g::SNAPSHOT_DELAY, g::BURST_HITMARK);
        let mut mine = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_CHEVREUSE_BURST_MINES, Element::Pyro, 25.0, g::BURST_SECONDARY[lvl]);
        mine.abil = "Secondary Explosive Shell";
        mine.blunt = true;
        mine.poise = 25.0;
        // eight mines in five groups
        for (count, delay) in [(1, 24), (2, 33), (2, 42), (2, 51), (1, 60)] {
            for _ in 0..count {
                s.queue_attack(mine, 0, g::SNAPSHOT_DELAY, g::BURST_HITMARK + delay);
            }
        }
        if s.chars[ci].cons >= 4 {
            s.add_status(ci, C4_STATUS, 6 * 60, true);
            s.st_mut::<State>(ci).c4_shots_left = 2;
        }
        s.consume_energy(ci, 4);
        s.set_cd(ci, Action::Burst, 15 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_SKILL_HEAL => {
                // a retrigger while it lasts (C4) only refreshes it
                if s.mod_active(ci, SKILL_HEAL) {
                    s.add_status(ci, SKILL_HEAL, 12 * 60 + 1, false);
                    return;
                }
                s.add_status(ci, SKILL_HEAL, 12 * 60 + 1, false);
                s.add_task(Task::Char { c: ci as u8, id: TASK_HEALING, arg: 0 }, g::SKILL_HEAL_INTERVAL);
                if s.st::<State>(ci).c6_heal_queued {
                    return;
                }
                s.st_mut::<State>(ci).c6_heal_queued = true;
                s.add_task(Task::Char { c: ci as u8, id: TASK_C6_TEAM_HEAL, arg: 0 }, 12 * 60);
            }
            TASK_HEALING => {
                if !s.mod_active(ci, SKILL_HEAL) {
                    return;
                }
                let lvl = s.talent(ci, 1);
                let src = g::SKILL_HP_REGEN[lvl] * s.max_hp(ci) + g::SKILL_HP_FLAT[lvl];
                let bonus = s.stat(ci, st::HEAL);
                let active = s.player.active as usize;
                s.heal(ci, active, src, bonus);
                c6(s, ci, s.player.active as usize);
                s.add_task(Task::Char { c: ci as u8, id: TASK_HEALING, arg: 0 }, g::SKILL_HEAL_INTERVAL);
            }
            TASK_C6_TEAM_HEAL => {
                if s.chars[ci].cons < 6 {
                    return;
                }
                s.st_mut::<State>(ci).c6_heal_queued = false;
                for i in 0..s.chars.len() {
                    c6(s, ci, i);
                }
                let src = 0.1 * s.max_hp(ci);
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, None, false, src, bonus);
            }
            TASK_A4 => {
                let v = (s.max_hp(ci) / 1000.0 * 0.01).min(0.4);
                for i in 0..s.chars.len() {
                    if matches!(s.chars[i].element, Element::Pyro | Element::Electro) {
                        s.add_stat_mod(i, A4, st::ATKP, v, 30 * 60, true);
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    /// C6's stacks: pyro and electro DMG.
    fn mod_amount(_: &Sim, _: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::PYROP] += 0.2;
        out[st::ELECTROP] += 0.2;
        true
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 10 * 60, false);
            s.queue_particle(4.0, Element::Pyro, s.particle_delay[ci]);
        }
        if cbs & CB_C2 != 0 && !s.mod_active(ci, C2_ICD) {
            s.add_status(ci, C2_ICD, 10 * 60, true);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Pyro, 25.0, 1.2);
            ai.abil = "Sniper Induced Explosion (C2)";
            ai.blunt = true;
            ai.poise = 25.0;
            // each bomb 0.6 s to 1 s after the hit
            let d1 = (60.0 * (0.6 + s.rng.float64() * (1.0 - 0.6))) as i32;
            let d2 = (60.0 * (0.6 + s.rng.float64() * (1.0 - 0.6))) as i32;
            s.queue_attack(ai, 0, d1, d1);
            s.queue_attack(ai, 0, d2, d2);
        }
        if cbs & (CB_ARKHE_PRESS | CB_ARKHE_HOLD) != 0 && !s.mod_active(ci, ARKHE_ICD) {
            s.add_status(ci, ARKHE_ICD, 10 * 60, true);
            let lvl = s.talent(ci, 1);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 0.0, g::ARKHE[lvl]);
            ai.abil = "Surging Blade (chevreuse)";
            let delay = if cbs & CB_ARKHE_PRESS != 0 { g::SKILL_PRESS_ARKHE_HITMARK - g::SKILL_PRESS_HITMARK } else { g::SKILL_HOLD_ARKHE_HITMARK - g::SKILL_HOLD_HITMARK };
            s.queue_attack(ai, 0, delay, delay);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Reaction(Reaction::Overload, ev) => {
                s.st_mut::<State>(ci).overcharged_ball = true;
                // C1, which gcsim subscribes after A1
                let st = s.st::<State>(ci);
                if s.chars[ci].cons < 1 || !st.only_pyro_electro {
                    return;
                }
                let actor = ev.info.actor as usize;
                if actor == ci || actor != s.player.active as usize || s.mod_active(ci, C1_ICD) {
                    return;
                }
                s.add_status(ci, C1_ICD, 10 * 60, true);
                s.add_energy(actor, 6.0);
            }
            Event::EnemyDamage(ev) => {
                if ev.info.tag != AttackTag::OverloadDamage {
                    return;
                }
                s.enemy_add_resist(A1_PYRO, Element::Pyro, -0.4, 6 * 60);
                s.enemy_add_resist(A1_ELECTRO, Element::Electro, -0.4, 6 * 60);
            }
            _ => {}
        }
    }
}

/// skillHeal: the heal over time from the cooldown's start.
fn skill_heal(s: &mut Sim, ci: usize, delay: i32) {
    s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_HEAL, arg: 0 }, delay);
}

/// C6: a stack of pyro and electro DMG on a character, in three slots.
fn c6(s: &mut Sim, ci: usize, on: usize) {
    if s.chars[ci].cons < 6 {
        return;
    }
    let slot = s.st::<State>(ci).c6_stacks[on];
    s.add_mod(on, Mod::owned(C6[slot], ModKind::Stat, Owner::Char(ci as u8), st::NONE, false, true, -1), 8 * 60);
    s.st_mut::<State>(ci).c6_stacks[on] = (slot + 1) % 3;
}
