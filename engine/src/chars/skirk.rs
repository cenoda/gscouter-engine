//! Skirk, from gcsim's internal/characters/skirk: Serpent's Subtlety (her
//! burst's resource), the skill's Seven-Phase Flash (cryo normal and
//! charged attacks, its own dash, the drain of Serpent's Subtlety and its
//! end) and its hold (Serpent's Subtlety and the Void Rifts at once), the
//! burst's Havoc: Ruin out of Seven-Phase Flash and Havoc: Extinction in
//! it, A1's Void Rifts (from cryo reactions on the enemy, absorbed by the
//! hold, Extinction and Seven-Phase Flash's charged attacks), A4's stacks
//! from the cryo and hydro members' hits, the talent passive (a team of
//! cryo and hydro members only, with both, raises every member's skill by
//! one level), her physical attacks, and constellations 1 to 6.
//!
//! gcsim's Extinction counts its ten boosted normal attacks inside an attack
//! mod, whose AddStatus of its 0.1 s ICD is appended to the mods while
//! ApplyAttackMods compacts them and so dropped: the ICD never applies, and
//! each boosted hit counts. The count is a Cell here, the attack mod hook
//! being read-only. The mod's AddStatus of the Extinction status with no
//! duration after the tenth hit is not reproduced: it changes nothing but
//! `.skirk.status.skirk-burst-extinction` from then on.

use std::cell::Cell;

use crate::content::{Character, Event};
use crate::gcsimdata::chars::skirk as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

/// gcsim's RingQueue of frames, of capacity 3.
#[derive(Clone, Copy, Default)]
struct Ring {
    data: [i32; 3],
    start: usize,
    len: usize,
}

impl Ring {
    fn push_overwrite(&mut self, v: i32) {
        if self.len == 3 {
            self.pop();
        }
        self.data[(self.start + self.len) % 3] = v;
        self.len += 1;
    }
    fn pop(&mut self) -> Option<i32> {
        if self.len == 0 {
            return None;
        }
        let v = self.data[self.start];
        self.start = (self.start + 1) % 3;
        self.len -= 1;
        Some(v)
    }
    fn count(&self, f: impl Fn(i32) -> bool) -> i32 {
        (0..self.len).filter(|&i| f(self.data[(self.start + i) % 3])).count() as i32
    }
    fn clear(&mut self) {
        self.start = (self.start + self.len) % 3;
        self.len = 0;
    }
}

pub struct State {
    serpents: f64,
    skill_src: i32,
    prev_na_skill: bool,
    burst_count: Cell<i32>,
    burst_voids: usize,
    void_rifts: Ring,
    a1_pause_stacks: i32,
    c6_stacks: Ring,
}

const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const HOLD_ANIM: ModKey = ModKey::new(g::SKILL_ABSORB_RIFT_ANIM_KEY);
const BURST_EXTINCT: ModKey = ModKey::new(g::BURST_EXTINCT_KEY);
const BURST_EXTINCT_DMG: ModKey = ModKey::new("skirk-burst-extinction-dmg");
const BURST_ANIM: ModKey = ModKey::new(g::BURST_ABSORB_RIFT_ANIM_KEY);
const A1_ICD: ModKey = ModKey::new(g::A1_ICD_KEY);
const A1_PAUSE: ModKey = ModKey::new(g::A1_SS_PAUSE_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C4: ModKey = ModKey::new("skirk-c4");
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);
/// A4's stacks: gcsim's "deaths-crossing-char<i>".
const A4_STACK: [ModKey; 4] = [ModKey::new("deaths-crossing-char0"), ModKey::new("deaths-crossing-char1"), ModKey::new("deaths-crossing-char2"), ModKey::new("deaths-crossing-char3")];

const CB_ABSORB: u16 = 1 << 0;
const CB_C6: u16 = 1 << 1;

const TASK_ENTER_SKILL: u8 = 0;
/// arg: src.
const TASK_EXIT_SKILL: u8 = 1;
/// arg: src.
const TASK_SS_REDUCE: u8 = 2;
const TASK_HOLD_GAIN: u8 = 3;
const TASK_SS_CONSUME: u8 = 4;
const TASK_PAUSE_TICK: u8 = 5;
const TASK_C2_BUFF: u8 = 6;
/// arg: n * 4 + i, plus 64 for Seven-Phase Flash's attacks, 128 when
/// they carry C6's callback, and A4's stacks as the attack started times
/// 256.
const TASK_NA_HIT: u8 = 7;

const FR_HOLD: u8 = 0;

const REMOVED_EXTINCTION: u8 = 0;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];
const SKILL_NAMES: [&str; 5] = ["Normal (Skill) 0", "Normal (Skill) 1", "Normal (Skill) 2", "Normal (Skill) 3", "Normal (Skill) 4"];
const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];
const CHARGE_SKILL_NAMES: [&str; 3] = ["Charge (Skill) 0", "Charge (Skill) 1", "Charge (Skill) 2"];

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn char_task(s: &mut Sim, ci: usize, id: u8, arg: i32, delay: i32) {
    s.queue_char_task(ci, Task::Char { c: ci as u8, id, arg }, delay);
}

fn core_task(s: &mut Sim, ci: usize, id: u8, arg: i32, delay: i32) {
    s.add_task(Task::Char { c: ci as u8, id, arg }, delay);
}

fn add_ss(s: &mut Sim, ci: usize, e: f64) {
    let st = st_mut(s, ci);
    st.serpents = (st.serpents + e).clamp(0.0, g::MAX_SERPENTS_SUBTLETY as f64);
}

fn a4_stacks(s: &Sim, ci: usize) -> usize {
    (0..s.chars.len()).filter(|&i| s.mod_active(ci, A4_STACK[i])).count()
}

fn a4_mult_attack(s: &Sim, ci: usize) -> f64 {
    g::A4_MULT_ATTACK[a4_stacks(s, ci)]
}

fn a4_mult_burst(s: &Sim, ci: usize) -> f64 {
    g::A4_MULT_BURST[a4_stacks(s, ci)]
}

fn enter_skill_state(s: &mut Sim, ci: usize) {
    let f = s.f;
    st_mut(s, ci).skill_src = f;
    s.add_status(ci, SKILL, g::SKILL_DUR, false);
    add_ss(s, ci, 45.0);
    c2_on_skill(s, ci);
    core_task(s, ci, TASK_SS_REDUCE, f, 12);
    core_task(s, ci, TASK_EXIT_SKILL, f, g::SKILL_DUR);
}

fn exit_skill_state(s: &mut Sim, ci: usize, src: i32) {
    if st(s, ci).skill_src != src {
        return;
    }
    st_mut(s, ci).skill_src = -1;
    s.delete_mod(ci, C2);
    s.delete_mod(ci, SKILL);
    s.delete_mod(ci, BURST_EXTINCT);
    s.set_cd(ci, Action::Skill, 8 * 60);
    st_mut(s, ci).serpents = 0.0;
}

/// A1: absorbs the Void Rifts still there.
fn absorb_void_rifts(s: &mut Sim, ci: usize) -> usize {
    let f = s.f;
    let count = st(s, ci).void_rifts.count(|src| src + g::A1_DUR >= f);
    st_mut(s, ci).void_rifts.clear();
    on_void_absorb(s, ci, count);
    count as usize
}

fn on_void_absorb(s: &mut Sim, ci: usize, count: i32) {
    if count <= 0 {
        return;
    }
    add_ss(s, ci, count as f64 * 8.0);
    // the drain of Serpent's Subtlety pauses 0.3 s a rift
    s.add_status(ci, A1_PAUSE, -1, false);
    if st(s, ci).a1_pause_stacks == 0 {
        core_task(s, ci, TASK_PAUSE_TICK, 0, 9);
    }
    let st = st_mut(s, ci);
    st.a1_pause_stacks = (st.a1_pause_stacks + count * 2).min(6);
    for _ in 0..count {
        c1(s, ci);
        if s.chars[ci].cons >= 6 {
            let t = s.chars[ci].time_passed;
            st_mut(s, ci).c6_stacks.push_overwrite(t);
        }
    }
}

fn create_void_rift(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, HOLD_ANIM) {
        on_void_absorb(s, ci, 1);
    }
    if s.mod_active(ci, BURST_ANIM) {
        on_void_absorb(s, ci, 1);
        let st = st_mut(s, ci);
        st.burst_voids = (st.burst_voids + 1).min(3);
    }
    let f = s.f;
    st_mut(s, ci).void_rifts.push_overwrite(f);
}

/// C1: a hit of 500% ATK for each rift absorbed.
fn c1(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 1 {
        return;
    }
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_SKIRK_CONS, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 5.0);
    ai.abil = "Far to Fall (C1)";
    ai.pattern = s.box_on_player(0.0, -3.0, 4.0, 8.0);
    s.queue_attack(ai, 0, 3, 3);
}

fn c2_on_skill(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons >= 2 {
        add_ss(s, ci, 10.0);
    }
}

/// C6 on Ruin: a hit for each stack.
fn c6_on_burst_ruin(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 6 {
        return;
    }
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 7.5 * a4_mult_burst(s, ci));
    ai.abil = "Havoc: Sever (Burst)";
    ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
    let t = s.chars[ci].time_passed;
    let count = st(s, ci).c6_stacks.count(|src| src + g::C6_DUR >= t);
    st_mut(s, ci).c6_stacks.clear();
    for _ in 0..count {
        s.queue_attack(ai, 0, 12, 12);
    }
}

impl Character for Char {
    const KEY: &'static str = "skirk";
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        let mut ss = g::MAX_SERPENTS_SUBTLETY as i64;
        if let Some(p) = cs.params.iter().find(|p| p.0 == "start_serpents_subtlety") {
            ss = p.1;
        }
        if let Some(p) = cs.params.iter().find(|p| p.0 == "start_energy") {
            ss = p.1;
        }
        State {
            serpents: ss.clamp(0, g::MAX_SERPENTS_SUBTLETY as i64) as f64,
            skill_src: 0,
            prev_na_skill: false,
            burst_count: Cell::new(0),
            burst_voids: 0,
            void_rifts: Ring::default(),
            a1_pause_stacks: 0,
            c6_stacks: Ring::default(),
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // onExitField
        s.subscribe(EventKind::Swap, o);
        // particleInit
        s.subscribe(EventKind::EnemyDamage, o);
        // BurstInit: Extinction's DMG
        s.chars[ci].mods.push(Mod::owned(BURST_EXTINCT_DMG, ModKind::Attack, o, st::NONE, false, false, -1));
        // a1Init
        s.subscribe(EventKind::Reaction, o);
        // a4Init
        s.subscribe(EventKind::EnemyHit, o);
        // talentPassiveInit: cryo and hydro only, with both
        let all = s.chars.iter().all(|c| matches!(c.element, Element::Cryo | Element::Hydro));
        let cryo = s.chars.iter().any(|c| c.element == Element::Cryo);
        let hydro = s.chars.iter().any(|c| c.element == Element::Hydro);
        if all && cryo && hydro {
            for c in s.chars.iter_mut() {
                c.skill_talent_bonus = 1;
            }
        }
        // c4Init: ATK by A4's stacks
        if s.chars[ci].cons >= 4 {
            s.chars[ci].mods.push(Mod::owned(C4, ModKind::Stat, o, st::ATKP, false, false, -1));
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let known: &[&str] = match a {
            Action::Attack | Action::Charge | Action::Burst => &[],
            Action::Skill => &["hold"],
            _ => return Some(name.into()),
        };
        p.0.iter().find(|(k, _)| !known.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
    }

    /// Charged attacks need no normal attack before them; no skill in
    /// Seven-Phase Flash.
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::Charge {
            return None;
        }
        if s.mod_active(ci, SKILL) && a == Action::Skill {
            return Some("skirk: cannot use skill in seven-phase flash".into());
        }
        crate::content::template_invalid(s, a)
    }

    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        match a {
            Action::Burst if !s.ignore_burst_energy && !s.mod_active(ci, SKILL) && st(s, ci).serpents < 50.0 => Some(Some(Failure::InsufficientEnergy)),
            Action::Skill if s.mod_active(ci, SKILL) => Some(Some(Failure::SkillCd)),
            _ => None,
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let st = st(s, ci);
        match fields[0].as_str() {
            "serpents_subtlety" => Some(Ok(Num::Float(st.serpents))),
            "void_rifts" => {
                let f = s.f;
                Some(Ok(Num::Int(st.void_rifts.count(|src| src + g::A1_DUR >= f) as i64)))
            }
            "a4_stacks" => Some(Ok(Num::Int(a4_stacks(s, ci) as i64))),
            "c6_stacks" => {
                if s.chars[ci].cons < 6 {
                    return Some(Ok(Num::Int(0)));
                }
                let t = s.chars[ci].time_passed;
                Some(Ok(Num::Int(st.c6_stacks.count(|src| src + g::C6_DUR >= t) as i64)))
            }
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let skill = s.mod_active(ci, SKILL);
        let mut windup_remove = 0;
        if skill {
            if n == 0 && s.current_state() == AnimState::Normal {
                windup_remove = g::WINDUP;
            }
            let c6 = if s.chars[ci].cons >= 6 && matches!(n, 2 | 4) { 128 } else { 0 };
            let a4 = a4_stacks(s, ci) as i32 * 256;
            for (i, &h) in g::ATTACK_SKILL_HITMARKS[n].iter().enumerate() {
                char_task(s, ci, TASK_NA_HIT, (n * 4 + i) as i32 + 64 + c6 + a4, h - windup_remove);
            }
            st_mut(s, ci).prev_na_skill = true;
        } else {
            if n == 0 && matches!(s.current_state(), AnimState::Burst | AnimState::Normal) {
                windup_remove = g::WINDUP;
            }
            for (i, &h) in g::ATTACK_HITMARKS[n].iter().enumerate() {
                char_task(s, ci, TASK_NA_HIT, (n * 4 + i) as i32, h - g::WINDUP);
            }
            st_mut(s, ci).prev_na_skill = false;
        }
        let (table, marks) = if skill { (&g::ATTACK_SKILL_FRAMES[n], g::ATTACK_SKILL_HITMARKS[n]) } else { (&g::ATTACK_FRAMES[n], g::ATTACK_HITMARKS[n]) };
        let info = ActInfo::new(Frames::AttackLive { table, minus: windup_remove }, table[0] - windup_remove, marks.last().unwrap() - windup_remove, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // Seven-Phase Flash's, also after its N1 to N4
        let skill = s.mod_active(ci, SKILL)
            || (s.current_state() == AnimState::Normal && s.chars[s.player.active as usize].normal_counter > 0 && st(s, ci).prev_na_skill);
        if skill {
            let lvl = s.talent(ci, 1);
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 0.0);
            ai.ignore_infusion = true;
            ai.pattern = s.circle_on_player(0.0, 0.0, 3.3);
            for (i, mult) in g::SKILL_CHARGE.iter().enumerate() {
                ai.mult = mult[lvl];
                ai.abil = CHARGE_SKILL_NAMES[i];
                let h = g::CHARGE_SKILL_HITMARKS[i];
                s.queue_attack(ai, CB_ABSORB, h, h);
            }
            let t = &g::CHARGE_SKILL_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Charge);
        }
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, 0.0);
        ai.pattern = s.circle_on_target(2.8);
        for (i, mult) in g::CHARGE.iter().enumerate() {
            ai.mult = mult[lvl];
            ai.abil = CHARGE_NAMES[i];
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(0) as i32;
        if hold > 0 {
            let extra = hold.min(184) - 1;
            char_task(s, ci, TASK_HOLD_GAIN, 0, g::SKILL_HOLD_GAIN_SS);
            s.add_status(ci, HOLD_ANIM, extra, true);
            s.set_cd_with_delay(ci, Action::Skill, 8 * 60, extra + g::SKILL_HOLD_GAIN_SS);
            let t = &g::SKILL_HOLD_FRAMES;
            return ActInfo::new(Frames::Char { id: FR_HOLD, arg: extra as i16 }, t[0] + extra, t[Action::Swap as usize] + extra, AnimState::Skill);
        }
        char_task(s, ci, TASK_ENTER_SKILL, 0, g::SKILL_DELAY);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, SKILL) {
            // Havoc: Extinction
            s.add_status(ci, BURST_EXTINCT, 750, false);
            st(s, ci).burst_count.set(10);
            let voids = absorb_void_rifts(s, ci);
            st_mut(s, ci).burst_voids = voids;
            let t = &g::BURST_SKILL_FRAMES;
            s.add_status(ci, BURST_ANIM, t[0], true);
            if s.chars[ci].cons >= 2 {
                char_task(s, ci, TASK_C2_BUFF, 0, 39);
            }
            s.set_cd(ci, Action::Burst, 15 * 60);
            let mut info = ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Burst);
            info.on_removed = OnRemoved::Char(REMOVED_EXTINCTION);
            return info;
        }
        // Havoc: Ruin
        let c2 = if s.chars[ci].cons >= 2 { 10.0 } else { 0.0 };
        let bonus = (st(s, ci).serpents - 50.0).min(12.0 + c2).max(0.0);
        let lvl = s.talent(ci, 2);
        let a4 = a4_mult_burst(s, ci);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, (g::BURST_DO_T[lvl] + bonus * g::BURST_BONUS[lvl]) * a4);
        ai.abil = "Havoc: Ruin (DoT)";
        ai.pattern = s.box_on_player(0.0, -5.0, 14.0, 12.0);
        for &d in g::BURST_HITMARKS {
            s.queue_attack(ai, 0, d, d);
        }
        ai.abil = "Havoc: Ruin (Final)";
        ai.mult = (g::BURST_FINAL[lvl] + bonus * g::BURST_BONUS[lvl]) * a4;
        s.queue_attack(ai, 0, g::BURST_HITMARK_FINAL, g::BURST_HITMARK_FINAL);
        c6_on_burst_ruin(s, ci);
        core_task(s, ci, TASK_SS_CONSUME, 0, 7);
        s.set_cd(ci, Action::Burst, 15 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Burst)
    }

    /// Seven-Phase Flash's dash: 11 frames, its stamina at the end.
    fn own_dash(s: &mut Sim, ci: usize, _: &Params) -> Option<ActInfo> {
        if !s.mod_active(ci, SKILL) {
            return None;
        }
        s.apply_dash_cd();
        s.add_task(Task::DashStam { c: ci as u8 }, g::SKILL_DASH_LENGTH);
        let (len, jump) = (g::SKILL_DASH_LENGTH, s.chars[ci].dash_jump);
        Some(ActInfo::new(Frames::Dash { len, jump }, len, jump, AnimState::Dash))
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_HOLD => g::SKILL_HOLD_FRAMES[next as usize] + arg as i32,
            _ => unreachable!(),
        }
    }

    fn on_removed(s: &mut Sim, ci: usize, id: u8, _: AnimState) {
        if id == REMOVED_EXTINCTION {
            s.delete_mod(ci, BURST_ANIM);
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_ENTER_SKILL => enter_skill_state(s, ci),
            TASK_EXIT_SKILL => exit_skill_state(s, ci, arg),
            TASK_SS_REDUCE => {
                if st(s, ci).skill_src != arg {
                    return;
                }
                // 1.4 every 12 frames, paused 0.3 s for each rift absorbed
                if st(s, ci).a1_pause_stacks == 0 {
                    add_ss(s, ci, -1.4);
                    if st(s, ci).serpents == 0.0 && s.mod_active(ci, SKILL) {
                        exit_skill_state(s, ci, arg);
                    }
                }
                core_task(s, ci, TASK_SS_REDUCE, arg, 12);
            }
            TASK_HOLD_GAIN => {
                add_ss(s, ci, 45.0);
                c2_on_skill(s, ci);
                absorb_void_rifts(s, ci);
            }
            TASK_SS_CONSUME => st_mut(s, ci).serpents = 0.0,
            TASK_PAUSE_TICK => {
                st_mut(s, ci).a1_pause_stacks -= 1;
                if st(s, ci).a1_pause_stacks == 0 {
                    s.delete_mod(ci, A1_PAUSE);
                    return;
                }
                core_task(s, ci, TASK_PAUSE_TICK, 0, 9);
            }
            // C2: 70% ATK in Seven-Phase Flash after Extinction
            TASK_C2_BUFF => s.add_mod(ci, Mod::owned(C2, ModKind::Stat, Owner::Char(ci as u8), st::ATKP, false, false, -1), 750),
            TASK_NA_HIT => {
                let (skill, c6) = (arg & 64 != 0, arg & 128 != 0);
                let (n, i) = (((arg & 63) / 4) as usize, ((arg & 63) % 4) as usize);
                let a4 = g::A4_MULT_ATTACK[(arg >> 8) as usize];
                let ai = if skill {
                    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL_ATTACK[n][i][s.talent(ci, 1)] * a4);
                    ai.abil = SKILL_NAMES[n];
                    ai.hitlag_factor = g::ATTACK_SKILL_HITLAG_FACTOR[n][i];
                    ai.hitlag_halt = g::ATTACK_SKILL_HITLAG_HALT_FRAME[n][i] * 60.0;
                    ai.ignore_infusion = true;
                    let hb = g::ATTACK_SKILL_HITBOXES[n][i];
                    ai.pattern = s.box_on_player(0.0, g::ATTACK_SKILL_OFFSETS[n][i], hb[0], hb[1]);
                    ai
                } else {
                    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][s.talent(ci, 0)]);
                    ai.abil = NORMAL_NAMES[n];
                    ai.hitlag_factor = g::ATTACK_HITLAG_FACTOR[n][i];
                    ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                    let hb = g::ATTACK_HITBOXES[n][i];
                    let off = g::ATTACK_OFFSETS[n][i];
                    ai.pattern = if n == 2 || n == 3 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
                    ai
                };
                s.queue_attack(ai, if c6 { CB_C6 } else { 0 }, 0, 0);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_ABSORB != 0 {
            absorb_void_rifts(s, ci);
        }
        // C6: a stack (the stale ones dropped first) for three hits of
        // 180%, once every 0.2 s
        if cbs & CB_C6 != 0 {
            let t = s.chars[ci].time_passed;
            let mut has = false;
            while let Some(src) = st_mut(s, ci).c6_stacks.pop() {
                if src + g::C6_DUR >= t {
                    has = true;
                    break;
                }
            }
            if !has || s.mod_active(ci, C6_ICD) {
                return;
            }
            s.add_status(ci, C6_ICD, g::C6_ICD as i32, true);
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 1.8 * a4_mult_attack(s, ci));
            ai.abil = "Havoc: Sever (Normal)";
            ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
            for i in 1..=3 {
                s.queue_attack(ai, 0, i * 3, i * 3);
            }
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C2 {
            if !s.mod_active(ci, SKILL) {
                return false;
            }
            out[st::ATKP] += 0.7;
        } else {
            out[st::ATKP] += g::C4_ATKP[a4_stacks(s, ci)];
        }
        true
    }

    /// Extinction: ten normal attacks deal more DMG, by the rifts it
    /// absorbed (gcsim's ICD on it never applies; see the module's notes).
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        let st = st(s, ci);
        if st.burst_count.get() <= 0 || ev.info.tag != AttackTag::Normal || !s.mod_active(ci, BURST_EXTINCT) {
            return;
        }
        st.burst_count.set(st.burst_count.get() - 1);
        ev.stats[st::DMGP] += g::BURST_DMG[st.burst_voids][s.talent(ci, 2)];
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { .. } => {
                if s.mod_active(ci, SKILL) {
                    let src = st(s, ci).skill_src;
                    exit_skill_state(s, ci, src);
                }
            }
            // four particles on her cryo damage, once every 15 s
            Event::EnemyDamage(ev) => {
                if ev.on_player || ev.info.actor as usize != ci || ev.info.element != Element::Cryo || s.mod_active(ci, PARTICLE_ICD) {
                    return;
                }
                s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD, false);
                s.queue_particle(4.0, Element::Cryo, s.particle_delay[ci]);
            }
            // A1: a Void Rift from a cryo reaction on the enemy, once
            // every 2.5 s
            Event::Reaction(r, ev) => {
                if ev.on_player || !matches!(r, Reaction::Frozen | Reaction::Superconduct | Reaction::StellarConduct | Reaction::SwirlCryo | Reaction::StellarSwirl | Reaction::CrystallizeCryo) {
                    return;
                }
                if s.mod_active(ci, A1_ICD) {
                    return;
                }
                s.add_status(ci, A1_ICD, 150, true);
                create_void_rift(s, ci);
            }
            // A4: a stack for each other cryo or hydro member's hit of its
            // element
            Event::EnemyHit(ev) => {
                let actor = ev.info.actor as usize;
                let el = s.chars[actor].element;
                if actor == ci || ev.info.element != el || !matches!(el, Element::Cryo | Element::Hydro) {
                    return;
                }
                s.add_status(ci, A4_STACK[actor], g::A4_DUR, true);
            }
            _ => {}
        }
    }
}
