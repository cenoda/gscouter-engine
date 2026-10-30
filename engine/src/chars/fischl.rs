//! Fischl, from gcsim's internal/characters/fischl: normal attacks, aimed
//! shots (without weak spots), Oz from the skill (and its recast) and from
//! the burst, A4, C1, C2, C4 and C6. Her Hexerei bonus needs a second
//! Hexerei character, which `profile` rejects.

use super::watch::Watcher;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::fischl as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// Oz's attack and snapshot.
    oz: AttackEvent,
    /// The frame this Oz spawned (gcsim's ozSource), and the frame its
    /// ticks started, which a recast resets.
    oz_source: i32,
    oz_tick_src: i32,
    /// For gcsl's .fischl.oz.
    oz_active: bool,
    oz_travel: i32,
    /// The frame of the burst whose Oz has not spawned yet, -1 once it has.
    burst_oz_spawn_src: i32,
    watch_src: i32,
}

const OZ_ACTIVE: ModKey = ModKey::new(g::OZ_ACTIVE_KEY);
const RECAST_CD: ModKey = ModKey::new(g::SKILL_RECAST_CD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;

const TASK_RECAST_CD: u8 = 0;
// arg: the first tick's delay * 2, plus 1 for the burst's Oz
const TASK_OZ_SPAWN: u8 = 1;
const TASK_OZ_REMOVE: u8 = 2;
const TASK_OZ_TICK: u8 = 3;
// arg: the burst's frame
const TASK_BURST_OZ: u8 = 4;
const TASK_C4_HEAL: u8 = 5;
const TASK_RECAST: u8 = 6;
const TASK_TRIGGER: u8 = 7;
const TASK_TICK: u8 = 8;

const ON_REMOVED_BURST: u8 = 0;

/// C6: an extra hit on the active character's normal attacks while Oz is
/// out.
const WATCH: Watcher = Watcher { status: OZ_ACTIVE, icd: None, tick_on_active: true, delay_key: 1, trigger_task: TASK_TRIGGER, tick_task: TASK_TICK, fire: c6_wave };

impl Character for Char {
    const KEY: &'static str = "fischl";
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        let oz_travel = cs.params.iter().find(|p| p.0 == "oz_travel").map_or(10, |p| p.1 as i32);
        State {
            oz: AttackEvent::new(AttackInfo::new(0, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_FISCHL, Element::Electro, 25.0, 0.0), 0),
            oz_source: -1,
            oz_tick_src: -1,
            oz_active: false,
            oz_travel,
            burst_oz_spawn_src: 0,
            watch_src: 0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::Reaction, Owner::Char(ci as u8));
        }
        if s.chars[ci].cons >= 6 {
            s.subscribe(EventKind::StateChange, Owner::Char(ci as u8));
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            Action::Aim if p.get("weakspot").unwrap_or(0) != 0 => Some("aim[weakspot]".into()),
            Action::Aim if !matches!(p.get("hold"), None | Some(0) | Some(1)) => Some("aim[hold]".into()),
            Action::Aim => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn action_ready(s: &Sim, ci: usize, a: Action, p: &Params) -> Option<Option<Failure>> {
        if a != Action::Skill || !s.st::<State>(ci).oz_active {
            return None;
        }
        // the skill recasts Oz, and cannot be used otherwise while Oz is out
        if p.get("recast").unwrap_or(0) != 0 {
            return Some(s.mod_active(ci, RECAST_CD).then_some(Failure::SkillCd));
        }
        Some(Some(Failure::NoFailure))
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let st = s.st::<State>(ci);
        match fields[0].as_str() {
            "oz" => Some(Ok(Num::from_bool(st.oz_active))),
            "oz-source" => Some(Ok(Num::Int(st.oz_source as i64))),
            "oz-duration" => Some(Ok(Num::Int(s.mod_duration(ci, g::OZ_ACTIVE_KEY) as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let hitmark = g::ATTACK_HITMARKS[n];
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::AUTO[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        s.queue_attack(ai, 0, hitmark, hitmark + travel);
        if s.chars[ci].cons >= 1 && !s.mod_active(ci, OZ_ACTIVE) {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 0.22);
            ai.abil = "Gaze of the Deep (C1)";
            s.queue_attack(ai, 0, hitmark, hitmark + travel);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hitmark, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        // without a weak spot hit, its hitlag (on headshots only) never applies
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::FULLAIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        ai.deployable = true;
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        let hitmark = g::AIMED_HITMARKS[hold];
        s.queue_attack(ai, 0, hitmark, hitmark + travel);
        let table = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Table(table), table[0], hitmark, AnimState::Aim)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if p.get("recast").unwrap_or(0) != 0 && s.st::<State>(ci).oz_active && !s.mod_active(ci, RECAST_CD) {
            return skill_recast(s, ci);
        }
        // the summon always applies electro
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_FISCHL, Element::Electro, 25.0, g::BIRD_SUM[lvl]);
        ai.abil = "Oz (Summon)";
        if s.chars[ci].cons >= 2 {
            ai.mult += 2.0;
        }
        s.queue_attack(ai, 0, g::SKILL_OZ_SPAWN, g::SKILL_OZ_HITMARK);
        s.set_cd(ci, Action::Skill, 25 * 60 + g::SKILL_OZ_SPAWN);
        s.add_task(Task::Char { c: ci as u8, id: TASK_RECAST_CD, arg: 0 }, g::SKILL_OZ_SPAWN);
        s.st_mut::<State>(ci).oz_active = true;
        queue_oz(s, ci, false, g::SKILL_OZ_SPAWN, g::SKILL_OZ_FIRST_TICK);
        ActInfo::new(Frames::Table(&g::SKILL_FRAMES), g::SKILL_FRAMES[0], g::SKILL_FRAMES[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_FISCHL, Element::Electro, 25.0, g::BURST[lvl]);
        ai.abil = "Midnight Phantasmagoria";
        ai.blunt = true;
        ai.poise = 150.0;
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        if s.chars[ci].cons >= 4 {
            // C4's hit lands before the burst's
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_FISCHL, Element::Electro, 50.0, 2.22);
            ai.abil = "Her Pilgrimage of Bleak (C4)";
            s.queue_attack(ai, 0, 8, 8);
        }
        s.consume_energy(ci, 6);
        s.set_cd(ci, Action::Burst, 15 * 60);
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        st.oz_active = true;
        st.burst_oz_spawn_src = f;
        // Oz spawns at the end of the animation, or after a swap that cuts it
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_OZ, arg: f }, g::BURST_FULL_OZ_SPAWN);
        let mut info = ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_FRAMES[Action::Swap as usize], AnimState::Burst);
        info.on_removed = OnRemoved::Char(ON_REMOVED_BURST);
        info
    }

    fn on_removed(s: &mut Sim, ci: usize, _: u8, next: AnimState) {
        if next == AnimState::Swap {
            // the swap's own delay comes first
            let src = s.st::<State>(ci).burst_oz_spawn_src;
            let d = s.swap_delay;
            burst_oz_spawn(s, ci, src, g::BURST_SHORT_OZ_SPAWN + d, g::BURST_SHORT_OZ_FIRST_TICK + d);
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_RECAST_CD => s.add_status(ci, RECAST_CD, g::SKILL_RECAST_CD, false),
            TASK_OZ_SPAWN => oz_spawn(s, ci, arg & 1 == 1, arg >> 1),
            TASK_OZ_REMOVE => {
                let st = s.st_mut::<State>(ci);
                if st.oz_source == arg {
                    st.oz_active = false;
                }
            }
            TASK_OZ_TICK => oz_tick(s, ci, arg),
            TASK_BURST_OZ => burst_oz_spawn(s, ci, arg, 0, g::BURST_FULL_OZ_FIRST_TICK),
            TASK_C4_HEAL => {
                let hp = s.max_hp(ci);
                let bonus = s.stat(ci, st::HEAL);
                s.heal(ci, ci, 0.2 * hp, bonus);
            }
            TASK_RECAST => {
                let f = s.f;
                s.st_mut::<State>(ci).oz_tick_src = f;
                s.add_task(Task::Char { c: ci as u8, id: TASK_OZ_TICK, arg: f }, 60);
                let oz = s.st::<State>(ci).oz;
                s.st_mut::<State>(ci).oz = s.snapshot_event(oz.info, oz.callbacks);
            }
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

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE == 0 || s.mod_active(ci, PARTICLE_ICD) {
            return;
        }
        s.add_status(ci, PARTICLE_ICD, 6, true);
        if s.rng.float64() < 0.67 {
            s.queue_particle(1.0, Element::Electro, s.particle_delay[ci]);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Reaction(r, ev) => a4(s, ci, r, ev),
            Event::StateChange(next) => {
                if let Some(src) = WATCH.on_state(s, ci, next) {
                    s.st_mut::<State>(ci).watch_src = src;
                }
            }
            _ => {}
        }
    }
}

fn skill_recast(s: &mut Sim, ci: usize) -> ActInfo {
    s.add_status(ci, RECAST_CD, g::SKILL_RECAST_CD, false);
    s.add_task(Task::Char { c: ci as u8, id: TASK_RECAST, arg: 0 }, 2);
    ActInfo::new(Frames::Table(&g::SKILL_RECAST_FRAMES), g::SKILL_RECAST_FRAMES[0], g::SKILL_RECAST_FRAMES[Action::Dash as usize], AnimState::Skill)
}

fn queue_oz(s: &mut Sim, ci: usize, burst: bool, spawn: i32, first_tick: i32) {
    if spawn > 0 {
        s.add_task(Task::Char { c: ci as u8, id: TASK_OZ_SPAWN, arg: first_tick * 2 + burst as i32 }, spawn);
    } else {
        oz_spawn(s, ci, burst, first_tick);
    }
}

fn oz_spawn(s: &mut Sim, ci: usize, burst: bool, first_tick: i32) {
    let dur = if s.chars[ci].cons == 6 { 600 + 120 } else { 600 };
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    st.oz_active = true;
    st.oz_source = f;
    st.oz_tick_src = f;
    s.add_status(ci, OZ_ACTIVE, dur, false);
    s.add_task(Task::Char { c: ci as u8, id: TASK_OZ_REMOVE, arg: f }, dur);
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_FISCHL, Element::Electro, 25.0, g::BIRD_ATK[lvl]);
    ai.abil = if burst { "Oz (Burst)" } else { "Oz (Skill)" };
    s.st_mut::<State>(ci).oz = s.snapshot_event(ai, CB_PARTICLE);
    s.add_task(Task::Char { c: ci as u8, id: TASK_OZ_TICK, arg: f }, first_tick);
}

fn oz_tick(s: &mut Sim, ci: usize, src: i32) {
    if src != s.st::<State>(ci).oz_tick_src || !s.mod_active(ci, OZ_ACTIVE) {
        return;
    }
    let st = s.st::<State>(ci);
    let (oz, travel) = (st.oz, st.oz_travel);
    s.queue_attack_with_snap(oz, CB_PARTICLE, travel);
    s.add_task(Task::Char { c: ci as u8, id: TASK_OZ_TICK, arg: src }, g::OZ_TICK_INTERVAL);
}

fn burst_oz_spawn(s: &mut Sim, ci: usize, src: i32, spawn: i32, first_tick: i32) {
    let st = s.st_mut::<State>(ci);
    if src != st.burst_oz_spawn_src {
        return;
    }
    st.burst_oz_spawn_src = -1;
    queue_oz(s, ci, true, spawn, first_tick);
    // the heal comes after Oz snapshots
    if s.chars[ci].cons >= 4 {
        s.add_task(Task::Char { c: ci as u8, id: TASK_C4_HEAL, arg: 0 }, spawn + 1);
    }
}

/// An attack that takes Oz's snapshot.
fn with_oz_snapshot(s: &Sim, ci: usize, ai: AttackInfo) -> AttackEvent {
    let oz = &s.st::<State>(ci).oz;
    let mut ev = AttackEvent::new(ai, 0);
    ev.stats = oz.stats;
    ev.char_lvl = oz.char_lvl;
    ev.snap_f = oz.snap_f;
    ev
}

/// A4: an electro reaction by the active character while Oz is out calls
/// down Thundering Retribution, with Oz's snapshot.
fn a4(s: &mut Sim, ci: usize, r: Reaction, ev: &AttackEvent) {
    if !matches!(r, Reaction::Overload | Reaction::ElectroCharged | Reaction::Superconduct | Reaction::SwirlElectro | Reaction::CrystallizeElectro) {
        return;
    }
    let active = s.player.active as usize;
    if ev.info.actor as usize != active || !s.mod_active(ci, OZ_ACTIVE) || s.mod_active(active, A4_ICD) {
        return;
    }
    s.add_status(active, A4_ICD, 30, true);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_FISCHL, Element::Electro, 25.0, 0.8);
    ai.abil = "Thundering Retribution (A4)";
    let atk = with_oz_snapshot(s, ci, ai);
    s.queue_attack_with_snap(atk, 0, 4);
}

fn c6_wave(s: &mut Sim, ci: usize) {
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_FISCHL, Element::Electro, 25.0, 0.3);
    ai.abil = "Evernight Raven (C6)";
    let atk = with_oz_snapshot(s, ci, ai);
    let travel = s.st::<State>(ci).oz_travel;
    s.queue_attack_with_snap(atk, 0, travel);
}
