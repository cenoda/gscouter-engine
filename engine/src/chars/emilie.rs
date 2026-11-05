//! Emilie, from gcsim's internal/characters/emilie: normal and charged
//! attacks, the plunges from Xianyun's airborne state, Fragrance
//! Extraction (the Lumidouce Case, its levels and Scents, the Spiritbreath
//! Thorn), Aromatic Explication (the burst's level 3 case), A1, A4 and
//! every constellation.
//!
//! The case has no gadget in gcsim, only a position: its attacks land on
//! the enemy, and its Scents come from burning within 20 of it. The
//! burst's case aims at an enemy not marked in the last 0.7 s (0.4 s from
//! C4) within 12, else at a random point around it (gcsim's draws).

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gamedata::chars::emilie as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    case_travel: i32,
    /// gcsim's lumidouceSrc and lumidoucePos.
    src: i32,
    pos: (f64, f64),
    prev_level: i32,
    mark_dur: i32,
    c6_scents: i32,
    /// gcsim's tags "lumidouce-level" and "lumidouce-scent".
    level: i32,
    scent: i32,
}

const LUMIDOUCE: ModKey = ModKey::new(g::LUMIDOUCE_STATUS);
const SCENT_CD: ModKey = ModKey::new(g::LUMIDOUCE_SCENT_CD_KEY);
const SCENT_RESET: ModKey = ModKey::new(g::LUMIDOUCE_SCENT_RESET_KEY);
const ARKHE_CD: ModKey = ModKey::new(g::LUMIDOUCE_ARKHE_CD);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST_MARK: ModKey = ModKey::new(g::BURST_MARK_KEY);
const A4: ModKey = ModKey::new(g::A4_MOD_KEY);
const C1: ModKey = ModKey::new(g::C1_MOD_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_SCENT_ICD_KEY);
const C2: ModKey = ModKey::new(g::C2_MOD_KEY);
const C6: ModKey = ModKey::new(g::C6_MOD_KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_C2: u16 = 1 << 1;
const CB_C6_SCENT: u16 = 1 << 2;

/// The case appears after the skill.
const TASK_SPAWN: u8 = 0;
/// The case's attack, Scent from burning, and Scent collection, with src.
const TASK_CASE_ATTACK: u8 = 1;
const TASK_ON_BURNING: u8 = 2;
const TASK_COLLECT: u8 = 3;
/// The burst's case, its attacks (with src), and the skill's case back.
const TASK_BURST_SPAWN: u8 = 4;
const TASK_BURST_ATTACK: u8 = 5;
const TASK_RESPAWN: u8 = 6;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];
const CASE_NAMES: [&str; 2] = ["Lumidouce Case (Level 1)", "Lumidouce Case (Level 2)"];

const PLUNGE: plunge::Spec = plunge::Spec {
    low_queue: Action::Burst,
    high_queue: Action::Burst,
    ..plunge::Spec::physical(
        g::LOW_PLUNGE_HITMARK,
        g::HIGH_PLUNGE_HITMARK,
        g::COLLISION_HITMARK,
        &g::LOW_PLUNGE_FRAMES,
        &g::HIGH_PLUNGE_FRAMES,
        g::LOW_PLUNGE_POISE_DMG,
        g::HIGH_PLUNGE_POISE_DMG,
        g::LOW_PLUNGE_RADIUS,
        g::HIGH_PLUNGE_RADIUS,
        g::LOW_PLUNGE,
        g::HIGH_PLUNGE,
        g::COLLISION,
    )
};

impl Character for Char {
    const KEY: &'static str = "emilie";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { case_travel: 0, src: 0, pos: (0.0, 0.0), prev_level: 0, mark_dur: 0, c6_scents: 0, level: 0, scent: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // a4: DMG against burning enemies
        if asc >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        if cons >= 1 {
            // c1A1: her skill's and A1's DMG +20%
            if asc >= 1 {
                s.chars[ci].mods.push(Mod::owned(C1, ModKind::Attack, o, st::NONE, false, false, -1));
            }
            // a Scent when burning starts, or when dendro hits a burning
            // enemy, once in 2.9 s
            s.subscribe(EventKind::Reaction, o);
            s.subscribe(EventKind::EnemyDamage, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            Action::Skill | Action::Burst => p.0.iter().find(|(k, _)| k != "travel").map(|(k, _)| format!("{name} param {k}")),
            Action::HighPlunge | Action::LowPlunge => plunge::unknown_param(a, p, &[]),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        if let Some(why) = plunge::grounded(s, a) {
            return Some(why);
        }
        crate::content::template_invalid(s, a)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let st = s.st::<State>(ci);
        match (fields[0].as_str(), fields.get(1).map(String::as_str)) {
            ("tags", Some(g::LUMIDOUCE_LEVEL)) => Some(Ok(Num::Int(st.level as i64))),
            ("tags", Some(g::LUMIDOUCE_SCENT)) => Some(Ok(Num::Int(st.scent as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let h = g::ATTACK_HITMARKS[n];
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        apply_c6(s, ci, &mut ai);
        let (o, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
        ai.pattern = if n == 1 { s.box_on_enemy(o[0], o[1], hb[0], hb[1]) } else { s.circle_on_enemy(o[0], o[1], hb[0]) };
        let cbs = c6_scent_cb(s, ci);
        s.queue_attack(ai, cbs, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge";
        ai.hitlag_halt = 0.09 * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        apply_c6(s, ci, &mut ai);
        ai.pattern = s.box_on_enemy(0.0, -1.2, 3.3, 3.5);
        let cbs = c6_scent_cb(s, ci);
        let h = g::CHARGE_HITMARK;
        s.queue_attack(ai, cbs, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Charge)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    /// Fragrance Extraction: the summon, the case 16 frames later, and the
    /// Spiritbreath Thorn once in 10 s.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        s.st_mut::<State>(ci).case_travel = p.get("travel").map_or(g::LUMIDOUCE_ATTACK_TRAVEL, |t| t as i32);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::SKILL_DMG[lvl]);
        ai.abil = "Lumidouce Case (Summon)";
        ai.pattern = s.circle_on_player(0.0, 2.6, 4.5);
        let h = g::LUMIDOUCE_SUMMON_HITMARK;
        s.queue_attack(ai, CB_C2, h, h);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SPAWN, arg: 0 }, g::LUMIDOUCE_SPAWN);
        // arkheAttack
        if !s.mod_active(ci, ARKHE_CD) {
            s.add_status(ci, ARKHE_CD, g::SKILL_ARKE_CD * 60, true);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 0.0, g::SKILL_ARKE[lvl]);
            ai.abil = "Spiritbreath Thorn";
            ai.pattern = s.circle_on_enemy(0.0, 2.6, 4.5);
            let h = g::LUMIDOUCE_ARKHE_HITMARK;
            s.queue_attack(ai, CB_C2, h, h);
        }
        s.set_cd(ci, Action::Skill, g::SKILL_CD * 60);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    /// Aromatic Explication: the level 3 case after 96 frames (2 s longer
    /// from C4), then the skill's case back at its level.
    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let cons = s.chars[ci].cons;
        let st = s.st_mut::<State>(ci);
        st.case_travel = p.get("travel").map_or(g::LUMIDOUCE_ATTACK_TRAVEL, |t| t as i32);
        let mut extend = 0;
        st.mark_dur = g::BURST_MARK_DURATION as i32;
        if cons >= 4 {
            extend += 2 * 60;
            st.mark_dur -= 18;
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_SPAWN, arg: (g::BURST_DURATION * 60.0) as i32 + extend }, g::BURST_SPAWN);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_RESPAWN, arg: 0 }, g::BURST_RESET_LUMIDOUCE + extend);
        s.consume_energy(ci, 9);
        s.set_cd(ci, Action::Burst, (g::BURST_CD * 60.0) as i32);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Walk as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SPAWN => {
                let pos = s.point_from_player(0.0, 2.6);
                spawn_case(s, ci, 1, pos, false);
                c6(s, ci);
            }
            TASK_CASE_ATTACK => case_attack(s, ci, arg),
            TASK_ON_BURNING => case_on_burning(s, ci, arg),
            TASK_COLLECT => scent_collect(s, ci, arg),
            TASK_BURST_SPAWN => {
                let prev = if s.mod_active(ci, LUMIDOUCE) { s.st::<State>(ci).level } else { 1 };
                s.st_mut::<State>(ci).prev_level = prev;
                // spawnBurstLumidouceCase
                let f = s.f;
                let pos = s.point_from_player(0.0, 2.1);
                let st = s.st_mut::<State>(ci);
                st.src = f;
                st.pos = pos;
                st.level = 3;
                s.add_status(ci, LUMIDOUCE, arg, true);
                s.add_status(ci, SCENT_RESET, g::LUMIDOUCE_SCENT_RESET_INTERVAL, true);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_ATTACK, arg: f }, g::BURST_TICK_INTERVAL as i32);
                c6(s, ci);
            }
            TASK_BURST_ATTACK => {
                if s.st::<State>(ci).src != arg || !s.mod_active(ci, LUMIDOUCE) {
                    return;
                }
                let (cx, cy) = s.st::<State>(ci).pos;
                let r = g::BURST_RADIUS as f64;
                // getRandomEnemyPosition: an enemy not marked, within 12
                let pos = if s.enemy_alive && !s.enemy_status_active(BURST_MARK) && s.enemy_within(Pattern::Circle { x: cx, y: cy, r }) {
                    let _ = s.rng.intn(1);
                    let dur = s.st::<State>(ci).mark_dur;
                    s.enemy_add_status(BURST_MARK, dur, true);
                    s.enemy_pos
                } else {
                    s.random_point(cx, cy, 0.5, r)
                };
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::BURST_DMG[lvl]);
                ai.abil = "Lumidouce Case (Level 3)";
                ai.pattern = Pattern::Circle { x: pos.0, y: pos.1, r: 2.5 };
                s.queue_attack(ai, CB_PARTICLE | CB_C2, g::BURST_HITMARK, g::BURST_HITMARK);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_ATTACK, arg }, g::BURST_TICK_INTERVAL as i32);
            }
            TASK_RESPAWN => {
                let st = s.st::<State>(ci);
                let (level, pos) = (st.prev_level, st.pos);
                spawn_case(s, ci, level, pos, true);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD as i32, true);
            s.queue_particle(1.0, Element::Dendro, s.particle_delay[ci]);
        }
        // C2: dendro RES -30% for 10 s on a hit that does damage
        if cbs & CB_C2 != 0 && s.chars[ci].cons >= 2 && ev.damage != 0.0 {
            s.enemy_add_resist(C2, Element::Dendro, -0.3, g::C2_DURATION);
        }
        // C6's Scents: one per attack, four in all
        if cbs & CB_C6_SCENT != 0 {
            generate_scent(s, ci);
            let st = s.st_mut::<State>(ci);
            st.c6_scents += 1;
            if st.c6_scents == 4 {
                s.delete_mod(ci, C6);
            }
        }
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A4 {
            if s.enemy_burning() {
                ev.stats[st::DMGP] += (s.total_atk(ci) / 1000.0 * 0.15).min(0.36);
            }
        } else if m.key == C1 && (ev.info.tag == AttackTag::ElementalArt || ev.info.abil == g::A1_ABIL) {
            ev.stats[st::DMGP] += 0.2;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Reaction(Reaction::Burning, ev) if ev.on_enemy() => c1_scent(s, ci),
            Event::EnemyDamage(ev) => {
                if s.enemy_burning() && ev.info.element == Element::Dendro {
                    c1_scent(s, ci);
                }
            }
            _ => {}
        }
    }
}

/// applyC6Bonus: while C6's status lasts, normal and charged attacks deal
/// dendro damage with 300% of her ATK added.
fn apply_c6(s: &Sim, ci: usize, ai: &mut AttackInfo) {
    if s.chars[ci].cons < 6 || !s.mod_active(ci, C6) || !matches!(ai.tag, AttackTag::Normal | AttackTag::Extra) {
        return;
    }
    ai.flat_dmg += s.total_atk(ci) * 3.0;
    ai.element = Element::Dendro;
    ai.ignore_infusion = true;
}

fn c6_scent_cb(s: &Sim, ci: usize) -> u16 {
    if s.chars[ci].cons >= 6 && s.mod_active(ci, C6) { CB_C6_SCENT } else { 0 }
}

fn c6(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 6 || s.mod_active(ci, C6_ICD) {
        return;
    }
    s.st_mut::<State>(ci).c6_scents = 0;
    s.add_status(ci, C6, g::C6_DURATION, true);
    s.add_status(ci, C6_ICD, g::C6_ICD, true);
}

fn c1_scent(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, C1_ICD) {
        return;
    }
    s.add_status(ci, C1_ICD, g::C1_SCENT_ICD as i32, true);
    generate_scent(s, ci);
}

fn generate_scent(s: &mut Sim, ci: usize) {
    s.st_mut::<State>(ci).scent += 1;
}

/// spawnLumidouceCase: in the burst's case only its Scent reset timer is
/// refreshed.
fn spawn_case(s: &mut Sim, ci: usize, level: i32, pos: (f64, f64), after_burst: bool) {
    if s.mod_active(ci, LUMIDOUCE) && s.st::<State>(ci).level == 3 {
        s.add_status(ci, SCENT_RESET, g::LUMIDOUCE_SCENT_RESET_INTERVAL, true);
        return;
    }
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    st.src = f;
    st.pos = pos;
    st.level = level;
    s.add_status(ci, LUMIDOUCE, g::SKILL_DURATION * 60, true);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CASE_ATTACK, arg: f }, g::LUMIDOUCE_TICK_INTERVAL as i32);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ON_BURNING, arg: f }, g::LUMIDOUCE_SCENT_INTERVAL as i32);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_COLLECT, arg: f }, g::LUMIDOUCE_SCENT_INTERVAL as i32);
    if !after_burst {
        s.st_mut::<State>(ci).scent = 0;
        s.add_status(ci, SCENT_RESET, g::LUMIDOUCE_SCENT_RESET_INTERVAL, true);
    }
}

/// The case's attack every 1.5 s (twice at level 2), then a Scent
/// collection now, which starts another collection chain as gcsim's does.
fn case_attack(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).src != src || !s.mod_active(ci, LUMIDOUCE) {
        return;
    }
    let st = s.st::<State>(ci);
    let (level, travel) = (st.level, st.case_travel);
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_EMILIE_LUMIDOUCE, icd::GROUP_EMILIE_LUMIDOUCE, Element::Dendro, 25.0, g::SKILL_LUMIDOUCE[(level - 1) as usize][lvl]);
    ai.abil = CASE_NAMES[(level - 1) as usize];
    // gcsim's NewCircleHit from the case, centered on the enemy
    ai.pattern = s.circle_on_target(1.0);
    let h = g::LUMIDOUCE_HITMARK + travel;
    s.queue_attack(ai, CB_PARTICLE, h, h);
    if level == 2 {
        let h = g::LUMIDOUCE_HITMARK_LEVEL2 + travel;
        s.queue_attack(ai, CB_PARTICLE, h, h);
    }
    scent_collect(s, ci, src);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CASE_ATTACK, arg: src }, g::LUMIDOUCE_TICK_INTERVAL as i32);
}

/// A Scent every 0.5 s while an enemy within 20 of the case burns, once
/// in 2 s.
fn case_on_burning(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).src != src || !s.mod_active(ci, LUMIDOUCE) {
        return;
    }
    let interval = g::LUMIDOUCE_SCENT_INTERVAL as i32;
    if s.mod_active(ci, SCENT_CD) {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ON_BURNING, arg: src }, interval);
        return;
    }
    let (x, y) = s.st::<State>(ci).pos;
    if s.enemy_alive && s.enemy_within(Pattern::Circle { x, y, r: 20.0 }) && s.enemy_burning() {
        generate_scent(s, ci);
        s.add_status(ci, SCENT_CD, g::LUMIDOUCE_SCENT_CD, true);
        s.add_status(ci, SCENT_RESET, g::LUMIDOUCE_SCENT_RESET_INTERVAL, true);
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ON_BURNING, arg: src }, interval);
}

/// Every 0.5 s: without a Scent for 8 s the case falls back to level 1;
/// two Scents raise it to level 2, or at level 2 are spent on A1.
fn scent_collect(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).src != src || !s.mod_active(ci, LUMIDOUCE) {
        return;
    }
    let reset = !s.mod_active(ci, SCENT_RESET);
    let st = s.st_mut::<State>(ci);
    if reset && st.level > 1 {
        st.level = 1;
        st.scent = 0;
    }
    if st.scent >= 2 {
        if st.level < 2 {
            st.level += 1;
            st.scent -= 2;
        } else {
            a1(s, ci);
        }
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_COLLECT, arg: src }, g::LUMIDOUCE_SCENT_INTERVAL as i32);
}

/// A1: Cleardew Cologne, two Scents spent on 600% of ATK.
fn a1(s: &mut Sim, ci: usize) {
    if s.chars[ci].ascension < 1 {
        return;
    }
    s.st_mut::<State>(ci).scent -= 2;
    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, 6.0);
    ai.abil = g::A1_ABIL;
    apply_c6(s, ci, &mut ai);
    // gcsim's NewCircleHit from the case, centered on the enemy
    ai.pattern = s.circle_on_target(3.0);
    s.queue_attack(ai, CB_C2, g::A1_HITMARK, g::A1_HITMARK);
}
