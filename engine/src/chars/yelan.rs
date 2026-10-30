//! Yelan, from gcsim's internal/characters/yelan: normal attacks, the
//! skill, the burst, A1, A4 and her constellations (C1's two charges, C2's
//! extra throw, C4's HP, C6's barbs), and aimed shots (Breakthrough Barb
//! while breakthrough lasts). Hits on weak spots are not supported.

use super::watch::Watcher;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::yelan as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The skill's damage, from Yelan's HP when it was cast, and how many
    /// enemies it marks.
    skill_flat: f64,
    skill_marked: i64,
    watch_src: i32,
    breakthrough: bool,
    /// The frame from which C2's throw can come again.
    c2_icd: i32,
    /// C4: the enemies marked while it lasts.
    c4_count: i32,
    /// C6: barbs shot.
    c6_count: i32,
}

const A1: ModKey = ModKey::new("yelan-a1");
const A4: ModKey = ModKey::new("yelan-a4");
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const BURST_ICD: ModKey = ModKey::new(g::BURST_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const C4_STATUS: ModKey = ModKey::new("yelanc4");
const C4: ModKey = ModKey::new("yelan-c4");
const C6_STATUS: ModKey = ModKey::new(g::C6_STATUS);

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

    fn new_state(cs: &CharSetup) -> State {
        let breakthrough = cs.params.iter().find(|p| p.0 == "breakthrough").is_none_or(|p| p.1 > 0);
        State { breakthrough, ..State::default() }
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].cons >= 1 {
            let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
            cd.available = 2;
            cd.max_extra = 1;
        }
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

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            // a burst's travel changes gcsim's package variable for every
            // later burst, of every Yelan
            Action::Burst if p.get("travel").is_some() => Some("burst[travel]".into()),
            Action::Aim if p.get("weakspot").unwrap_or(0) != 0 => Some("aim[weakspot]".into()),
            Action::Aim if !matches!(p.get("hold"), None | Some(0) | Some(1)) => Some("aim[hold]".into()),
            Action::Attack | Action::Skill | Action::Burst | Action::Aim => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        (fields[0] == "breakthrough").then(|| Ok(Num::from_bool(s.st::<State>(ci).breakthrough)))
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(TRAVEL, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        if s.chars[ci].cons >= 6 && s.core_status(g::C6_STATUS) > 0 {
            // C6: barbs instead, in the default ICD group
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
            ai.abil = "Breakthrough Barb";
            ai.flat_dmg = g::BARB[lvl] * s.max_hp(ci) * 1.56;
            for i in 0..g::ATTACK[n].len() {
                let st = s.st_mut::<State>(ci);
                st.c6_count += 1;
                if st.c6_count >= 5 {
                    s.delete_core_status(C6_STATUS);
                }
                s.queue_attack(ai, 0, g::ATTACK_HITMARKS[n][i], g::ATTACK_HITMARKS[n][i] + travel);
            }
        } else {
            for (i, mult) in g::ATTACK[n].iter().enumerate() {
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
                ai.abil = NORMAL_NAMES[n];
                s.queue_attack(ai, 0, g::ATTACK_HITMARKS[n][i], g::ATTACK_HITMARKS[n][i] + travel);
            }
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").map_or(TRAVEL, |t| t as i32);
        let lvl = s.talent(ci, 0);
        if s.st::<State>(ci).breakthrough && hold == 1 {
            s.st_mut::<State>(ci).breakthrough = false;
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_YELAN_BREAKTHROUGH, icd::GROUP_YELAN_BREAKTHROUGH, Element::Hydro, 25.0, 0.0);
            ai.abil = "Breakthrough Barb";
            ai.flat_dmg = g::BARB[lvl] * s.max_hp(ci);
            s.queue_attack(ai, 0, g::AIMED_BARB_HITMARK, g::AIMED_BARB_HITMARK + travel);
            let t = &g::AIMED_BARB_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], g::AIMED_BARB_HITMARK, AnimState::Aim);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::FULLAIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        let h = g::AIMED_HITMARKS[hold];
        s.queue_attack(ai, 0, h, h + travel);
        let t = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Aim)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let flat = g::SKILL[lvl] * s.max_hp(ci);
        let c4_on = s.mod_active(ci, C4_STATUS);
        let st = s.st_mut::<State>(ci);
        st.skill_flat = flat;
        st.skill_marked = p.get(g::SKILL_TARGET_COUNT_TAG).unwrap_or(1);
        if !c4_on {
            st.c4_count = 0;
        }
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
        if s.chars[ci].cons >= 6 {
            s.add_core_status(C6_STATUS, 20 * 60);
            s.st_mut::<State>(ci).c6_count = 0;
        }
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

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 {
            particle_cb(s, ci);
        }
        if cbs & CB_SKILL != 0 {
            if s.rng.float64() < 0.34 {
                s.st_mut::<State>(ci).breakthrough = true;
            }
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

/// The two tasks gcsim queues at the skill's hitmark: mark the enemy (C4
/// counts it), then explode the marks and grant C4's HP. The damage uses
/// Yelan's HP at the cast.
fn skill_hit(s: &mut Sim, ci: usize) {
    let marked = s.st::<State>(ci).skill_marked > 0;
    if marked {
        s.st_mut::<State>(ci).c4_count += 1;
        if s.chars[ci].cons >= 4 {
            s.add_status(ci, C4_STATUS, 25 * 60, true);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
        ai.abil = "Lingering Lifeline";
        ai.flat_dmg = s.st::<State>(ci).skill_flat;
        s.queue_attack(ai, CB_PARTICLE | CB_SKILL, 1, 1);
    }
    let count = s.st::<State>(ci).c4_count;
    if s.chars[ci].cons >= 4 && count > 0 {
        let v = (count as f64 * 0.1).min(0.4);
        for i in 0..s.chars.len() {
            s.add_stat_mod(i, C4, st::HPP, v, 25 * 60, true);
        }
    }
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
    if s.chars[ci].cons >= 2 && s.st::<State>(ci).c2_icd <= s.f {
        ai.abil = "Exquisite Throw (C2)";
        ai.icd_tag = icd::TAG_NONE;
        ai.icd_group = icd::GROUP_DEFAULT;
        ai.flat_dmg = 14.0 / 100.0 * hp;
        s.st_mut::<State>(ci).c2_icd = s.f + 108;
        s.queue_attack(ai, 0, 0, g::C2_HITMARK);
    }
}
