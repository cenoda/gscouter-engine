//! Lohen, from gcsim's internal/characters/lohen: normal and charged
//! attacks (Masterstroke's cryo ones too), Unforeseen Strike with Joy,
//! Will to Win and Etched Into Bone and Soul, the burst, A1, A4, When the
//! Mood Strikes (High Spirits), his Hexerei Secret Rite, and every
//! constellation. No plunges (gcsim has none for him). He is Hexerei.
//!
//! As in gcsim:
//! - Joy: each of his hits on the enemy in Masterstroke adds 17, at most
//!   once every 0.1 s per source (normal attacks, the charged attack's
//!   first hit, its second), until Etched Into Bone and Soul has been used
//!   its most times. At 100 the skill is Etched Into Bone and Soul,
//!   ready whatever the skill's cooldown.
//! - Will to Win: each teammate's damage on the enemy in Masterstroke,
//!   not reaction damage unless direct, adds 20 when at least 10 times his
//!   base ATK and 1 otherwise, and with A1 60 more when at least 30 times,
//!   each once every 0.1 s per teammate. C1 makes it 5 times as fast and
//!   its limit 300.
//! - Etched Into Bone and Soul and the burst take the Will to Win as they
//!   start (the burst at its gadget, 88 frames on) and empty it, unless
//!   C6 keeps it.
//! - Leaving the field ends Masterstroke; the Etched count is reset only by
//!   the next Masterstroke.

use crate::content::{Character, Event};
use crate::gamedata::chars::lohen as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    joy: f64,
    will: f64,
    etched: i32,
    /// The frame until which each Joy source is gated.
    joy_gate: [i32; 3],
    /// The frame until which each teammate's Will to Win sources (success,
    /// fail, A1) are gated.
    will_gate: [[i32; 3]; 4],
    c6_mark: bool,
    c2_armed: bool,
    /// The Will to Win the Etched Into Bone and Soul in flight took.
    etched_will: f64,
    /// Its multiplier, taken as it starts.
    etched_mult: f64,
    /// The burst in flight: C6's crit, and its multiplier from the gadget.
    burst_c6: bool,
    burst_mult: f64,
}

const MASTERSTROKE: ModKey = ModKey::new(g::MASTERSTROKE_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_KEY);
const HIGH_SPIRITS: ModKey = ModKey::new(g::HIGH_SPIRITS_KEY);
const HIGH_SPIRITS_ICD: ModKey = ModKey::new(g::HIGH_SPIRITS_ICD_KEY);
const HEX: ModKey = ModKey::new(g::HEX_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const C2_WINDOW: ModKey = ModKey::new(g::C2_WINDOW_KEY);
const C2_EM: ModKey = ModKey::new(g::C2_EM_KEY);
const C4: ModKey = ModKey::new(g::C4_KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const CB_JOY_NORMAL: u16 = 1 << 0;
const CB_JOY_CHARGE_A: u16 = 1 << 1;
const CB_JOY_CHARGE_B: u16 = 1 << 2;
const CB_PARTICLE: u16 = 1 << 3;
const CB_C2: u16 = 1 << 4;
const CB_ETCHED_FINAL: u16 = 1 << 5;

// arg: the hit
const TASK_ETCHED_HIT: u8 = 0;
const TASK_BURST_GADGET: u8 = 1;
// arg: the hit
const TASK_BURST_HIT: u8 = 2;
const TASK_BURST_END: u8 = 3;

const MASTERSTROKE_NORMAL_NAMES: [&str; 5] = ["Masterstroke Normal 0", "Masterstroke Normal 1", "Masterstroke Normal 2", "Masterstroke Normal 3", "Masterstroke Normal 4"];
const ETCHED_NAMES: [&str; 4] = ["Etched Into Bone and Soul 0", "Etched Into Bone and Soul 1", "Etched Into Bone and Soul 2", "Etched Into Bone and Soul 3"];
const BURST_NAMES: [&str; 6] = ["Manifest Judgment 0", "Manifest Judgment 1", "Manifest Judgment 2", "Manifest Judgment 3", "Manifest Judgment 4", "Manifest Judgment 5"];

impl Character for Char {
    const KEY: &'static str = "lohen";
    const HEXEREI: bool = true;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { joy: 0.0, will: 0.0, etched: 0, joy_gate: [0; 3], will_gate: [[0; 3]; 4], c6_mark: false, c2_armed: false, etched_will: 0.0, etched_mult: 0.0, burst_c6: false, burst_mult: 0.0 }
    }

    /// gcsim's Init: leaving the field, Will to Win, A4.
    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        s.subscribe(EventKind::Swap, o);
        s.subscribe(EventKind::EnemyDamage, o);
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::Reaction, o);
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<crate::gcsl::Num, String>> {
        use crate::gcsl::Num;
        let ms = masterstroke(s, ci);
        let st = s.st::<State>(ci);
        Some(Ok(match fields[0].as_str() {
            "masterstroke" => Num::Int(i64::from(ms)),
            "joy" => Num::Float(if ms { st.joy } else { 0.0 }),
            "will" => Num::Float(if ms { st.will } else { 0.0 }),
            "etched" => Num::Int(if ms { st.etched as i64 } else { 0 }),
            _ => return None,
        }))
    }

    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && etched_ready(s, ci)).then_some(None)
    }

    fn action_stam(s: &Sim, ci: usize, a: Action) -> Option<f64> {
        (a == Action::Charge && masterstroke(s, ci)).then_some(10.0)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let ms = masterstroke(s, ci);
        let lvl = s.talent(ci, 0);
        let slvl = skill_lvl(s, ci);
        for i in 0..g::ATTACK_HITMARKS[n].len() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_LOHEN_NORMAL_ATTACK, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            if ms {
                ai.abil = MASTERSTROKE_NORMAL_NAMES[n];
                ai.element = Element::Cryo;
                ai.mult = g::SKILL_ATTACK[n][i][slvl];
            }
            let hb = g::ATTACK_HITBOXES[n][i];
            ai.pattern = if hb.len() == 2 { s.box_on_player(0.0, 0.0, hb[0], hb[1]) } else { s.circle_on_player(0.0, 0.0, hb[0]) };
            s.queue_attack_at(ci, ai, CB_JOY_NORMAL | CB_PARTICLE | CB_C2, g::ATTACK_HITMARKS[n][i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let ms = masterstroke(s, ci);
        let lvl = s.talent(ci, 0);
        let slvl = skill_lvl(s, ci);
        const NAMES: [&str; 2] = ["Charge 0", "Charge 1"];
        const MS_NAMES: [&str; 2] = ["Masterstroke Charge 0", "Masterstroke Charge 1"];
        for (i, &h) in g::CHARGE_HITMARKS.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_LOHEN_NORMAL_ATTACK, Element::Physical, 25.0, g::CHARGE[lvl]);
            ai.abil = NAMES[i];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::CHARGE_HITLAG_HALT_FRAME[i] * 60.0;
            if ms {
                ai.abil = MS_NAMES[i];
                ai.element = Element::Cryo;
                ai.mult = g::SKILL_CHARGE[slvl];
            }
            ai.pattern = s.box_on_player(0.0, 0.0, 3.5, 4.8);
            let joy = if i == 0 { CB_JOY_CHARGE_A } else { CB_JOY_CHARGE_B };
            s.queue_attack_at(ci, ai, joy | CB_PARTICLE | CB_C2, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARKS[1], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if etched_ready(s, ci) {
            return etched(s, ci);
        }
        let st = s.st_mut::<State>(ci);
        st.etched = 0;
        st.joy = 0.0;
        st.will = 0.0;
        st.c6_mark = false;
        st.joy_gate = [0; 3];
        st.will_gate = [[0; 3]; 4];
        s.add_status(ci, MASTERSTROKE, g::SKILL_DURATION * 60, true);
        high_spirits(s, ci);
        // C4: 15 energy when not full, else on the next burst within 15 s
        if s.chars[ci].cons >= 4 {
            if s.chars[ci].energy < s.chars[ci].energy_max {
                s.add_energy(ci, g::C4_ENERGY as f64);
            } else {
                s.add_status(ci, C4, g::C4_DUR, true);
            }
        }
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD, 0);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let ms = masterstroke(s, ci);
        s.st_mut::<State>(ci).burst_c6 = s.chars[ci].cons >= 6 && ms;
        extend_masterstroke(s, ci, g::BURST_EXTEND);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_GADGET, arg: 0 }, g::BURST_GADGET);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_END, arg: 0 }, g::BURST_END);
        s.set_cd(ci, Action::Burst, g::BURST_CD);
        s.consume_energy(ci, 4);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_ETCHED_HIT => {
                let i = arg as usize;
                let n = g::ETCHED_HITMARKS.len();
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, s.st::<State>(ci).etched_mult);
                ai.abil = ETCHED_NAMES[i];
                if i == n - 1 {
                    ai.hitlag_factor = 0.01;
                    ai.hitlag_halt = 0.02 * 60.0;
                }
                let cbs = if i == n - 1 { CB_PARTICLE | CB_ETCHED_FINAL } else { CB_PARTICLE };
                // snapshot at the hit, with C6's CRIT DMG
                let mut ev = s.snapshot_event(ai, cbs);
                if s.chars[ci].cons >= 6 {
                    ev.stats[st::CD] += g::C6_CD;
                }
                ev.info.pattern = s.circle_on_player(0.0, 0.0, 2.5);
                s.queue_attack_with_snap(ev, cbs, 0);
            }
            TASK_BURST_GADGET => {
                if s.chars[ci].cons >= 4 && masterstroke(s, ci) {
                    s.st_mut::<State>(ci).will = will_max(s, ci);
                }
                let will = current_will(s, ci);
                hexerei(s, ci, will);
                if !c6_keep_will(s, ci) {
                    s.st_mut::<State>(ci).will = 0.0;
                }
                // C4: the energy kept for this burst
                if s.chars[ci].cons >= 4 && s.mod_active(ci, C4) {
                    s.delete_mod(ci, C4);
                    s.add_energy(ci, g::C4_ENERGY as f64);
                }
                let lvl = s.talent(ci, 2);
                s.st_mut::<State>(ci).burst_mult = g::BURST[lvl] * (1.0 + will * g::BURST_WILL);
                for (i, &h) in g::BURST_HITMARKS.iter().enumerate() {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_HIT, arg: i as i32 }, h);
                }
            }
            TASK_BURST_HIT => {
                let i = arg as usize;
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, s.st::<State>(ci).burst_mult);
                ai.abil = BURST_NAMES[i];
                let r = if i == g::BURST_HITMARKS.len() - 1 { 6.5 } else { 5.0 };
                let mut ev = s.snapshot_event(ai, 0);
                if s.st::<State>(ci).burst_c6 {
                    ev.stats[st::CD] += g::C6_CD;
                }
                ev.info.pattern = s.circle_on_player(0.0, 5.0, r);
                s.queue_attack_with_snap(ev, 0, 0);
            }
            TASK_BURST_END => {
                c2_arm(s, ci);
                c6_trigger(s, ci);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_JOY_NORMAL != 0 {
            add_joy(s, ci, 0);
        }
        if cbs & CB_JOY_CHARGE_A != 0 {
            add_joy(s, ci, 1);
        }
        if cbs & CB_JOY_CHARGE_B != 0 {
            add_joy(s, ci, 2);
        }
        // a particle from his cryo hits, once every 2 s
        if cbs & CB_PARTICLE != 0 && ev.info.element == Element::Cryo && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD, true);
            s.queue_particle(1.0, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_C2 != 0 {
            c2_hit(s, ci);
        }
        // the last Etched hit: Hexerei, C2, C6
        if cbs & CB_ETCHED_FINAL != 0 {
            let will = s.st::<State>(ci).etched_will;
            hexerei(s, ci, will);
            c2_arm(s, ci);
            c6_trigger(s, ci);
        }
    }

    /// Hexerei's Secret Rite: normal and charged attack DMG +40%.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            ev.stats[st::DMGP] += g::HEX_BONUS;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { prev, .. } => {
                if prev == ci {
                    s.delete_mod(ci, MASTERSTROKE);
                    let st = s.st_mut::<State>(ci);
                    st.joy = 0.0;
                    st.will = 0.0;
                    st.c6_mark = false;
                }
            }
            Event::EnemyDamage(ev) => will_to_win(s, ci, ev),
            // A4: a cryo reaction by another member in Masterstroke gives
            // them and him 15% ATK for 8 s
            Event::Reaction(r, ev) => {
                if !matches!(r, Reaction::Melt | Reaction::Frozen | Reaction::Superconduct | Reaction::SwirlCryo | Reaction::StellarSwirl | Reaction::CrystallizeCryo | Reaction::StellarConduct) {
                    return;
                }
                if !ev.on_enemy() || ev.info.actor as usize == ci || !masterstroke(s, ci) {
                    return;
                }
                let who = ev.info.actor as usize;
                s.add_mod(who, Mod::stat(A4, st::ATKP, g::A4_ATK, false, true, -1), g::A4_DUR);
                s.add_mod(ci, Mod::stat(A4, st::ATKP, g::A4_ATK, false, true, -1), g::A4_DUR);
            }
            _ => {}
        }
    }
}

fn masterstroke(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, MASTERSTROKE)
}

fn current_will(s: &Sim, ci: usize) -> f64 {
    if masterstroke(s, ci) {
        s.st::<State>(ci).will
    } else {
        0.0
    }
}

fn etched_max(s: &Sim, ci: usize) -> i32 {
    if s.chars[ci].cons >= 6 {
        g::ETCHED_MAX_C6
    } else {
        g::ETCHED_MAX_C0
    }
}

fn etched_ready(s: &Sim, ci: usize) -> bool {
    let st = s.st::<State>(ci);
    masterstroke(s, ci) && st.joy >= g::JOY_MAX as f64 && st.etched < etched_max(s, ci)
}

fn will_max(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].cons >= 1 {
        (g::WILL_MAX_BASE * g::C1_WILL_MAX_RATIO) as f64
    } else {
        g::WILL_MAX_BASE as f64
    }
}

fn add_will(s: &mut Sim, ci: usize, amt: f64) {
    let amt = if s.chars[ci].cons >= 1 { amt * g::C1_WILL_EFFICIENCY as f64 } else { amt };
    let max = will_max(s, ci);
    let st = s.st_mut::<State>(ci);
    st.will = (st.will + amt).min(max);
}

fn add_joy(s: &mut Sim, ci: usize, src: usize) {
    if !masterstroke(s, ci) || s.st::<State>(ci).etched >= etched_max(s, ci) {
        return;
    }
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    if f < st.joy_gate[src] {
        return;
    }
    st.joy_gate[src] = f + g::JOY_GATE_DUR;
    st.joy = (st.joy + g::JOY_PER_HIT as f64).min(g::JOY_MAX as f64);
}

/// The skill's level with High Spirits, as an index into the tables.
fn skill_lvl(s: &Sim, ci: usize) -> usize {
    let lvl = s.talent(ci, 1) + usize::from(s.mod_active(ci, HIGH_SPIRITS));
    lvl.min(14)
}

/// Masterstroke's remaining duration grows by `dur`, up to 20 s.
fn extend_masterstroke(s: &mut Sim, ci: usize, dur: i32) {
    if !masterstroke(s, ci) {
        return;
    }
    let dur = dur.min(g::MASTERSTROKE_MAX_DUR - s.mod_duration(ci, g::MASTERSTROKE_KEY));
    if dur > 0 {
        s.extend_mod(ci, MASTERSTROKE, dur);
    }
}

/// When the Mood Strikes: High Spirits for 9 s, 15 s when another member's
/// normal attack, skill or burst level is at least his skill's.
fn high_spirits(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, HIGH_SPIRITS_ICD) {
        return;
    }
    s.add_status(ci, HIGH_SPIRITS_ICD, g::HIGH_SPIRITS_ICD, true);
    let mut dur = g::HIGH_SPIRITS_DUR;
    let lvl = s.talent(ci, 1);
    for i in 0..s.chars.len() {
        if i == ci {
            continue;
        }
        if s.talent(i, 0) >= lvl || s.talent(i, 1) >= lvl || s.talent(i, 2) >= lvl {
            dur += g::HIGH_SPIRITS_EXTRA_DUR;
            break;
        }
    }
    s.add_status(ci, HIGH_SPIRITS, dur, true);
}

/// Hexerei: Secret Rite, from Will to Win at half its limit or more.
fn hexerei(s: &mut Sim, ci: usize, will: f64) {
    if !s.chars[ci].hexerei || s.hexerei_count() < 2 {
        return;
    }
    if will < will_max(s, ci) * g::HEX_THRESHOLD {
        return;
    }
    s.add_mod(ci, Mod::owned(HEX, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), g::HEX_DUR);
}

fn c6_keep_will(s: &Sim, ci: usize) -> bool {
    s.chars[ci].cons >= 6 && masterstroke(s, ci) && !s.mod_active(ci, C6_ICD)
}

/// C6: Joy at its limit, and the next Etched extends Masterstroke.
fn c6_trigger(s: &mut Sim, ci: usize) {
    if !c6_keep_will(s, ci) || s.st::<State>(ci).etched >= etched_max(s, ci) {
        return;
    }
    let st = s.st_mut::<State>(ci);
    st.joy = g::JOY_MAX as f64;
    st.c6_mark = true;
    s.add_status(ci, C6_ICD, g::C6_ICD, true);
}

/// C2: Evilsbane Blade for 4 s, once every 4 s.
fn c2_arm(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 2 || s.mod_active(ci, C2_WINDOW) {
        return;
    }
    s.add_status(ci, C2_WINDOW, g::C2_WINDOW, true);
    s.st_mut::<State>(ci).c2_armed = true;
}

/// C2: the next normal or charged attack hit in Masterstroke follows up
/// with a cryo strike and gives the others 200 EM for 8 s.
fn c2_hit(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 2 || !s.st::<State>(ci).c2_armed {
        return;
    }
    if !masterstroke(s, ci) || !s.mod_active(ci, C2_WINDOW) {
        return;
    }
    s.st_mut::<State>(ci).c2_armed = false;
    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::C2_MULT as f64);
    ai.abil = "Evilsbane Blade (C2)";
    ai.pattern = s.box_on_player(0.0, 1.3, 5.5, 5.5);
    s.queue_attack(ai, 0, 0, 0);
    for i in 0..s.chars.len() {
        if i != ci {
            s.add_mod(i, Mod::stat(C2_EM, st::EM, g::C2_EM as f64, false, true, -1), g::C2_EM_DUR);
        }
    }
}

/// Will to Win from a teammate's damage on the enemy.
fn will_to_win(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    let who = ev.info.actor as usize;
    if who == ci {
        return;
    }
    if ev.info.tag.is_reaction() && !ev.info.tag.is_direct() {
        return;
    }
    if !masterstroke(s, ci) || ev.damage <= 0.0 {
        return;
    }
    let base = s.stat(ci, st::BASEATK);
    let dmg = ev.damage;
    if dmg >= base * g::WILL_RATIO as f64 {
        try_gate(s, ci, who, g::WILL_GATE_SUCCESS as usize, g::WILL_SUCCESS as f64);
    } else {
        try_gate(s, ci, who, g::WILL_GATE_FAIL as usize, g::WILL_FAIL as f64);
    }
    if s.chars[ci].ascension >= 1 && dmg >= base * g::A1_RATIO as f64 {
        try_gate(s, ci, who, g::WILL_GATE_A1 as usize, g::A1_WILL as f64);
    }
}

fn try_gate(s: &mut Sim, ci: usize, who: usize, i: usize, amt: f64) {
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    if f < st.will_gate[who][i] {
        return;
    }
    st.will_gate[who][i] = f + g::WILL_GATE_DUR;
    add_will(s, ci, amt);
}

/// Etched Into Bone and Soul.
fn etched(s: &mut Sim, ci: usize) -> ActInfo {
    let keep = c6_keep_will(s, ci);
    let st = s.st_mut::<State>(ci);
    let will = st.will;
    st.etched += 1;
    st.joy = 0.0;
    if !keep {
        st.will = 0.0;
    }
    st.etched_will = will;
    let mark = st.c6_mark;
    if mark {
        s.st_mut::<State>(ci).c6_mark = false;
        extend_masterstroke(s, ci, g::C6_EXTEND);
    }
    s.st_mut::<State>(ci).etched_mult = g::SKILL_ETCHED[skill_lvl(s, ci)] * (1.0 + will * g::SKILL_WILL);
    for (i, &h) in g::ETCHED_HITMARKS.iter().enumerate() {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ETCHED_HIT, arg: i as i32 }, h);
    }
    let t = &g::ETCHED_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
}
