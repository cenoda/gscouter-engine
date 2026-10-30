//! Yelan, from gcsim's internal/characters/yelan, for C0 without
//! breakthrough shots.

use super::watch::Watcher;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::yelan as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The skill's damage, from Yelan's HP when it was cast.
    skill_flat: f64,
    watch_src: i32,
}

const A1: ModKey = ModKey::new("yelan-a1");
const A4: ModKey = ModKey::new("yelan-a4");
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const BURST_ICD: ModKey = ModKey::new(g::BURST_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);

const TRAVEL: i32 = 10;

const CB_PARTICLE: u16 = 1 << 0;
const CB_SKILL: u16 = 1 << 1;

const TASK_SKILL_HIT: u8 = 0;
const TASK_BURST_STATUS: u8 = 1;
const TASK_TRIGGER: u8 = 2;
const TASK_TICK: u8 = 3;

const WATCH: Watcher = Watcher { status: BURST, icd: Some(BURST_ICD), tick_on_active: false, delay_key: 1, trigger_task: TASK_TRIGGER, tick_task: TASK_TICK, fire };

impl Character for Char {
    const KEY: &'static str = "yelan";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].ascension >= 1 {
            let mut els: Vec<Element> = s.chars.iter().map(|c| c.element).collect();
            els.sort_by_key(|e| *e as u8);
            els.dedup();
            let count = els.len();
            let v = if count >= 4 { 0.3 } else { count as f64 * 0.06 };
            s.chars[ci].mods.push(Mod::stat(A1, st::HPP, v, false, false, -1));
        }
        s.subscribe(EventKind::StateChange, Owner::Char(ci as u8));
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(_: &Sim, _: usize, fields: &[String]) -> Option<Result<Num, String>> {
        (fields[0] == "breakthrough").then_some(Ok(Num::from_bool(true)))
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            s.queue_attack(ai, 0, g::ATTACK_HITMARKS[n][i], g::ATTACK_HITMARKS[n][i] + TRAVEL);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let flat = g::SKILL[lvl] * s.max_hp(ci);
        s.st_mut::<State>(ci).skill_flat = flat;
        s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_HIT, arg: 0 }, g::SKILL_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 600, g::SKILL_HITMARK - 2);
        ActInfo::new(Frames::Table(&g::SKILL_FRAMES), g::SKILL_FRAMES[0], g::SKILL_FRAMES[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 50.0, 0.0);
        ai.abil = "Depth-Clarion Dice";
        ai.flat_dmg = g::BURST[lvl] * s.max_hp(ci);
        ai.hitlag_halt = 0.05 * 60.0;
        ai.hitlag_factor = 0.05;
        ai.deployable = true;
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_STATUS, arg: 0 }, g::BURST_HITMARK);
        s.set_cd(ci, Action::Burst, 18 * 60);
        s.consume_energy(ci, 6);
        ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_FRAMES[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL_HIT => skill_hit(s, ci),
            TASK_BURST_STATUS => burst_status(s, ci),
            TASK_TRIGGER => {
                if let Some(src) = WATCH.trigger(s, ci) {
                    s.st_mut::<State>(ci).watch_src = src;
                }
            }
            TASK_TICK => {
                let current = s.st::<State>(ci).watch_src;
                WATCH.tick(s, ci, arg, current);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16) {
        if cbs & CB_PARTICLE != 0 {
            particle_cb(s, ci);
        }
        if cbs & CB_SKILL != 0 {
            // breakthrough: no aimed shots here, but the roll is kept
            let _ = s.rng.float64() < 0.34;
            if s.mod_active(ci, BURST) {
                throw(s, ci);
            }
        }
    }

    fn attack_mod(s: &Sim, _ci: usize, on: usize, m: &Mod, ev: &mut AttackEvent) {
        // A4: while the burst lasts, the active character's damage grows
        if s.player.active as usize == on {
            let dmg = ((s.f - m.started) / 60) as f64 * 0.035 + 0.01;
            ev.stats[st::DMGP] += dmg.min(0.5);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::StateChange(next) = *e {
            if let Some(src) = WATCH.on_state(s, ci, next) {
                s.st_mut::<State>(ci).watch_src = src;
            }
        }
    }
}

/// The two tasks gcsim queues at the skill's hitmark: mark the enemy, then
/// explode the marks. The damage uses Yelan's HP at the cast.
fn skill_hit(s: &mut Sim, ci: usize) {
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
    ai.abil = "Lingering Lifeline";
    ai.flat_dmg = s.st::<State>(ci).skill_flat;
    s.queue_attack(ai, CB_PARTICLE | CB_SKILL, 1, 1);
}

fn particle_cb(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, PARTICLE_ICD) {
        return;
    }
    s.add_status(ci, PARTICLE_ICD, 18, true);
    s.queue_particle(4.0, Element::Hydro, s.particle_delay[ci]);
}

fn burst_status(s: &mut Sim, ci: usize) {
    s.add_status(ci, BURST, 15 * 60, false);
    if s.chars[ci].ascension < 4 {
        return;
    }
    let f = s.f;
    for i in 0..s.chars.len() {
        let mut m = Mod::owned(A4, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, f + 15 * 60);
        m.started = f;
        mod_add(&mut s.chars[i].mods, m);
    }
}

fn fire(s: &mut Sim, ci: usize) {
    throw(s, ci);
    s.add_status(ci, BURST_ICD, 60, true);
}

fn throw(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 2);
    let hp = s.max_hp(ci);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_YELAN_BURST, icd::GROUP_YELAN_BURST, Element::Hydro, 25.0, 0.0);
    ai.abil = "Exquisite Throw";
    ai.flat_dmg = g::BURST_DICE[lvl] * hp;
    let ev = s.snapshot_event(ai, 0);
    for i in 0..3 {
        s.queue_attack_with_snap(ev, 0, g::BURST_TRAVEL + i * 6);
    }
}
