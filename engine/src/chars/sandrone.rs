//! Sandrone, from gcsim's internal/characters/sandrone: normal attacks,
//! the held charged attack (Fagio's Decoding mode with its sweeping fire
//! and condensed beams, and Power Overdrive with its shots, driven by
//! Decoding Power), the skill's hover and its Prism Shots, the burst's
//! bombardment and ray, A1 (the 400% Prism Shot, Refined Tactics and the
//! ray's bonus), A4, C1 to C6, and her Stellar part: she turns
//! superconduct into Stellar-Conduct (`stellar.rs`), and the Base DMG of
//! every direct Stellar-Conduct hit gains 0.7% per 100 of her ATK (at most
//! 14%). In Radiance: Stellar-Conduct (inside a Polestar Field) her beams,
//! second Prism Shot, ray, C4 cannon and C6 cluster beam deal direct
//! Stellar-Conduct DMG instead. Stellar Swirl, and with it Radiance:
//! Stellar Swirl and the Stellar Swirl halves of these, is not ported: a
//! team that could swirl cryo with her is an error. No plunges.

use crate::content::{Character, Event, EventKind, Owner};
use crate::gcsimdata::chars::sandrone as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::stellar::POLESTAR_FIELD;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// Fagio's Decoding Power (0 to 100) and Power Overdrive mode.
    power: f64,
    overdrive: bool,
    /// Decoding Power lost since the last burst (A1's Refined Tactics) and
    /// the frame of the last loss.
    power_lost: f64,
    power_lost_src: i32,
    /// The source frames of the current charged attack and hover, 0 when
    /// none.
    ca_src: i32,
    hover_src: i32,
    /// The source frames of the last charged attack and skill, for their
    /// OnRemoved.
    last_ca: i32,
    last_hover: i32,
    /// Bumped by the skill to cancel a pending set to 100.
    force_gen: i32,
    /// The beam counter of the current Decoding period.
    count: i32,
    /// Condensed beams of the current Decoding period (C2).
    c2_layers: i32,
    a4_em: f64,
    /// The ray's factor from A1, for the burst under way.
    ray_mult: f64,
    /// A1's 400% on the second Prism Shot, for the skill under way.
    a1_prism: bool,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const C1: ModKey = ModKey::new(g::C1_KEY);
const C4_ICD: ModKey = ModKey::new(g::C4_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_C4: u16 = 1 << 1;

// tasks; arg: the charged attack's or hover's source frame unless noted
const TASK_SWEEP_START: u8 = 0;
const TASK_DECODING: u8 = 1;
const TASK_RELEASE: u8 = 2;
const TASK_DEC_TICK: u8 = 3;
const TASK_COUNTER: u8 = 4;
// arg: the force generation
const TASK_FORCE: u8 = 5;
const TASK_FIRE: u8 = 6;
const TASK_SWEEP: u8 = 7;
const TASK_SHOT: u8 = 8;
const TASK_C6_BEAM: u8 = 9;
const TASK_DECAY: u8 = 10;
// arg: the frame of the loss
const TASK_LOST_RESET: u8 = 11;
const TASK_C1: u8 = 12;
const TASK_DRAIN: u8 = 13;
const TASK_END_HOVER: u8 = 14;
const TASK_PRISM2: u8 = 15;
const TASK_RAY: u8 = 16;
const TASK_A4: u8 = 17;

const ON_REMOVED_CHARGE: u8 = 0;
const ON_REMOVED_SKILL: u8 = 1;

const NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];

fn gain_ratio(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].cons >= 1 { 0.5 } else { 1.0 }
}

/// The beam count that sets Decoding Power to 100 (none at C6).
fn forced_count(s: &Sim, ci: usize) -> i32 {
    match s.chars[ci].cons {
        6.. => -1,
        1.. => 6,
        _ => 3,
    }
}

fn ca_active(s: &Sim, ci: usize, src: i32) -> bool {
    src != 0 && s.st::<State>(ci).ca_src == src
}

fn decoding_active(s: &Sim, ci: usize, src: i32) -> bool {
    ca_active(s, ci, src) && !s.st::<State>(ci).overdrive
}

fn add_power(s: &mut Sim, ci: usize, v: f64) {
    let st = s.st_mut::<State>(ci);
    st.power = (st.power + v).clamp(0.0, g::POWER_MAX as f64);
    if !st.overdrive && st.power >= g::OVERDRIVE_ENTER {
        st.overdrive = true;
        let src = st.ca_src;
        if ca_active(s, ci, src) {
            start_overdrive_shots(s, ci, src);
        }
    } else if st.overdrive && st.power <= g::OVERDRIVE_EXIT as f64 {
        st.overdrive = false;
    }
}

/// Lowers Decoding Power, counting the loss towards Refined Tactics.
fn reduce_power(s: &mut Sim, ci: usize, v: f64) {
    let lost = v.min(s.st::<State>(ci).power);
    add_power(s, ci, -v);
    if lost > 0.0 {
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        st.power_lost = (st.power_lost + lost).min(g::TACTICS_MAX as f64 * 10.0);
        st.power_lost_src = f;
        s.add_task(Task::Char { c: ci as u8, id: TASK_LOST_RESET, arg: f }, g::TACTICS_DUR);
    }
}

fn refined_tactics(s: &Sim, ci: usize) -> i32 {
    ((s.st::<State>(ci).power_lost / 10.0) as i32).min(g::TACTICS_MAX)
}

/// The frames after the start of Decoding mode at which Fagio enters Power
/// Overdrive, by the same rules as decoding.
fn overdrive_after(s: &Sim, ci: usize) -> i32 {
    let st = s.st::<State>(ci);
    if st.overdrive {
        return 0;
    }
    let (ratio, cons, fc) = (gain_ratio(s, ci), s.chars[ci].cons, forced_count(s, ci));
    let mut p = st.power;
    let mut count = 0;
    let mut forced = -1;
    for f in 1..=g::MAX_HOLD {
        if f % g::DECODING_TICK == 0 {
            p += g::DECODING_GAIN * ratio;
        }
        if f % g::BEAM_TICK == 0 {
            if cons < 6 || count <= 1 {
                count += 1;
                if count == fc {
                    forced = f + g::FORCE_DELAY;
                }
            }
            p += g::BEAM_GAIN as f64 * ratio;
        }
        if f == forced {
            p += g::POWER_MAX as f64;
        }
        if p >= g::OVERDRIVE_ENTER {
            return f;
        }
    }
    g::MAX_HOLD
}

fn radiance(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, POLESTAR_FIELD)
}



/// gcsim's stellarInfo, for Radiance: Stellar-Conduct: the hit becomes
/// direct Stellar-Conduct DMG that applies no element.
fn stellar_info(ai: &mut AttackInfo, abil: &'static str, mult: f64) {
    ai.icd_tag = icd::TAG_NONE;
    ai.icd_group = icd::GROUP_DEFAULT;
    ai.durability = 0.0;
    ai.ignore_def = 1.0;
    ai.abil = abil;
    ai.tag = AttackTag::DirectStellarConduct;
    ai.mult = mult;
}

fn c4_cb(s: &Sim, ci: usize) -> u16 {
    if s.chars[ci].cons >= 4 { CB_C4 } else { 0 }
}

/// A beam hit: cryo DMG, or direct Stellar-Conduct DMG in Radiance.
/// `cluster` is C6's Condensed Cluster Beam.
fn fire_beam(s: &mut Sim, ci: usize, cluster: bool) {
    let lvl = s.talent(ci, 0);
    let abil = if cluster { g::C6_CLUSTER } else { "Condensed Beam" };
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 0.0);
    ai.abil = abil;
    ai.blunt = true;
    ai.mult = if cluster { g::C6_CLUSTER_MULT } else { g::BEAM[lvl] };
    let rad = radiance(s, ci);
    if rad {
        let (abil, mult) = if cluster { ("Condensed Cluster Beam (Stellar-Conduct)", g::C6_CLUSTER_SSC) } else { ("Condensed Beam (Stellar-Conduct)", g::BEAM_SSC[lvl]) };
        stellar_info(&mut ai, abil, mult);
    }
    ai.pattern = s.circle_on_target(2.5);
    let mut ev = s.snapshot_event(ai, 0);
    if rad {
        ev.stats[st::CD] += c2_crit_dmg(s, ci);
    }
    // the Stellar-Conduct versions always count for particles, the normal
    // cluster beam does not
    let cbs = if !cluster || rad { CB_PARTICLE | c4_cb(s, ci) } else { c4_cb(s, ci) };
    s.queue_attack_with_snap(ev, cbs, 0);
}

fn c2_crit_dmg(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].cons < 2 {
        return 0.0;
    }
    g::C2_BASE + g::C2_PER_LAYER * s.st::<State>(ci).c2_layers as f64
}

fn start_decoding(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).overdrive {
        start_overdrive_shots(s, ci, src);
        return;
    }
    let st = s.st_mut::<State>(ci);
    st.c2_layers = 0;
    st.count = 0;
    let c = ci as u8;
    s.add_task(Task::Char { c, id: TASK_DEC_TICK, arg: src }, g::DECODING_TICK);
    s.add_task(Task::Char { c, id: TASK_COUNTER, arg: src }, g::BEAM_TICK);
    s.add_task(Task::Char { c, id: TASK_FIRE, arg: src }, g::BEAM_FIRST);
}

/// Power Overdrive: a shot now and every 0.4 s while the charged attack
/// is held.
fn start_overdrive_shots(s: &mut Sim, ci: usize, src: i32) {
    overdrive_shot(s, ci, src);
}

fn overdrive_shot(s: &mut Sim, ci: usize, src: i32) {
    if !ca_active(s, ci, src) || !s.st::<State>(ci).overdrive {
        return;
    }
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_SANDRONE_MACHINE_GUN, icd::GROUP_SANDRONE_MACHINE_GUN, Element::Cryo, 25.0, g::OVERDRIVE[lvl]);
    ai.abil = "Power Overdrive Shot";
    ai.pattern = s.circle_on_target(0.5);
    s.queue_attack(ai, 0, 0, 0);
    s.add_task(Task::Char { c: ci as u8, id: TASK_SHOT, arg: src }, g::OVERDRIVE_INTERVAL);
}

impl Character for Char {
    const KEY: &'static str = "sandrone";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { ray_mult: 1.0, ..State::default() }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // stellarInit: the Base DMG bonus on direct Stellar-Conduct hits
        s.subscribe(EventKind::EnemyHit, o);
        // fagioInit: Decoding Power decays
        s.add_task(Task::Char { c: ci as u8, id: TASK_DECAY, arg: 0 }, g::DECAY_INTERVAL);
        // A4: EM from her ATK, refreshed every 0.4 s
        if s.chars[ci].ascension >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Stat, o, st::EM, false, false, -1));
            a4_update(s, ci);
        }
        // C1: Stellar DMG +30% for the party while she charges
        if s.chars[ci].cons >= 1 {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(C1, ModKind::ReactBonus, o, st::NONE, false, false, -1));
            }
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::Charge => p.0.iter().find(|(k, _)| k != "hold").map(|(k, _)| format!("charge param {k}")),
            Action::Attack | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let st = s.st::<State>(ci);
        match fields[0].as_str() {
            "power" => Some(Ok(Num::Float(st.power))),
            "overdrive" => Some(Ok(Num::from_bool(st.overdrive))),
            "tactics" => Some(Ok(Num::Int(refined_tactics(s, ci) as i64))),
            _ => None,
        }
    }

    fn action_stam(_: &Sim, _: usize, a: Action) -> Option<f64> {
        // Fagio fires the charged attack: no stamina
        (a == Action::Charge).then_some(0.0)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let h = g::ATTACK_HITMARKS[n];
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NAMES[n];
        ai.blunt = true;
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG[n] * 60.0;
        ai.def_halt = true;
        ai.pattern = s.circle_on_player(0.0, 1.0, g::ATTACK_RADIUS[n]);
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = match p.get("hold") {
            Some(h) => (h as i32).clamp(0, g::MAX_HOLD),
            None if s.st::<State>(ci).overdrive => g::OVERDRIVE_HOLD_DEF,
            None => overdrive_after(s, ci),
        };
        let src = s.f + 1;
        let st = s.st_mut::<State>(ci);
        st.ca_src = src;
        st.last_ca = src;
        // C1: the party buff, refreshed every 1.5 s while she charges
        if s.chars[ci].cons >= 1 {
            c1_refresh(s, ci, src);
        }
        let c = ci as u8;
        s.add_task(Task::Char { c, id: TASK_SWEEP_START, arg: src }, g::SWEEP_START);
        s.add_task(Task::Char { c, id: TASK_DECODING, arg: src }, g::CHARGE_ENTRY);
        s.add_task(Task::Char { c, id: TASK_RELEASE, arg: src }, g::CHARGE_ENTRY + hold);
        let len = g::CHARGE_ENTRY + hold;
        let t = &g::CHARGE_END_FRAMES;
        let mut info = ActInfo::new(Frames::Char { id: 0, arg: hold as i16 }, len + t[0], len + t[Action::Dash as usize], AnimState::Charge);
        info.on_removed = OnRemoved::Char(ON_REMOVED_CHARGE);
        info
    }

    fn frames(_: u8, hold: i16, next: Action) -> i32 {
        g::CHARGE_ENTRY + hold as i32 + g::CHARGE_END_FRAMES[next as usize]
    }

    fn on_removed(s: &mut Sim, ci: usize, id: u8, _: AnimState) {
        let st = s.st_mut::<State>(ci);
        match id {
            ON_REMOVED_CHARGE => {
                if st.ca_src == st.last_ca {
                    st.ca_src = 0;
                }
            }
            _ => {
                if st.hover_src == st.last_hover {
                    st.hover_src = 0;
                }
            }
        }
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // A1: the second Prism Shot deals 400% if Decoding Power was at
        // least 50 on boarding
        let a1 = s.chars[ci].ascension >= 1 && s.st::<State>(ci).power >= g::A1_POWER_MIN as f64;
        let src = s.f + 1;
        let st = s.st_mut::<State>(ci);
        st.a1_prism = a1;
        st.force_gen += 1;
        st.hover_src = src;
        st.last_hover = src;
        reduce_power(s, ci, g::HOVER_DRAIN_FIRST as f64);
        let c = ci as u8;
        s.add_task(Task::Char { c, id: TASK_DRAIN, arg: src }, g::HOVER_TICK);
        s.add_task(Task::Char { c, id: TASK_END_HOVER, arg: src }, g::HOVER_DUR);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::PRISM[lvl]);
        ai.abil = "Prism Shot 1";
        ai.pattern = s.circle_on_target(0.5);
        s.queue_attack(ai, CB_PARTICLE, g::PRISM_FIRST, g::PRISM_FIRST + g::PRISM_TRAVEL);
        s.add_task(Task::Char { c, id: TASK_PRISM2, arg: src }, g::PRISM_RADIANCE);
        s.set_cd(ci, Action::Skill, g::SKILL_CD);
        let t = &g::SKILL_FRAMES;
        let mut info = ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill);
        info.on_removed = OnRemoved::Char(ON_REMOVED_SKILL);
        info
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // A1: the Refined Tactics stacks are spent, each adding 10% to the
        // ray's Stellar-Conduct DMG
        let mut ray_mult = 1.0;
        if s.chars[ci].ascension >= 1 {
            ray_mult += g::A1_RAY_PER_STACK * refined_tactics(s, ci) as f64;
            let st = s.st_mut::<State>(ci);
            st.power_lost = 0.0;
            st.power_lost_src = -1;
        }
        s.st_mut::<State>(ci).ray_mult = ray_mult;
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BOMBARD[lvl]);
        const BOMBARD_NAMES: [&str; 3] = ["Bombardment 0", "Bombardment 1", "Bombardment 2"];
        for (i, &h) in g::BURST_BOMBARD_HITMARKS.iter().enumerate() {
            ai.abil = BOMBARD_NAMES[i];
            ai.pattern = s.circle_on_player(0.0, 6.5, 7.3);
            s.queue_attack(ai, 0, g::BURST_ATTACK + h, g::BURST_ATTACK + h);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_RAY, arg: 0 }, g::BURST_ATTACK + g::BURST_RADIANCE);
        s.set_cd(ci, Action::Burst, g::BURST_CD);
        s.consume_energy(ci, 5);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        let c = ci as u8;
        match id {
            TASK_SWEEP_START => {
                if decoding_active(s, ci, arg) {
                    s.add_task(Task::Char { c, id: TASK_SWEEP, arg }, g::SWEEP_INTERVAL);
                }
            }
            TASK_SWEEP => {
                if !decoding_active(s, ci, arg) {
                    return;
                }
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_SANDRONE_MACHINE_GUN, icd::GROUP_SANDRONE_MACHINE_GUN, Element::Cryo, 25.0, g::SWEEP[lvl]);
                ai.abil = "Sweeping Fire";
                ai.pattern = s.circle_on_target(1.8);
                s.queue_attack(ai, 0, 0, 0);
                s.add_task(Task::Char { c, id: TASK_SWEEP, arg }, g::SWEEP_INTERVAL);
            }
            TASK_DECODING => {
                if ca_active(s, ci, arg) {
                    start_decoding(s, ci, arg);
                }
            }
            TASK_RELEASE => {
                let st = s.st_mut::<State>(ci);
                if st.ca_src == arg {
                    st.ca_src = 0;
                }
            }
            TASK_DEC_TICK => {
                if !decoding_active(s, ci, arg) {
                    return;
                }
                add_power(s, ci, g::DECODING_GAIN * gain_ratio(s, ci));
                s.add_task(Task::Char { c, id: TASK_DEC_TICK, arg }, g::DECODING_TICK);
            }
            TASK_COUNTER => {
                if !decoding_active(s, ci, arg) {
                    return;
                }
                let count = s.st::<State>(ci).count;
                if s.chars[ci].cons < 6 || count <= 1 {
                    s.st_mut::<State>(ci).count = count + 1;
                    if count + 1 == forced_count(s, ci) {
                        let fg = s.st::<State>(ci).force_gen;
                        s.add_task(Task::Char { c, id: TASK_FORCE, arg: fg }, g::FORCE_DELAY);
                    }
                } else if count == 2 {
                    s.st_mut::<State>(ci).count = 3;
                    // C6: the Condensed Cluster Beam
                    for &h in g::C6_HITMARKS {
                        s.add_task(Task::Char { c, id: TASK_C6_BEAM, arg }, h);
                    }
                }
                add_power(s, ci, g::BEAM_GAIN as f64 * gain_ratio(s, ci));
                s.add_task(Task::Char { c, id: TASK_COUNTER, arg }, g::BEAM_TICK);
            }
            TASK_FORCE => {
                // the skill removes the pending set
                if s.st::<State>(ci).force_gen == arg {
                    add_power(s, ci, g::POWER_MAX as f64);
                }
            }
            TASK_FIRE => {
                if !decoding_active(s, ci, arg) {
                    return;
                }
                if s.chars[ci].cons >= 2 {
                    let st = s.st_mut::<State>(ci);
                    st.c2_layers = (st.c2_layers + 1).min(g::C2_MAX_LAYER);
                }
                fire_beam(s, ci, false);
                s.add_task(Task::Char { c, id: TASK_FIRE, arg }, g::BEAM_INTERVAL);
            }
            TASK_C6_BEAM => {
                if decoding_active(s, ci, arg) {
                    fire_beam(s, ci, true);
                }
            }
            TASK_SHOT => overdrive_shot(s, ci, arg),
            TASK_DECAY => {
                s.add_task(Task::Char { c, id: TASK_DECAY, arg: 0 }, g::DECAY_INTERVAL);
                let st = s.st::<State>(ci);
                if st.ca_src != 0 || st.hover_src != 0 || st.power <= 0.0 {
                    return;
                }
                let mut amt = g::DECAY_AMOUNT;
                if s.player.active as usize != ci {
                    amt *= g::DECAY_OFF_FIELD as f64;
                }
                reduce_power(s, ci, amt);
            }
            TASK_LOST_RESET => {
                let st = s.st_mut::<State>(ci);
                if st.power_lost_src == arg {
                    st.power_lost = 0.0;
                }
            }
            TASK_C1 => c1_refresh(s, ci, arg),
            TASK_DRAIN => {
                if s.st::<State>(ci).hover_src != arg {
                    return;
                }
                reduce_power(s, ci, g::HOVER_DRAIN as f64);
                s.add_task(Task::Char { c, id: TASK_DRAIN, arg }, g::HOVER_TICK);
            }
            TASK_END_HOVER => {
                let st = s.st_mut::<State>(ci);
                if st.hover_src == arg {
                    st.hover_src = 0;
                }
            }
            TASK_PRISM2 => {
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::PRISM[lvl]);
                ai.abil = "Prism Shot 2";
                if radiance(s, ci) {
                    stellar_info(&mut ai, "Prism Shot 2 (Stellar-Conduct)", g::PRISM_SSC[lvl]);
                    if s.st::<State>(ci).a1_prism {
                        ai.mult *= g::A1_PRISM_MULT as f64;
                    }
                }
                ai.pattern = s.circle_on_target(0.5);
                let d = g::PRISM_SECOND - g::PRISM_RADIANCE;
                s.queue_attack(ai, CB_PARTICLE | c4_cb(s, ci), d, d + g::PRISM_TRAVEL);
            }
            TASK_RAY => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::RAY[lvl]);
                ai.abil = "Convective Inhibition Ray";
                ai.blunt = true;
                if radiance(s, ci) {
                    stellar_info(&mut ai, "Convective Inhibition Ray (Stellar-Conduct)", g::RAY_SSC[lvl]);
                    ai.mult *= s.st::<State>(ci).ray_mult;
                }
                ai.pattern = s.circle_on_player(0.0, 6.5, 7.5);
                let d = g::BURST_RAY - g::BURST_RADIANCE;
                s.queue_attack(ai, c4_cb(s, ci), d, d);
            }
            TASK_A4 => a4_update(s, ci),
            _ => unreachable!(),
        }
    }

    /// C6: her direct Stellar-Conduct DMG (stellarInfo's hits and C4's
    /// cannon, all of it) is elevated by 20%.
    fn own_hit(s: &Sim, ci: usize, ev: &mut AttackEvent) {
        if s.chars[ci].cons >= 6 && ev.info.tag == AttackTag::DirectStellarConduct {
            ev.elevation += (g::C6_ELEV * 100.0).round() as u8;
        }
    }

    /// A4's EM.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::EM] += s.st::<State>(ci).a4_em;
        true
    }

    /// C1: Stellar DMG +30% while her C1 status lasts.
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if !s.mod_active(ci, C1) || info.tag != AttackTag::DirectStellarConduct {
            return 0.0;
        }
        g::C1_BONUS
    }

    /// stellarInit: a direct Stellar-Conduct hit by anyone gains Base DMG,
    /// 0.7% per 100 of her ATK, at most 14%.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if ev.info.tag != AttackTag::DirectStellarConduct {
            return;
        }
        let bonus = (total_atk(&s.stats(ci)) / 100.0 * 0.007).min(0.14);
        s.hit_base_bonus += bonus;
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD, true);
            s.queue_particle(1.0, Element::Cryo, s.particle_delay[ci]);
        }
        // C4: direct Stellar-Conduct DMG on the enemy fires an extra
        // cannon 0.5 s later, once every 4 s
        if cbs & CB_C4 != 0 && ev.info.tag == AttackTag::DirectStellarConduct && !s.mod_active(ci, C4_ICD) {
            s.add_status(ci, C4_ICD, g::C4_ICD, false);
            let mut ai = AttackInfo::new(ci, AttackTag::DirectStellarConduct, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 0.0, g::C4_SSC);
            ai.abil = "Prismatic Resonance Cannon (C4) (Stellar-Conduct)";
            ai.ignore_def = 1.0;
            ai.pattern = Pattern::Single;
            s.queue_attack(ai, 0, g::C4_DELAY, g::C4_DELAY);
        }
    }
}

fn c1_refresh(s: &mut Sim, ci: usize, src: i32) {
    if !ca_active(s, ci, src) {
        return;
    }
    s.add_status(ci, C1, g::C1_DUR, false);
    s.add_task(Task::Char { c: ci as u8, id: TASK_C1, arg: src }, g::C1_INTERVAL);
}

fn a4_update(s: &mut Sim, ci: usize) {
    let em = (total_atk(&s.stats(ci)) * g::A4_RATIO).min(g::A4_MAX as f64);
    s.st_mut::<State>(ci).a4_em = em;
    s.add_task(Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, g::A4_INTERVAL);
}
