//! Vesna, from gcsim's internal/characters/vesna: normal and charged
//! attacks with their Wind Pinions, the skill (The Art of Victory) and its
//! Armed for Action state with Sword Essence and Windborne Sword Lv. 1 to
//! 3, the burst's Spirit Blade, A1 (Disciplinary Action), A4, C1 to C6
//! (C6's Windborne Sword: Transpose), and her Stellar part: she enables
//! Stellar Swirl (`stellar.rs`), the Base DMG of every direct Stellar
//! Swirl hit and Stellar Swirl contribution gains 0.7% per 100 of her ATK
//! (at most 14%), and in Radiance: Stellar Swirl her Spirit Blades deal
//! direct Stellar Swirl DMG instead. No plunges (gcsim has none).

use crate::content::{Character, Event, EventKind, Owner};
use crate::gcsimdata::chars::vesna as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::stellar::RADIANCE_SWIRL;
use crate::types::*;

pub struct Char;

pub struct State {
    /// gcsim's armedSrc: the frame Armed for Action started, 0 when not.
    armed_src: i32,
    essence: f64,
    wb_level: i32,
    lv3_count: i32,
    lv3_max: i32,
    c1_free: bool,
    /// A1's Disciplinary Action count and the expiry of each instance.
    a1_count: i32,
    a1_expiry: Vec<i32>,
    /// The area of the Lv. 3 Spirit Blades, set as the sword is used.
    wb3_ap: Pattern,
    /// The first hit of a normal or charged attack, built as the attack
    /// starts and dealt by the task that fires its pinions (one task in
    /// gcsim, so the hit's hitlag does not delay them).
    pinion_hit: AttackInfo,
}

const ARMED: ModKey = ModKey::new(g::ARMED_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const C1: ModKey = ModKey::new(g::C1_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C6_TRANSPOSE: ModKey = ModKey::new(g::C6_TRANSPOSE_KEY);
const C6_COMBO: ModKey = ModKey::new(g::C6_COMBO_KEY);

const CB_PARTICLE: u16 = 1 << 0;

const TASK_ARMED_EXPIRY: u8 = 0;
const TASK_A1_EXPIRE: u8 = 1;
const TASK_WB1: u8 = 2;
const TASK_WB2: u8 = 3;
const TASK_WB2_BLADE: u8 = 4;
const TASK_WB3_BLADE: u8 = 5;
const TASK_BURST_BLADE: u8 = 6;
const TASK_PINIONS: u8 = 7;
const TASK_TRANSPOSE_HIT: u8 = 8;
const TASK_TRANSPOSE_BLADE: u8 = 9;

/// The Spirit Blades: their names, and their names as direct Stellar
/// Swirl DMG.
const BLADE_LV2: usize = 0;
const BLADE_LV3: usize = 1; // to 4, and 5 the final hit
const BLADE_BURST: usize = 6;
const BLADE_TRANSPOSE: usize = 7;
const BLADES: [(&str, &str); 8] = [
    ("Windborne Sword Lv. 2 Spirit Blade", "Windborne Sword Lv. 2 Spirit Blade (Stellar Swirl)"),
    ("Windborne Sword Lv. 3 Spirit Blade 1", "Windborne Sword Lv. 3 Spirit Blade 1 (Stellar Swirl)"),
    ("Windborne Sword Lv. 3 Spirit Blade 2", "Windborne Sword Lv. 3 Spirit Blade 2 (Stellar Swirl)"),
    ("Windborne Sword Lv. 3 Spirit Blade 3", "Windborne Sword Lv. 3 Spirit Blade 3 (Stellar Swirl)"),
    ("Windborne Sword Lv. 3 Spirit Blade 4", "Windborne Sword Lv. 3 Spirit Blade 4 (Stellar Swirl)"),
    ("Windborne Sword Lv. 3 Spirit Blade Final Hit", "Windborne Sword Lv. 3 Spirit Blade Final Hit (Stellar Swirl)"),
    ("For the Tsaritsa! Spirit Blade", "For the Tsaritsa! Spirit Blade (Stellar Swirl)"),
    ("Windborne Sword: Transpose Spirit Blade", "Windborne Sword: Transpose Spirit Blade (Stellar Swirl)"),
];

/// The Wind Pinions' names: after each normal attack, the charged attack
/// and Transpose.
const PINION_CHARGE: i32 = 6;
const PINION_TRANSPOSE: i32 = 7;
const PINIONS: [&str; 8] = [
    "Wind Pinion (N1)",
    "Wind Pinion (N2)",
    "Wind Pinion (N3)",
    "Wind Pinion (N4)",
    "Wind Pinion (N5)",
    "Wind Pinion (N6)",
    "Wind Pinion (Charge)",
    "Wind Pinion (Transpose)",
];

/// The normal attacks' offsets (gcsim's attackOffsets).
const ATTACK_OFFSETS: [(f64, f64); 6] = [(0.0, -1.0), (0.0, 0.98), (0.0, -1.53), (0.0, 0.2), (0.0, 0.5), (-0.43, -0.76)];

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn armed(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, ARMED)
}

fn radiance(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, RADIANCE_SWIRL)
}

fn add_essence(s: &mut Sim, ci: usize, v: f64) {
    let state = st_mut(s, ci);
    state.essence = (state.essence + v).min(g::ESSENCE_MAX as f64);
}

/// A1: an instance of Disciplinary Action for 20 s, each with its own
/// expiry (a core task).
fn a1_add(s: &mut Sim, ci: usize) {
    if s.chars[ci].ascension < 1 {
        return;
    }
    let exp = s.f + g::A1_DUR;
    let state = st_mut(s, ci);
    state.a1_count = (state.a1_count + 1).min(g::A1_MAX_STACKS);
    state.a1_expiry.push(exp);
    s.add_task(Task::Char { c: ci as u8, id: TASK_A1_EXPIRE, arg: exp }, g::A1_DUR);
}

fn a1_expire(s: &mut Sim, ci: usize, exp: i32) {
    let state = st_mut(s, ci);
    if let Some(i) = state.a1_expiry.iter().position(|&e| e == exp) {
        state.a1_expiry.remove(i);
        state.a1_count = (state.a1_count - 1).max(0);
    }
}

fn a1_clear(s: &mut Sim, ci: usize) {
    let state = st_mut(s, ci);
    state.a1_count = 0;
    state.a1_expiry.clear();
}

fn a1_mult(s: &Sim, ci: usize) -> f64 {
    1.0 + g::A1_PER_STACK * st(s, ci).a1_count as f64
}

/// gcsim's startArmed.
fn start_armed(s: &mut Sim, ci: usize) {
    let c1 = s.chars[ci].cons >= 1;
    let mut dur = g::ARMED_DUR * 60;
    if c1 {
        dur += g::C1_ARMED_EXTRA;
    }
    let f = s.f;
    s.add_status(ci, ARMED, dur, true);
    let state = st_mut(s, ci);
    state.armed_src = f;
    state.wb_level = 1;
    state.lv3_count = 0;
    state.lv3_max = if c1 { 4 } else { 3 };
    state.c1_free = c1;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ARMED_EXPIRY, arg: f }, dur);
}

fn check_armed_expiry(s: &mut Sim, ci: usize, src: i32) {
    if st(s, ci).armed_src != src {
        return;
    }
    if armed(s, ci) {
        let d = s.mod_duration(ci, g::ARMED_KEY).max(1);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ARMED_EXPIRY, arg: src }, d);
        return;
    }
    end_armed(s, ci);
}

/// gcsim's endArmed.
fn end_armed(s: &mut Sim, ci: usize) {
    if st(s, ci).armed_src == 0 {
        return;
    }
    s.delete_mod(ci, ARMED);
    let state = st_mut(s, ci);
    state.armed_src = 0;
    state.essence = 0.0;
    state.wb_level = 0;
    state.c1_free = false;
}

fn on_windborne(s: &mut Sim, ci: usize) {
    let state = st_mut(s, ci);
    state.wb_level = (state.wb_level + 1).min(3);
    a1_add(s, ci);
}

/// gcsim's windborneAI.
fn windborne_ai(ci: usize, abil: &'static str, mult: f64) -> AttackInfo {
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_VESNA_MAGIC_SWORD, icd::GROUP_VESNA_MAGIC_SWORD, Element::Anemo, 25.0, mult);
    ai.abil = abil;
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = 0.03 * 60.0;
    ai
}

/// gcsim's spiritBlade: Windborne Sword DMG, or direct Stellar Swirl DMG
/// in Radiance (`star`, read as the sword was used).
fn spirit_blade(s: &mut Sim, ci: usize, blade: usize, star: bool, mult: f64, mult_ssw: f64, ap: Pattern, burst: bool) {
    let a1 = a1_mult(s, ci);
    let (abil, abil_ssw) = BLADES[blade];
    if !star {
        let mut ai = windborne_ai(ci, abil, mult * a1);
        if burst {
            ai.tag = AttackTag::ElementalBurst;
            ai.icd_tag = icd::TAG_ELEMENTAL_BURST;
            ai.icd_group = icd::GROUP_DEFAULT;
            ai.hitlag_halt = 0.05 * 60.0;
            ai.hitlag_factor = 0.05;
        }
        ai.deployable = true;
        ai.pattern = ap;
        s.queue_attack(ai, CB_PARTICLE, 0, 0);
        return;
    }
    let mut ai = AttackInfo::new(ci, AttackTag::DirectStellarSwirl, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 0.0, mult_ssw * a1);
    ai.abil = abil_ssw;
    ai.ignore_def = 1.0;
    ai.pattern = ap;
    s.queue_attack(ai, CB_PARTICLE, 0, 0);
}

/// gcsim's pinion: only while armed, Sword Essence and a hit 12 frames
/// later.
fn pinion(s: &mut Sim, ci: usize, name: i32) {
    if !armed(s, ci) {
        return;
    }
    add_essence(s, ci, g::ESSENCE_PER_PINON as f64);
    let lvl = s.talent(ci, 1);
    let mut ai = windborne_ai(ci, PINIONS[name as usize], g::PINION[lvl]);
    ai.hitlag_halt = 0.0;
    ai.hitlag_factor = 0.0;
    ai.deployable = true;
    ai.pattern = Pattern::Single;
    s.queue_attack(ai, CB_PARTICLE, 0, g::PINION_TRAVEL);
}

fn star_arg(star: bool) -> i32 {
    star as i32
}

/// C6's Windborne Sword: Transpose, by the skill or a normal attack.
fn transpose(s: &mut Sim, ci: usize) -> ActInfo {
    s.delete_mod(ci, C6_TRANSPOSE);
    s.add_status(ci, C6_COMBO, g::TRANSPOSE_LENGTH + 60, true);
    let star = radiance(s, ci);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_TRANSPOSE_HIT, arg: 0 }, g::TRANSPOSE_HIT);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_TRANSPOSE_BLADE, arg: star_arg(star) }, g::TRANSPOSE_BLADE);
    let t = &g::TRANSPOSE_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
}

fn windborne(s: &mut Sim, ci: usize) -> ActInfo {
    let state = st_mut(s, ci);
    let level = state.wb_level;
    if level == 3 && state.c1_free {
        state.c1_free = false;
    } else {
        state.essence -= g::ESSENCE_COST as f64;
    }
    match level {
        1 => {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_WB1, arg: 0 }, g::WB1_HITMARK);
            let t = &g::WB1_FRAMES;
            ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
        }
        2 => {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_WB2, arg: 0 }, g::WB2_HITMARK);
            let t = &g::WB2_FRAMES;
            ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
        }
        _ => {
            let star = radiance(s, ci);
            on_windborne(s, ci);
            let state = st_mut(s, ci);
            state.lv3_count += 1;
            let last = state.lv3_count >= state.lv3_max;
            if s.chars[ci].cons >= 6 {
                s.add_status(ci, C6_TRANSPOSE, g::C6_TRANSPOSE_DUR, true);
            }
            let ap = s.circle_on_player(0.0, 1.0, 6.0);
            st_mut(s, ci).wb3_ap = ap;
            for (i, &d) in g::WB3_HITMARKS.iter().enumerate() {
                let arg = star_arg(star) | ((BLADE_LV3 + i) as i32) << 1;
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_WB3_BLADE, arg }, d);
            }
            let arg = star_arg(star) | ((BLADE_LV3 + 4) as i32) << 1 | (last as i32) << 4;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_WB3_BLADE, arg }, g::WB3_FINAL);
            let t = &g::WB3_FRAMES;
            ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
        }
    }
}

fn particle_cb(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, PARTICLE_ICD) {
        return;
    }
    s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD, false);
    s.queue_particle(1.0, Element::Anemo, s.particle_delay[ci]);
}

/// stellarInit's bonus: 0.7% per 100 of her ATK, at most 14%.
fn stellar_bonus(s: &Sim, ci: usize) -> f64 {
    (s.total_atk(ci) / 100.0 * g::STELLAR_PER100).min(g::STELLAR_MAX)
}

impl Character for Char {
    const KEY: &'static str = "vesna";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            armed_src: 0,
            essence: 0.0,
            wb_level: 0,
            lv3_count: 0,
            lv3_max: 0,
            c1_free: false,
            a1_count: 0,
            a1_expiry: Vec::with_capacity(16),
            wb3_ap: Pattern::Single,
            pinion_hit: AttackInfo::new(0, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 0.0, 0.0),
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // stellarInit (StellarSwirlEnableKey is set before any Init) and
        // C6's elevation of her Stellar Swirl contributions
        s.subscribe(EventKind::EnemyHit, o);
        s.subscribe(EventKind::SpecialReactionAttack, o);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // A4: ATK% and EM by the party's elements, in Radiance
        if asc >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Stat, o, st::NONE, false, false, -1));
        }
        // C1: her Stellar Swirl DMG while armed
        if cons >= 1 {
            s.chars[ci].mods.push(Mod::owned(C1, ModKind::ReactBonus, o, st::NONE, false, false, -1));
        }
        // C2: ATK% at six instances of Disciplinary Action
        if cons >= 2 && asc >= 1 {
            s.chars[ci].mods.push(Mod::owned(C2, ModKind::Stat, o, st::ATKP, false, false, -1));
        }
        s.subscribe(EventKind::Swap, o);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::Charge && s.mod_active(ci, C6_COMBO) {
            return None;
        }
        crate::content::template_invalid(s, a)
    }

    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        if a != Action::Skill {
            return None;
        }
        if s.mod_active(ci, C6_TRANSPOSE) {
            return Some(None);
        }
        if armed(s, ci) {
            let state = st(s, ci);
            if state.essence >= g::ESSENCE_COST as f64 || (state.wb_level == 3 && state.c1_free) {
                return Some(None);
            }
            return Some(Some(Failure::SkillCd));
        }
        None
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, C6_TRANSPOSE) {
            return transpose(s, ci);
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let is_armed = armed(s, ci);
        let (ox, oy) = ATTACK_OFFSETS[n];
        let hb = g::ATTACK_HITBOXES[n];
        let ap = if hb.len() == 2 { s.box_on_player(ox, oy, hb[0], hb[1]) } else { s.circle_on_player(ox, oy, hb[0]) };
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = true;
            if is_armed {
                ai.element = Element::Anemo;
                ai.ignore_infusion = true;
            }
            ai.pattern = ap;
            let h = g::ATTACK_HITMARKS[n][i];
            if i == 0 {
                st_mut(s, ci).pinion_hit = ai;
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_PINIONS, arg: n as i32 }, h);
            } else {
                s.queue_attack_at(ci, ai, 0, h);
            }
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let hm = g::ATTACK_HITMARKS[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hm[hm.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge";
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = 0.03 * 60.0;
        if armed(s, ci) {
            ai.element = Element::Anemo;
            ai.ignore_infusion = true;
        }
        ai.pattern = s.circle_on_player(0.0, 0.57, 3.0);
        st_mut(s, ci).pinion_hit = ai;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_PINIONS, arg: PINION_CHARGE }, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, C6_TRANSPOSE) {
            return transpose(s, ci);
        }
        if armed(s, ci) {
            return windborne(s, ci);
        }
        a1_clear(s, ci);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL[lvl]);
        ai.abil = "The Art of Victory";
        ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
        s.queue_attack(ai, CB_PARTICLE, g::SKILL_HITMARK, g::SKILL_HITMARK);
        start_armed(s, ci);
        add_essence(s, ci, g::ESSENCE_SKILL as f64);
        // C2: six instances of Disciplinary Action
        if s.chars[ci].cons >= 2 {
            for _ in 0..g::A1_MAX_STACKS {
                a1_add(s, ci);
            }
        }
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD, 0);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        add_essence(s, ci, g::ESSENCE_BURST as f64);
        a1_add(s, ci);
        let star = radiance(s, ci);
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_BLADE, arg: star_arg(star) }, g::BURST_HITMARK);
        s.set_cd(ci, Action::Burst, g::BURST_CD);
        s.consume_energy(ci, 5);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_ARMED_EXPIRY => check_armed_expiry(s, ci, arg),
            TASK_A1_EXPIRE => a1_expire(s, ci, arg),
            TASK_WB1 => {
                on_windborne(s, ci);
                let lvl = s.talent(ci, 1);
                let mut ai = windborne_ai(ci, "Windborne Sword Lv. 1", g::WINDBORNE1[lvl]);
                ai.hitlag_halt = 0.0;
                ai.pattern = Pattern::Single;
                s.queue_attack(ai, CB_PARTICLE, 0, 0);
            }
            TASK_WB2 => {
                on_windborne(s, ci);
                let lvl = s.talent(ci, 1);
                let mut ai = windborne_ai(ci, "Windborne Sword Lv. 2", g::WINDBORNE2[lvl]);
                ai.pattern = s.circle_on_player(0.0, 2.3, 5.0);
                s.queue_attack(ai, CB_PARTICLE, 0, 0);
                let star = radiance(s, ci);
                s.add_task(Task::Char { c: ci as u8, id: TASK_WB2_BLADE, arg: star_arg(star) }, g::WB2_BLADE_WAIT);
            }
            TASK_WB2_BLADE => {
                let lvl = s.talent(ci, 1);
                let ap = s.circle_on_target(5.0);
                spirit_blade(s, ci, BLADE_LV2, arg & 1 != 0, g::BLADE2[lvl], g::BLADE2_S_SW[lvl], ap, false);
            }
            TASK_WB3_BLADE => {
                let lvl = s.talent(ci, 1);
                let blade = (arg >> 1 & 7) as usize;
                let (m, m_ssw) = if blade == BLADE_LV3 + 4 { (g::BLADE3_FINAL[lvl], g::BLADE3_FINAL_S_SW[lvl]) } else { (g::BLADE3[lvl], g::BLADE3_S_SW[lvl]) };
                let ap = st(s, ci).wb3_ap;
                spirit_blade(s, ci, blade, arg & 1 != 0, m, m_ssw, ap, false);
                if arg >> 4 & 1 != 0 {
                    end_armed(s, ci);
                }
            }
            TASK_BURST_BLADE => {
                let lvl = s.talent(ci, 2);
                let ap = s.circle_on_player(0.0, 5.0, 6.5);
                spirit_blade(s, ci, BLADE_BURST, arg & 1 != 0, g::BURST[lvl], g::BURST_S_SW[lvl], ap, true);
            }
            TASK_PINIONS => {
                let ai = st(s, ci).pinion_hit;
                s.queue_attack(ai, 0, 0, 0);
                let count = if arg == PINION_CHARGE { 2 } else { g::ATTACK_PINIONS[arg as usize] };
                for _ in 0..count {
                    pinion(s, ci, arg);
                }
            }
            TASK_TRANSPOSE_HIT => {
                let mut ai = windborne_ai(ci, "Windborne Sword: Transpose", g::C6_MULT);
                ai.hitlag_halt = 0.04 * 60.0;
                ai.pattern = s.circle_on_player(0.24, 1.45, 3.0);
                s.queue_attack(ai, CB_PARTICLE, 0, 0);
                pinion(s, ci, PINION_TRANSPOSE);
            }
            TASK_TRANSPOSE_BLADE => {
                let ap = s.circle_on_player(0.0, 2.0, 4.0);
                let m = g::C6_BLADE_MULT as f64;
                spirit_blade(s, ci, BLADE_TRANSPOSE, arg & 1 != 0, m, m, ap, false);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 {
            particle_cb(s, ci);
        }
    }

    /// C6: her direct Stellar Swirl DMG is elevated by 20% (gcsim's
    /// OnApplyAttack, for her own attacks).
    fn own_hit(s: &Sim, ci: usize, ev: &mut AttackEvent) {
        if s.chars[ci].cons >= 6 && ev.info.tag == AttackTag::DirectStellarSwirl {
            ev.elevation += (g::C6_ELEVATION * 1000.0).round() as u16;
        }
    }

    /// A4's ATK% and EM in Radiance: Stellar Swirl (tripled at C4); C2's
    /// ATK% at six instances of Disciplinary Action.
    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C2 {
            if st(s, ci).a1_count < g::A1_MAX_STACKS {
                return false;
            }
            out[st::ATKP] += g::C2_ATK;
            return true;
        }
        if !radiance(s, ci) {
            return false;
        }
        let mut ice_wind = 0;
        for c in &s.chars {
            if matches!(c.element, Element::Cryo | Element::Anemo) {
                ice_wind += 1;
            }
        }
        let other = s.chars.len() as i32 - ice_wind;
        let ice_wind = ice_wind.min(g::A4_MAX_COUNT);
        let other = other.min(g::A4_MAX_COUNT - ice_wind);
        let factor = if s.chars[ci].cons >= 4 { 3.0 } else { 1.0 };
        out[st::ATKP] += g::A4_ATK * ice_wind as f64 * factor;
        out[st::EM] += g::A4_EM as f64 * other as f64 * factor;
        true
    }

    /// C1: Stellar Swirl DMG +20% while armed.
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if !info.tag.is_stellar_swirl() || !armed(s, ci) {
            return 0.0;
        }
        g::C1_BONUS
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // leaving the field ends Armed for Action, Transpose and A1
            Event::Swap { prev, .. } => {
                if prev != ci {
                    return;
                }
                end_armed(s, ci);
                s.delete_mod(ci, C6_TRANSPOSE);
                a1_clear(s, ci);
            }
            Event::EnemyHit(ev) => {
                if ev.info.tag == AttackTag::DirectStellarSwirl {
                    s.hit_base_bonus += stellar_bonus(s, ci);
                }
            }
            Event::SpecialReactionAttack(ev) => {
                if ev.info.tag != AttackTag::ReactionStellarSwirl {
                    return;
                }
                s.special_base_bonus += stellar_bonus(s, ci);
                if s.chars[ci].cons >= 6 && ev.info.actor as usize == ci {
                    s.special_atk.elevation += (g::C6_ELEVATION * 1000.0).round() as u16;
                }
            }
            _ => {}
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let state = st(s, ci);
        let v = match fields[0].as_str() {
            "armed" => Num::from_bool(armed(s, ci)),
            "essence" => Num::Int(state.essence as i64),
            "windborne-level" => Num::Int(if armed(s, ci) { state.wb_level as i64 } else { 0 }),
            "lv3-count" => Num::Int(state.lv3_count as i64),
            "disciplinary-action" => Num::Int(state.a1_count as i64),
            "radiance" => Num::from_bool(radiance(s, ci)),
            _ => return None,
        };
        Some(Ok(v))
    }
}
