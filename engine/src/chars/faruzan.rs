//! Faruzan, from gcsim's internal/characters/faruzan: normal attacks,
//! aimed shots (weak spots, the Hurricane Arrow and its Pressurized
//! Collapse vortex), Wind Realm of Nasamjnin, The Wind's Secret Ways (the
//! polyhedron's pulses at its corners and the party's Prayerful Wind's
//! Gift), A1, A4 and every constellation.
//!
//! The polyhedron moves along a triangle in front of where the player
//! stood at the cast; each pulse shreds the enemy when it is within 6 of
//! that corner. The buff refreshes every 239 frames while the player is
//! within 40 of where the cast was.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::faruzan as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    hurricane_count: i32,
    burst_src: i32,
    /// The polyhedron's corners and the burst's area center, at the cast.
    corners: [(f64, f64); 3],
    burst_center: (f64, f64),
}

const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST_BUFF: ModKey = ModKey::new(g::BURST_BUFF_KEY);
const BURST_SHRED: ModKey = ModKey::new(g::BURST_SHRED_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const CB_COLLAPSE: u16 = 1 << 0;
const CB_SHRED: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;
const CB_PARTICLE: u16 = 1 << 3;

/// The skill's Manifest Gale and cooldown, 12 frames in (the fight's
/// queue).
const TASK_SKILL_STATUS: u8 = 0;
/// A pulse at a corner: arg is src * 4 + the corner (the fight's queue).
const TASK_PULSE: u8 = 1;
/// The party's buff: arg is src (the fight's queue).
const TASK_BUFF: u8 = 2;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];

impl Character for Char {
    const KEY: &'static str = "faruzan";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { hurricane_count: 0, burst_src: 0, corners: [(0.0, 0.0); 3], burst_center: (0.0, 0.0) }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        if s.chars[ci].cons >= 6 {
            s.subscribe(EventKind::EnemyDamage, o);
            s.subscribe(EventKind::SpecialReactionAttack, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            Action::Aim if !matches!(p.get("hold"), None | Some(0) | Some(1)) => Some("aim[hold]".into()),
            Action::Aim => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        (fields[0] == "hurricane-count").then(|| Ok(Num::Int(if s.mod_active(ci, SKILL) { s.st::<State>(ci).hurricane_count as i64 } else { 0 })))
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

    /// Aimed shots; in Manifest Gale a shot (of either kind) uses a
    /// Hurricane Arrow, and a fully charged one becomes it, charged 60%
    /// faster from A1.
    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let skill_active = s.mod_active(ci, SKILL) && s.st::<State>(ci).hurricane_count > 0;
        let short_aim = skill_active && s.chars[ci].ascension >= 1;
        if skill_active {
            let state = s.st_mut::<State>(ci);
            state.hurricane_count -= 1;
            if state.hurricane_count <= 0 {
                s.delete_mod(ci, SKILL);
            }
        }
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::FULLAIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        ai.deployable = true;
        // its hitlag applies on weak spot hits only (HitlagOnHeadshotOnly)
        if p.get("weakspot") == Some(1) {
            ai.weak_point = true;
            ai.hitlag_halt = 0.12 * 60.0;
        }
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        let mut cbs = 0;
        let (table, queue) = if skill_active && hold == 1 {
            ai.abil = "Fully-Charged Aimed Shot (Hurricane Arrow)";
            cbs = CB_COLLAPSE;
            if short_aim { (&g::AIMED_E_FRAMES, g::AIMED_E_HITMARK) } else { (&g::AIMED_FRAMES[hold], g::AIMED_HITMARKS[hold]) }
        } else {
            (&g::AIMED_FRAMES[hold], g::AIMED_HITMARKS[hold])
        };
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        s.queue_attack(ai, cbs, queue, queue + travel);
        ActInfo::new(Frames::Table(table), table[0], queue, AnimState::Aim)
    }

    /// Wind Realm of Nasamjnin: the hit, and Manifest Gale with one
    /// Hurricane Arrow (two from C1).
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL[lvl]);
        ai.abil = "Wind Realm of Nasamjnin";
        ai.pattern = s.circle_on_player(0.0, 3.0, 3.0);
        s.queue_attack(ai, 0, g::SKILL_HITMARK, g::SKILL_HITMARK);
        s.st_mut::<State>(ci).hurricane_count = if s.chars[ci].cons >= 1 { 2 } else { 1 };
        s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_STATUS, arg: 0 }, 12);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
    }

    /// The Wind's Secret Ways: the hit (with its shred), the polyhedron's
    /// pulses at its corners, and the party's buff while it lasts (6 s
    /// longer from C2).
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST[lvl]);
        ai.abil = "The Wind's Secret Ways";
        ai.pattern = s.circle_on_player(0.0, 1.5, 6.3);
        s.queue_attack(ai, CB_SHRED, g::BURST_HITMARK, g::BURST_HITMARK);
        let duration = if s.chars[ci].cons >= 2 { 745 + 360 } else { 745 };
        let f = s.f;
        let corners = [s.point_from_player(5.19, 10.5), s.point_from_player(-5.19, 10.5), s.point_from_player(0.0, 1.5)];
        let center = s.player_pos;
        let state = s.st_mut::<State>(ci);
        state.burst_src = f;
        state.corners = corners;
        state.burst_center = center;
        let mut count = 0;
        let mut i = 137;
        while i <= duration {
            s.add_task(Task::Char { c: ci as u8, id: TASK_PULSE, arg: f << 2 | count % 3 }, 43 + i);
            count += 1;
            i += 120;
        }
        let mut i = 0;
        while i <= duration {
            s.add_task(Task::Char { c: ci as u8, id: TASK_BUFF, arg: f }, 43 + i);
            i += 239;
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_BUFF, arg: f }, 43 + duration);
        s.set_cd(ci, Action::Burst, 1200);
        s.consume_energy(ci, 3);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL_STATUS => {
                s.add_status(ci, SKILL, 1080, true);
                s.set_cd(ci, Action::Skill, 360);
            }
            TASK_PULSE => {
                let state = s.st::<State>(ci);
                if state.burst_src != arg >> 2 {
                    return;
                }
                let (x, y) = state.corners[(arg & 3) as usize];
                if s.enemy_alive && s.enemy_within(Pattern::Circle { x, y, r: 6.0 }) {
                    burst_shred(s);
                }
            }
            TASK_BUFF => {
                let state = s.st::<State>(ci);
                if state.burst_src != arg {
                    return;
                }
                let (x, y) = state.burst_center;
                if !s.player_within(Pattern::Circle { x, y, r: 40.0 }) {
                    return;
                }
                let v = g::BURST_BUFF[s.talent(ci, 2)];
                for i in 0..s.chars.len() {
                    s.add_stat_mod(i, BURST_BUFF, st::ANEMOP, v, 240, true);
                    if s.chars[ci].cons >= 6 {
                        s.add_mod(i, Mod::owned(C6, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 240);
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_COLLAPSE != 0 {
            let pos = s.enemy_pos;
            pressurized_collapse(s, ci, pos);
        }
        if cbs & CB_C4 != 0 {
            // a vortex's first enemy: 2 energy (0.5 for each more)
            s.add_energy(ci, 2.0);
        }
        if cbs & CB_SHRED != 0 {
            burst_shred(s);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 330, true);
            s.queue_particle(2.0, Element::Anemo, s.particle_delay[ci]);
        }
    }

    /// C6: CRIT DMG +40% on anemo damage under the buff.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.element == Element::Anemo {
            ev.stats[st::CD] += 0.4;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // A4: a buffed member's anemo hit adds 32% of her base ATK, once
            // in 0.8 s
            Event::EnemyHit(ev) => {
                if ev.info.element != Element::Anemo {
                    return;
                }
                if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge | AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst) {
                    return;
                }
                if s.mod_active(ev.info.actor as usize, BURST_BUFF) && !s.mod_active(ci, A4_ICD) {
                    s.hit_flat += 0.32 * s.stat(ci, st::BASEATK);
                    s.add_status(ci, A4_ICD, 48, false);
                }
            }
            // C6: the active member's damage under the buff fires a
            // Hurricane Arrow's vortex at the enemy, once in 3 s
            Event::EnemyDamage(ev) => {
                if ev.damage == 0.0 {
                    return;
                }
                let active = s.player.active as usize;
                if active != ev.info.actor as usize || !s.mod_active(active, BURST_BUFF) || s.mod_active(ci, C6_ICD) {
                    return;
                }
                s.add_status(ci, C6_ICD, 180, false);
                let pos = s.enemy_pos;
                pressurized_collapse(s, ci, pos);
            }
            Event::SpecialReactionAttack(ev) => {
                if ev.info.element == Element::Anemo && s.mod_active(ev.info.actor as usize, C6) {
                    s.special_atk.stats[st::CD] += 0.4;
                }
            }
            _ => {}
        }
    }
}

/// Perfidious Wind's Ruin: anemo RES -30% for 4 s.
fn burst_shred(s: &mut Sim) {
    s.enemy_add_resist(BURST_SHRED, Element::Anemo, -0.3, 240);
}

/// Pressurized Collapse: snapshot now, a vortex of radius 6 at `pos` 33
/// frames later (A1's shred, C4's energy, the particles).
fn pressurized_collapse(s: &mut Sim, ci: usize, pos: (f64, f64)) {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::VORTEX_DMG[lvl]);
    ai.abil = g::VORTEX_ABIL_NAME;
    ai.pattern = Pattern::Circle { x: pos.0, y: pos.1, r: 6.0 };
    let mut cbs = CB_PARTICLE;
    if s.chars[ci].cons >= 4 {
        cbs |= CB_C4;
    }
    if s.chars[ci].ascension >= 1 {
        cbs |= CB_SHRED;
    }
    let ev = s.snapshot_event(ai, 0);
    s.queue_attack_with_snap(ev, cbs, g::VORTEX_HITMARK);
}
