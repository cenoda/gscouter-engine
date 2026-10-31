//! Alhaitham, from gcsim's internal/characters/alhaitham: normal and
//! charged attacks, Universality pressed or held, the low plunge after a
//! held skill, Particular Field, the Chisel-Light Mirrors, A1, A4 and
//! every constellation.
//!
//! A high plunge, and a low plunge with `collision`, are Xianyun's
//! airborne plunges. That state is not in the engine, so those actions
//! are errors that name it. The held skill's low plunge is not that.
//!
//! The burst's flat damage uses the skill's talent level, as gcsim's
//! Burst does. Projection attacks draw one random number, for the left
//! or right hitmarks, and only at one or two mirrors.

use crate::content::{Character, Event};
use crate::gcsl::Num;
use crate::gcsimdata::chars::alhaitham as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    mirrors: i32,
    /// The frame the current infusion started. A mirror-loss task from an
    /// older frame does nothing. 0 at the start, as gcsim's zero value.
    last_infusion: i32,
    /// Which of the four C2 mods the next stack refreshes.
    c2_counter: i32,
    /// The last skill was held: the low plunge after it is his own.
    skill_hold: bool,
}

const A1_ICD: ModKey = ModKey::new(g::A1_ICD_KEY);
const PROJECTION_ICD: ModKey = ModKey::new(g::PROJECTION_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const A4: ModKey = ModKey::new("alhaitham-a4");
const C4_LOSS: ModKey = ModKey::new("alhaitham-c4-loss");
const C4_GAIN: ModKey = ModKey::new("alhaitham-c4-gain");
const C6: ModKey = ModKey::new(g::C6KEY);

const C2_KEYS: [ModKey; 4] = [
    ModKey::new("alhaitham-c2-1-stack"),
    ModKey::new("alhaitham-c2-2-stack"),
    ModKey::new("alhaitham-c2-3-stack"),
    ModKey::new("alhaitham-c2-4-stack"),
];

const CB_A1: u16 = 1 << 0;
const CB_PROJECTION: u16 = 1 << 1;
const CB_C1: u16 = 1 << 2;
const CB_PARTICLE: u16 = 1 << 3;

const TASK_SKILL_MIRROR: u8 = 0;
const TASK_MIRROR_LOSS: u8 = 1;
const TASK_BURST: u8 = 2;

const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];
const PROJ: [&str; 3] = [
    "Chisel-Light Mirror: Projection Attack 1",
    "Chisel-Light Mirror: Projection Attack 2",
    "Chisel-Light Mirror: Projection Attack 3",
];

impl Character for Char {
    const KEY: &'static str = "alhaitham";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Swap, Owner::Char(ci as u8));
        if s.chars[ci].ascension >= 4 {
            let o = Owner::Char(ci as u8);
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Attack, o, st::NONE, false, false, -1));
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let unknown = |known: &[&str]| {
            p.0.iter()
                .find(|(k, _)| !known.iter().any(|n| k == *n))
                .map(|(k, _)| format!("{name} param {k}"))
        };
        match a {
            Action::Attack | Action::Charge | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            Action::Skill => unknown(&["hold"]),
            Action::LowPlunge if p.get("collision").is_some() => Some("low_plunge[collision] needs xianyun".into()),
            Action::LowPlunge => unknown(&["short"]),
            Action::HighPlunge => Some("high_plunge needs xianyun".into()),
            _ => Some(name.into()),
        }
    }

    /// The low plunge after a held skill. Anything else needs Xianyun's
    /// airborne state, which the engine does not have.
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        let after_hold = s.player.last.char as usize == ci
            && s.player.last.action == Action::Skill
            && s.st::<State>(ci).skill_hold;
        if a == Action::LowPlunge && !after_hold {
            return Some("low_plunge can only be used while airborne or after hold skill".into());
        }
        crate::content::template_invalid(s, a)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let marks = g::ATTACK_HITMARKS[n];
        for i in 0..marks.len() {
            let mut ai = AttackInfo::new(
                ci,
                AttackTag::Normal,
                icd::TAG_NORMAL_ATTACK,
                icd::GROUP_DEFAULT,
                Element::Physical,
                25.0,
                g::ATTACK[n][i][lvl],
            );
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let off = g::ATTACK_OFFSETS[n];
            ai.pattern = if matches!(n, 1 | 2 | 3) {
                let box_ = g::ATTACK_HITBOXES[n];
                s.box_on_player(0.0, off, box_[0], box_[1])
            } else {
                s.fan_on_player(0.0, off, g::ATTACK_HITBOXES[n][0], g::ATTACK_FAN_ANGLES[n])
            };
            let h = marks[i];
            s.queue_attack(ai, CB_PROJECTION, h, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *marks.last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        for i in 0..g::CHARGE.len() {
            let mut ai = AttackInfo::new(
                ci,
                AttackTag::Extra,
                icd::TAG_EXTRA_ATTACK,
                icd::GROUP_ALHAITHAM_EXTRA_ATTACK,
                Element::Physical,
                25.0,
                g::CHARGE[i][lvl],
            );
            ai.abil = CHARGE_NAMES[i];
            ai.pattern = s.circle_on_player(0.0, 0.0, 2.5);
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, CB_A1 | CB_PROJECTION, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARKS[1], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold") == Some(1);
        s.st_mut::<State>(ci).skill_hold = hold;
        if hold { skill_hold(s, ci) } else { skill_tap(s, ci) }
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let skip = if p.get("short").unwrap_or(0) > 0 { 20 } else { 0 };
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::LOW_PLUNGE[lvl]);
        ai.abil = "Low Plunge Attack";
        ai.blunt = true;
        ai.poise = g::LOW_PLUNGE_POISE_DMG;
        ai.pattern = s.circle_on_player(0.0, 1.0, 3.0);
        let h = g::LOW_PLUNGE_HITMARK_AL - skip;
        s.queue_attack(ai, CB_A1 | CB_PROJECTION, h, h);
        let t = &g::LOW_PLUNGE_FRAMES_AL;
        ActInfo::new(
            Frames::Char { id: 0, arg: skip as i16 },
            t[0] - skip,
            t[Action::Dash as usize] - skip,
            AnimState::Plunge,
        )
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mirrors = s.st::<State>(ci).mirrors;
        let lvl_b = s.talent(ci, 2);
        let lvl_s = s.talent(ci, 1);
        let flat = g::BURST_EM[lvl_s] * s.stat(ci, st::EM);
        let pattern = s.circle_on_player(0.0, 7.1, 6.8);
        for i in 0..4 + 2 * mirrors {
            let mut ai = AttackInfo::new(
                ci,
                AttackTag::ElementalBurst,
                icd::TAG_ELEMENTAL_BURST,
                icd::GROUP_DEFAULT,
                Element::Dendro,
                25.0,
                g::BURST_ATK[lvl_b],
            );
            ai.abil = "Particular Field: Fetters of Phenomena";
            ai.flat_dmg = flat;
            ai.pattern = pattern;
            s.queue_attack(ai, 0, 67, g::BURST_HITMARK + i * 21);
        }
        s.consume_energy(ci, 6);
        s.set_cd(ci, Action::Burst, 18 * 60);
        let generated = if s.chars[ci].cons >= 6 { 3 } else { 3 - mirrors };
        let src = s.st::<State>(ci).last_infusion;
        mirror_loss(s, ci, src, mirrors);
        if s.chars[ci].cons >= 4 {
            c4_loss(s, ci, mirrors);
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST, arg: generated }, 184);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn frames(_: u8, skip: i16, next: Action) -> i32 {
        g::LOW_PLUNGE_FRAMES_AL[next as usize] - skip as i32
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL_MIRROR => {
                let n = if s.st::<State>(ci).mirrors == 0 { 2 } else { 1 };
                mirror_gain(s, ci, n);
            }
            TASK_MIRROR_LOSS => mirror_loss(s, ci, arg, 1),
            TASK_BURST => {
                if s.player.active as usize != ci {
                    return;
                }
                mirror_gain(s, ci, arg);
                if s.chars[ci].cons >= 4 {
                    c4_gain(s, ci, arg);
                }
            }
            _ => unreachable!(),
        }
    }

    /// A1 before the projection, so the mirror it adds counts for it.
    /// C1 before the particle, the order gcsim passes the callbacks.
    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_A1 != 0 {
            a1(s, ci);
        }
        if cbs & CB_PROJECTION != 0 {
            projection(s, ci, ev);
        }
        if cbs & CB_C1 != 0 {
            c1(s, ci);
        }
        if cbs & CB_PARTICLE != 0 {
            particle(s, ci);
        }
    }

    /// While he has a mirror, normal, charged and plunging attacks are
    /// dendro. gcsim sets this after weapon infusion, so it wins.
    fn snapshot(s: &Sim, ci: usize, info: &mut AttackInfo) {
        if s.st::<State>(ci).mirrors <= 0 {
            return;
        }
        if matches!(info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            info.element = Element::Dendro;
        }
    }

    /// A4: EM, up to 100%, on projection attacks and the burst.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key != A4 {
            return;
        }
        if ev.info.tag != AttackTag::ElementalBurst && ev.info.icd_group != icd::GROUP_ALHAITHAM_PROJECTION_ATTACK {
            return;
        }
        let mut dmg = 0.001 * s.stat(ci, st::EM);
        if dmg > 1.0 {
            dmg = 1.0;
        }
        ev.stats[st::DMGP] += dmg;
    }

    /// C6: CRIT Rate and CRIT DMG while the overflow buff lasts.
    fn mod_amount(_: &Sim, _: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key != C6 {
            return false;
        }
        out[st::CR] += 0.1;
        out[st::CD] += 0.7;
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Swap { prev, .. } = *e else { return };
        if prev != ci {
            return;
        }
        let st = s.st_mut::<State>(ci);
        st.last_infusion = -1;
        st.mirrors = 0;
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "mirrors" => Some(Ok(Num::Int(s.st::<State>(ci).mirrors as i64))),
            "c2-stacks" => {
                let n = (0..4).filter(|&i| s.mod_active(ci, C2_KEYS[i])).count();
                Some(Ok(Num::Int(n as i64)))
            }
            _ => None,
        }
    }
}

fn skill_tap(s: &mut Sim, ci: usize) -> ActInfo {
    rush(s, ci, "Universality: An Elaboration on Form", g::SKILL_TAP_HITMARK, 1.0, 2.25);
    s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_MIRROR, arg: 0 }, 15);
    s.set_cd_with_delay(ci, Action::Skill, 18 * 60, 15);
    let t = &g::SKILL_TAP_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Skill)
}

fn skill_hold(s: &mut Sim, ci: usize) -> ActInfo {
    rush(s, ci, "Universality: An Elaboration on Form (Hold)", g::SKILL_HOLD_HITMARK, 2.0, 2.25);
    s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_MIRROR, arg: 0 }, 23);
    s.set_cd_with_delay(ci, Action::Skill, 18 * 60, 23);
    let t = &g::SKILL_HOLD_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::LowPlunge as usize], AnimState::Skill)
}

/// The skill's rush. Its center is the enemy, offset further along the
/// facing (gcsim's NewCircleHit from the player onto the primary target).
fn rush(s: &mut Sim, ci: usize, abil: &'static str, hitmark: i32, oy: f64, r: f64) {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::RUSH_ATK[lvl]);
    ai.abil = abil;
    ai.flat_dmg = g::RUSH_EM[lvl] * s.stat(ci, st::EM);
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = 0.04 * 60.0;
    ai.def_halt = true;
    ai.pattern = Pattern::Circle { x: 0.0, y: s.target_dist + oy, r };
    s.queue_attack(ai, 0, hitmark, hitmark);
}

fn mirror_gain(s: &mut Sim, ci: usize, generated: i32) {
    if generated == 0 {
        return;
    }
    let f = s.f;
    if s.st::<State>(ci).mirrors == 0 {
        s.st_mut::<State>(ci).last_infusion = f;
        s.add_task(Task::Char { c: ci as u8, id: TASK_MIRROR_LOSS, arg: f }, g::MIRROR_INTERVAL);
    }
    s.st_mut::<State>(ci).mirrors += generated;
    let mirrors = s.st::<State>(ci).mirrors;
    if s.chars[ci].cons >= 2 {
        c2(s, ci, generated);
    }
    if mirrors > 3 {
        let overflow = mirrors - 3;
        if s.chars[ci].cons >= 6 {
            c6(s, ci, overflow);
        }
        s.st_mut::<State>(ci).mirrors = 3;
        // one loss task per frame, when several gains overflow together
        if f != s.st::<State>(ci).last_infusion {
            s.st_mut::<State>(ci).last_infusion = f;
            s.add_task(Task::Char { c: ci as u8, id: TASK_MIRROR_LOSS, arg: f }, g::MIRROR_INTERVAL);
        }
    }
}

/// `src` is the infusion frame that queued this loss. A newer infusion
/// ignores it. Not extended by hitlag: gcsim queues it on the core.
fn mirror_loss(s: &mut Sim, ci: usize, src: i32, consumed: i32) {
    if consumed <= 0 {
        return;
    }
    if s.st::<State>(ci).last_infusion != src || s.st::<State>(ci).mirrors == 0 {
        return;
    }
    let st = s.st_mut::<State>(ci);
    st.mirrors -= consumed;
    if st.mirrors < 0 {
        st.mirrors = 0;
    }
    let left = st.mirrors;
    if left > 0 {
        s.add_task(Task::Char { c: ci as u8, id: TASK_MIRROR_LOSS, arg: src }, g::MIRROR_INTERVAL);
    }
}

fn a1(s: &mut Sim, ci: usize) {
    if s.chars[ci].ascension < 1 || s.core_status(g::A1_ICD_KEY) > 0 {
        return;
    }
    // the fight's status, not his: hitlag does not extend it
    s.add_core_status(A1_ICD, 12 * 60);
    mirror_gain(s, ci, 1);
}

fn projection(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    if s.mod_active(ci, PROJECTION_ICD) || s.player.active as usize != ci {
        return;
    }
    let mirrors = s.st::<State>(ci).mirrors;
    if mirrors == 0 || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
        return;
    }
    let lvl = s.talent(ci, 1);
    let flat = g::MIRROR_EM[lvl] * s.stat(ci, st::EM);
    let pattern = match mirrors {
        3 => s.circle_on_player(0.0, 4.0, 4.0),
        2 => s.fan_on_player(0.0, -0.1, 5.5, 180.0),
        _ => s.box_on_player(0.0, 0.0, 7.0, 3.0),
    };
    let snap = g::SNAPSHOT_TIMINGS[(mirrors as usize) - 1];
    let marks = mirror_marks(s, mirrors);
    let cbs = CB_PARTICLE | if s.chars[ci].cons >= 1 { CB_C1 } else { 0 };
    for i in 0..mirrors as usize {
        let mut ai = AttackInfo::new(
            ci,
            AttackTag::ElementalArt,
            icd::TAG_ELEMENTAL_ART,
            icd::GROUP_ALHAITHAM_PROJECTION_ATTACK,
            Element::Dendro,
            25.0,
            g::MIRROR_ATK[lvl],
        );
        ai.abil = PROJ[(mirrors as usize) - 1];
        ai.flat_dmg = flat;
        ai.pattern = pattern;
        s.queue_attack(ai, cbs, snap, marks[i]);
    }
    s.add_status(ci, PROJECTION_ICD, 96, true);
}

fn mirror_marks(s: &mut Sim, mirrors: i32) -> &'static [i32] {
    match mirrors {
        3 => g::MIRROR3_HITMARKS,
        2 if s.rng.float64() < 0.5 => g::MIRROR2_HITMARKS_RIGHT,
        2 => g::MIRROR2_HITMARKS_LEFT,
        _ if s.rng.float64() < 0.5 => g::MIRROR1_HITMARK_RIGHT,
        _ => g::MIRROR1_HITMARK_LEFT,
    }
}

fn c1(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 1 || s.mod_active(ci, C1_ICD) {
        return;
    }
    s.reduce_cd(ci, Action::Skill, 72);
    s.add_status(ci, C1_ICD, 60, true);
}

fn particle(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, PARTICLE_ICD) {
        return;
    }
    s.add_status(ci, PARTICLE_ICD, 90, true);
    s.queue_particle(1.0, Element::Dendro, s.particle_delay[ci]);
}

fn c2(s: &mut Sim, ci: usize, generated: i32) {
    for _ in 0..generated {
        let i = s.st::<State>(ci).c2_counter as usize;
        s.st_mut::<State>(ci).c2_counter = (i as i32 + 1) % g::C2_MAX_STACKS;
        s.add_stat_mod(ci, C2_KEYS[i], st::EM, 50.0, 8 * 60, true);
    }
}

fn c4_loss(s: &mut Sim, ci: usize, consumed: i32) {
    if consumed <= 0 {
        return;
    }
    let em = 30.0 * consumed as f64;
    for i in 0..s.chars.len() {
        if i != ci {
            s.add_stat_mod(i, C4_LOSS, st::EM, em, 15 * 60, true);
        }
    }
}

fn c4_gain(s: &mut Sim, ci: usize, generated: i32) {
    if generated <= 0 {
        return;
    }
    s.add_stat_mod(ci, C4_GAIN, st::DENDROP, 0.1 * generated as f64, 15 * 60, true);
}

fn c6(s: &mut Sim, ci: usize, generated: i32) {
    for _ in 0..generated {
        if s.mod_active(ci, C6) {
            s.extend_mod(ci, C6, 6 * 60);
        } else {
            let o = Owner::Char(ci as u8);
            s.add_mod(ci, Mod::owned(C6, ModKind::Stat, o, st::NONE, false, true, -1), 6 * 60);
        }
    }
}
