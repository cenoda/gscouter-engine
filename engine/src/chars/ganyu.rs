//! Ganyu, from gcsim's internal/characters/ganyu: normal attacks, aimed
//! shots (the Frostflake Arrow and its Bloom, weak spots, C6's instant
//! charge), Trail of the Qilin (the Ice Lotus's two hits), Celestial
//! Shower (its 50 ice shards, A4's Cryo DMG Bonus, C4's stacks), A1 and
//! every constellation.
//!
//! Each shard falls on the enemy when it is in the burst's area and not
//! struck in the last 1.45 s (RandomEnemyWithinArea draws one number for
//! the one enemy), and on a random point of the area otherwise.

use std::cell::Cell;

use crate::content::Character;
use crate::gamedata::chars::ganyu as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// A1: the CRIT Rate of Frostflake Arrows fired before this frame.
    a1_expiry: i32,
    /// C1: the arrows whose 2 energy is paid, by the parity of the arrow
    /// (its arrow and bloom pay once between them).
    c1_paid: [bool; 2],
    c1_parity: usize,
    /// C4: the enemy's stacks (gcsim's tag on the enemy), dropped when an
    /// attack finds the debuff gone.
    c4_stacks: Cell<i32>,
    /// The last two bursts (their shards overlap the next cast): the
    /// area's center at the cast and the snapshot, by the cast's parity.
    bursts: [((f64, f64), AttackEvent); 2],
    burst_parity: usize,
}

const BURST_MARK: ModKey = ModKey::new(g::BURST_MARK_KEY);
const BURST_STATUS: ModKey = ModKey::new("ganyuburst");
const C1: ModKey = ModKey::new(g::C1_KEY);
const C4: ModKey = ModKey::new(g::C4_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);
const A4: ModKey = ModKey::new("ganyu-field");

const CB_PARTICLE: u16 = 1 << 0;
/// C1 on an arrow or bloom of each parity.
const CB_C1_0: u16 = 1 << 1;
const CB_C1_1: u16 = 1 << 2;

/// The Frostflake Arrow's snapshot and queue: arg is travel * 2 + weakspot.
const TASK_FROSTFLAKE: u8 = 0;
/// A shard of the burst: arg is its slot * 1024 + the tick's offset i.
const TASK_SHARD: u8 = 1;

const FR_AIM: u8 = 0;

const NORMAL_NAMES: [&str; 6] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4", "Normal 5"];

impl Character for Char {
    const KEY: &'static str = "ganyu";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        let ev = AttackEvent::new(AttackInfo::new(0, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 0.0), 0);
        State { a1_expiry: -1, c1_paid: [false; 2], c1_parity: 0, c4_stacks: Cell::new(0), bursts: [((0.0, 0.0), ev); 2], burst_parity: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].cons >= 2 {
            let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
            cd.available = 2;
            cd.max_extra = 1;
        }
        // C4: every member's damage on the debuffed enemy
        if s.chars[ci].cons >= 4 {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(C4, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1));
            }
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            Action::Aim => p.0.iter().find(|(k, _)| !matches!(k.as_str(), "hold" | "travel" | "weakspot")).map(|(k, _)| format!("aim param {k}")),
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// gcsim's error for an aim with another hold.
    fn invalid(s: &Sim, _: usize, a: Action, p: &Params) -> Option<String> {
        if a == Action::Aim {
            if let Some(h) = p.get("hold") {
                if !(0..=2).contains(&h) {
                    return Some(format!("invalid hold param supplied, got {h}"));
                }
            }
        }
        crate::content::template_invalid(s, a)
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let h = g::ATTACK_HITMARKS[n];
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        s.queue_attack(ai, 0, h, h + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    /// Aimed shots: physical (hold 0), charged (hold 1), or the
    /// Frostflake Arrow and its Bloom (hold 2, the default), snapshot at
    /// the hitmark; C6's status skips 83 frames of the next one.
    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(2) as usize;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let weakspot = p.get("weakspot") == Some(1);
        let mut skip = 0;
        if s.core_status(g::C6_KEY) > 0 && hold == 2 {
            s.delete_core_status(C6);
            skip = 83;
        }
        if hold == 2 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_FROSTFLAKE, arg: travel * 2 + i32::from(weakspot) }, g::AIMED_HITMARKS[2] - skip);
        } else {
            let lvl = s.talent(ci, 0);
            let mut ai = arrow_info(ci, weakspot);
            ai.mult = g::FULLAIM[lvl];
            if hold < 1 {
                ai.abil = "Aimed Shot";
                ai.element = Element::Physical;
                ai.mult = g::AIM[lvl];
            }
            ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
            let h = g::AIMED_HITMARKS[hold];
            s.queue_attack(ai, 0, h, h + travel);
        }
        let t = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Char { id: FR_AIM, arg: (hold as i16) << 8 | skip as i16 }, t[0] - skip, g::AIMED_HITMARKS[hold] - skip, AnimState::Aim)
    }

    /// Trail of the Qilin: the Ice Lotus hits now and when it bursts 6 s
    /// later, on one snapshot, each hit with its 2 particles.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::LOTUS[lvl]);
        ai.abil = "Ice Lotus";
        ai.pattern = s.circle_on_player(0.0, 0.0, 4.0);
        let ev = s.snapshot_event(ai, 0);
        s.queue_attack_with_snap(ev, CB_PARTICLE, 13);
        s.queue_attack_with_snap(ev, CB_PARTICLE, 373);
        if s.chars[ci].cons == 6 {
            s.add_core_status(C6, 1800);
        }
        s.set_cd_with_delay(ci, Action::Skill, 600, 10);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    /// Celestial Shower: a shard every 18 frames for 15 s from 122 frames
    /// in, on one snapshot.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SHOWER[lvl]);
        ai.abil = "Celestial Shower";
        let ev = s.snapshot_event(ai, 0);
        s.add_core_status(BURST_STATUS, 15 * 60 + g::BURST_START);
        let center = s.player_pos;
        let state = s.st_mut::<State>(ci);
        state.burst_parity ^= 1;
        let slot = state.burst_parity as i32;
        state.bursts[slot as usize] = (center, ev);
        let mut i = 0;
        while i < 15 * 60 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_SHARD, arg: (slot << 10) | i }, i + g::BURST_START);
            i += 18;
        }
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 3);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_AIM => g::AIMED_FRAMES[(arg >> 8) as usize][next as usize] - (arg & 0xff) as i32,
            _ => unreachable!(),
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_FROSTFLAKE => frostflake(s, ci, arg >> 1, arg & 1 == 1),
            TASK_SHARD => shard(s, ci, (arg >> 10) as usize, arg & 1023),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 {
            s.queue_particle(2.0, Element::Cryo, s.particle_delay[ci]);
        }
        for (bit, parity) in [(CB_C1_0, 0), (CB_C1_1, 1)] {
            if cbs & bit == 0 {
                continue;
            }
            // C1: cryo RES -15% for 5 s on each hit, 2 energy once an arrow
            s.enemy_add_resist(C1, Element::Cryo, -0.15, 300);
            let state = s.st_mut::<State>(ci);
            if !state.c1_paid[parity] {
                state.c1_paid[parity] = true;
                s.add_energy(ci, 2.0);
            }
        }
    }

    /// C4: 5% DMG per stack on the debuffed enemy; the stacks go when the
    /// debuff has.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if !s.enemy_status_active(C4) {
            s.st::<State>(ci).c4_stacks.set(0);
            return;
        }
        ev.stats[st::DMGP] += s.st::<State>(ci).c4_stacks.get() as f64 * 0.05;
    }
}

/// The aimed shots' info: a pierce shot, deployable, its hitlag on weak
/// spot hits only (HitlagOnHeadshotOnly).
fn arrow_info(ci: usize, weakspot: bool) -> AttackInfo {
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 0.0);
    ai.abil = "Fully-Charged Aimed Shot";
    ai.deployable = true;
    if weakspot {
        ai.weak_point = true;
        ai.hitlag_halt = 0.12 * 60.0;
    }
    ai
}

/// The Frostflake Arrow and its Bloom 18 frames after it, on one snapshot
/// (with A1's CRIT Rate when an arrow was fired in the last 5 s).
fn frostflake(s: &mut Sim, ci: usize, travel: i32, weakspot: bool) {
    let lvl = s.talent(ci, 0);
    let mut ai = arrow_info(ci, weakspot);
    ai.abil = "Frostflake Arrow";
    ai.mult = g::FFA[lvl];
    let mut ev = s.snapshot_event(ai, 0);
    if s.chars[ci].ascension >= 1 && s.f < s.st::<State>(ci).a1_expiry {
        ev.stats[st::CR] += 0.20;
    }
    let mut cbs = 0;
    if s.chars[ci].cons >= 1 {
        let state = s.st_mut::<State>(ci);
        state.c1_parity ^= 1;
        let p = state.c1_parity;
        state.c1_paid[p] = false;
        cbs = if p == 0 { CB_C1_0 } else { CB_C1_1 };
    }
    ev.info.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
    s.queue_attack_with_snap(ev, cbs, travel);
    ev.info.abil = "Frostflake Arrow Bloom";
    ev.info.mult = g::FFB[lvl];
    // not on a weak spot, so without the arrow's hitlag
    ev.info.weak_point = false;
    ev.info.hitlag_halt = 0.0;
    ev.info.pattern = s.circle_on_target(5.0);
    s.queue_attack_with_snap(ev, cbs, travel + 18);
    // the first arrow and bloom do not benefit from A1
    s.st_mut::<State>(ci).a1_expiry = s.f + 60 * 5;
}

/// A shard of Celestial Shower at offset i of the burst in `slot`: on
/// the enemy when it is in the area and unmarked, else on a random point;
/// A4's Cryo DMG Bonus to the active member in the area; C4's debuff.
fn shard(s: &mut Sim, ci: usize, slot: usize, i: i32) {
    let ((cx, cy), mut ev) = s.st::<State>(ci).bursts[slot];
    let area = Pattern::Circle { x: cx, y: cy, r: 10.0 };
    let enemy_in = s.enemy_alive && s.enemy_within(area);
    let pos = if enemy_in && !s.enemy_status_active(BURST_MARK) {
        let _ = s.rng.intn(1);
        s.enemy_add_status(BURST_MARK, 87, true);
        s.enemy_pos
    } else {
        s.random_point(cx, cy, 0.5, 9.5)
    };
    ev.info.pattern = Pattern::Circle { x: pos.0, y: pos.1, r: 2.5 };
    s.queue_attack_with_snap(ev, 0, 8);
    if s.chars[ci].ascension >= 4 && s.player_within(area) {
        let active = s.player.active as usize;
        s.add_stat_mod(active, A4, st::CRYOP, 0.2, 60, true);
    }
    if s.chars[ci].cons >= 4 && enemy_in {
        s.enemy_add_status(C4, g::C4_DUR, true);
        if i % 180 == 0 {
            let stacks = &s.st::<State>(ci).c4_stacks;
            stacks.set((stacks.get() + 1).min(5));
        }
    }
}
