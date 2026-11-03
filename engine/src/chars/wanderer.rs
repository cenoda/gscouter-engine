//! Wanderer, from gcsim's internal/characters/wanderer: normal and charged
//! attacks, Hanega: Song of the Wind (the Windfavored state and its
//! Kuugoryoku points, a point lost every 6 frames, 15 for a dash; its
//! normal and charged attacks, dash, jump and burst; the fall at its end),
//! the burst, the low plunge after the state ends, A1 (the elements the
//! skill absorbs), A4 (Gales of Reverie, from a dash), C1, C2, C4 and C6,
//! and .wanderer.skydweller-points.
//!
//! gcsim's quirks, kept: an action that finds the points spent ends the
//! state first and waits out the 26-frame fall (the low plunge only 7 of
//! it, and its frames not at all); while the state lasts the swap is
//! locked (Player.SwapCD); his plain jump lasts the template's 60 frames
//! for every next action; the state's points run on the core's clock,
//! while the state itself is extended by hitlag; he has no high plunge.

use crate::content::Character;
use crate::gcsimdata::chars::wanderer as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    points: i32,
    max_points: i32,
    /// A1's elements still to absorb (gcsim's a1ValidBuffs), in order.
    a1_valid: [Element; 4],
    a1_len: usize,
    a4_prob: f64,
    c6_count: i32,
    /// The attack speed and windup the current action's frames read
    /// (gcsim's frames functions fix them as the action starts).
    fr_atkspd: f64,
    fr_windup: i32,
}

const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const PLUNGE_AVAILABLE: ModKey = ModKey::new(g::PLUNGE_AVAILABLE_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const A4_PREVENT: ModKey = ModKey::new(g::A4_PREVENT);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const A1_ELECTRO: ModKey = ModKey::new(g::A1_ELECTRO_KEY);
const A1_ELECTRO_ICD: ModKey = ModKey::new(g::A1_ELECTRO_ICD_KEY);
const A1_PYRO: ModKey = ModKey::new(g::A1_PYRO_KEY);
const A1_CRYO: ModKey = ModKey::new(g::A1_CRYO_KEY);
const C1: ModKey = ModKey::new("wanderer-c1-atkspd");
const C2: ModKey = ModKey::new("wanderer-c2-burstbonus");
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const A1_ELEMENTS: [Element; 4] = [Element::Pyro, Element::Hydro, Element::Electro, Element::Cryo];

// callbacks, in the order gcsim gives them
const CB_A4: u16 = 1 << 0;
const CB_A1_ELECTRO: u16 = 1 << 1;
const CB_C6: u16 = 1 << 2;
const CB_PARTICLE: u16 = 1 << 3;

const TASK_DEPLETE: u8 = 0;
const TASK_DASH: u8 = 1;

// frames functions
const FR_NORMAL: u8 = 0;
const FR_NORMAL_E: u8 = 1;
const FR_CHARGE: u8 = 2;
const FR_CHARGE_E: u8 = 3;
const FR_BURST: u8 = 4;
const FR_SKILL_END: u8 = 5;

const ON_REMOVED_BURST_E: u8 = 0;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];
const NORMAL_E_NAMES: [&str; 3] = ["Normal 0 (Windfavored)", "Normal 1 (Windfavored)", "Normal 2 (Windfavored)"];

/// gcsim's skillEndRoutine's fall.
const FALL: i32 = 26;

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

/// gcsim's checkForSkillEnd: the state ends when its points are spent,
/// and the action waits out the fall.
fn check_skill_end(s: &mut Sim, ci: usize) -> i32 {
    if s.mod_active(ci, SKILL) && st(s, ci).points <= 0 {
        return skill_end(s, ci);
    }
    0
}

/// gcsim's skillEndRoutine.
fn skill_end(s: &mut Sim, ci: usize) -> i32 {
    s.delete_mod(ci, SKILL);
    s.player.swap_cd = FALL;
    if s.mod_active(ci, A4) {
        s.delete_mod(ci, A4);
    }
    if s.mod_active(ci, A4_PREVENT) {
        s.delete_mod(ci, A4_PREVENT);
    }
    let x = st_mut(s, ci);
    x.points = 0;
    x.a4_prob = 0.16;
    s.set_cd(ci, Action::Skill, 360);
    s.delete_mod(ci, A1_PYRO);
    s.delete_mod(ci, A1_CRYO);
    s.delete_mod(ci, A1_ELECTRO);
    if s.mod_active(ci, C1) {
        s.delete_mod(ci, C1);
    }
    s.add_status(ci, PLUNGE_AVAILABLE, FALL, true);
    FALL
}

fn skill_activate(s: &mut Sim, ci: usize) -> ActInfo {
    s.add_status(ci, SKILL, 20 * 60, true);
    s.player.swap_cd = i16::MAX as i32;
    let x = st_mut(s, ci);
    x.points = 100;
    x.max_points = 100;
    x.c6_count = 0;
    s.add_task(Task::Char { c: ci as u8, id: TASK_DEPLETE, arg: 0 }, 6);
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL[lvl]);
    ai.abil = "Hanega: Song of the Wind";
    ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
    s.queue_attack(ai, 0, g::SKILL_HITMARK, g::SKILL_HITMARK);
    if s.chars[ci].ascension >= 1 {
        let x = st_mut(s, ci);
        x.a1_valid = A1_ELEMENTS;
        x.a1_len = 4;
        a1_absorb(s, ci);
    }
    if s.chars[ci].cons >= 1 {
        s.add_stat_mod(ci, C1, st::ATKSPD, 0.1, 1200, true);
    }
    let t = &g::SKILL_FRAMES_NORMAL;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
}

/// gcsim's absorbCheckA1: two elements absorbed around him, and at C4 a
/// third at random from those left.
fn a1_absorb(s: &mut Sim, ci: usize) {
    let mut proc = false;
    for _ in 0..2 {
        let x = st(s, ci);
        let valid = x.a1_valid;
        let len = x.a1_len;
        let p = s.circle_on_player(0.0, 0.0, 5.0);
        let e = s.absorb_check_in(p, &valid[..len]);
        if e == Element::NoElement {
            continue;
        }
        proc = true;
        a1_buff(s, ci, e);
        a1_remove(s, ci, e);
    }
    if s.chars[ci].cons >= 4 && proc {
        let len = st(s, ci).a1_len as i32;
        let i = s.rng.intn(len) as usize;
        let e = st(s, ci).a1_valid[i];
        a1_buff(s, ci, e);
        a1_remove(s, ci, e);
    }
}

fn a1_remove(s: &mut Sim, ci: usize, e: Element) {
    let x = st_mut(s, ci);
    let mut n = 0;
    for i in 0..x.a1_len {
        if x.a1_valid[i] != e {
            x.a1_valid[n] = x.a1_valid[i];
            n += 1;
        }
    }
    x.a1_len = n;
}

fn a1_buff(s: &mut Sim, ci: usize, e: Element) {
    match e {
        Element::Hydro => {
            let x = st_mut(s, ci);
            x.max_points += 20;
            x.points += 20;
        }
        Element::Pyro => s.add_stat_mod(ci, A1_PYRO, st::ATKP, 0.3, 1200, true),
        Element::Cryo => s.add_stat_mod(ci, A1_CRYO, st::CR, 0.2, 1200, true),
        Element::Electro => s.add_status(ci, A1_ELECTRO, 1200, true),
        _ => unreachable!(),
    }
}

/// gcsim's a4: Gales of Reverie, if a proc waits.
fn a4(s: &mut Sim, ci: usize) -> bool {
    if !s.mod_active(ci, A4) || s.mod_active(ci, A4_PREVENT) {
        return false;
    }
    s.delete_mod(ci, A4);
    s.add_status(ci, A4_PREVENT, 20, true);
    let mult = if s.mod_active(ci, C1) { 0.6 } else { 0.35 };
    for &r in g::A4_RELEASE {
        let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_WANDERER_A4, icd::GROUP_WANDERER_A4, Element::Anemo, 25.0, mult);
        ai.abil = "Gales of Reverie";
        ai.pattern = s.circle_on_player(0.0, 0.0, 1.0);
        s.queue_attack(ai, 0, r, r + g::A4_HITMARK);
    }
    true
}

/// The callbacks of his normal and charged attacks.
fn attack_cbs(s: &Sim, ci: usize, c6: bool) -> u16 {
    let c = &s.chars[ci];
    let mut cbs = CB_PARTICLE;
    if c.ascension >= 4 {
        cbs |= CB_A4;
    }
    if c.ascension >= 1 {
        cbs |= CB_A1_ELECTRO;
    }
    if c6 && c.cons >= 6 {
        cbs |= CB_C6;
    }
    cbs
}

impl Character for Char {
    const KEY: &'static str = "wanderer";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { points: 0, max_points: 100, a1_valid: A1_ELEMENTS, a1_len: 4, a4_prob: 0.16, c6_count: 0, fr_atkspd: 0.0, fr_windup: 0 }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack => p.0.iter().find(|(k, _)| k != "travel").map(|(k, _)| format!("{name} param {k}")),
            Action::LowPlunge => p.0.iter().find(|(k, _)| k != "collision").map(|(k, _)| format!("{name} param {k}")),
            Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    /// The low plunge comes only from the fall at the state's end (gcsim's
    /// error in the plunge, after its checkForSkillEnd).
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::LowPlunge && !s.mod_active(ci, PLUNGE_AVAILABLE) && !(s.mod_active(ci, SKILL) && st(s, ci).points <= 0) {
            return Some("only plunge after skill ends".into());
        }
        crate::content::template_invalid(s, a)
    }

    fn action_stam(s: &Sim, ci: usize, _: Action) -> Option<f64> {
        s.mod_active(ci, SKILL).then_some(0.0)
    }

    fn start_delay(s: &Sim, ci: usize, key: usize) -> Option<i32> {
        (key == 2).then(|| if s.mod_active(ci, SKILL) { 12 } else { 0 })
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "skydweller-points" => Some(Ok(Num::Int(st(s, ci).points.max(0) as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let delay = check_skill_end(s, ci);
        let e = s.mod_active(ci, SKILL);
        let n = s.chars[ci].normal_counter as usize;
        let last = s.player.last.action;
        let windup = if e {
            // attackWindupE
            match last {
                Action::Dash => -3,
                Action::Charge | Action::Jump => -2,
                _ => 0,
            }
        } else {
            // attackWindupNormal
            delay
                + match last {
                    Action::Attack if n == 0 => 3,
                    Action::Charge | Action::Burst => -2,
                    _ => 0,
                }
        };
        let travel = p.get("travel").unwrap_or(5) as i32;
        let (lvl, slvl) = (s.talent(ci, 0), s.talent(ci, 1));
        let (release, mult, name, r) = if e {
            (g::ATTACK_RELEASE_E[n], g::SKILL_NA_BONUS[slvl], NORMAL_E_NAMES[n], g::ATTACK_RADIUS_E[n])
        } else {
            (g::ATTACK_RELEASE_NORMAL[n], 1.0, NORMAL_NAMES[n], 1.0)
        };
        let cbs = attack_cbs(s, ci, true);
        for (i, m) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Anemo, 25.0, mult * m[lvl]);
            ai.abil = name;
            ai.pattern = s.circle_on_player(0.0, 0.0, r);
            let rel = windup + release[i];
            s.queue_attack(ai, cbs, rel, rel + travel);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let x = st_mut(s, ci);
        x.fr_atkspd = atkspd;
        x.fr_windup = windup;
        let (table, cqa, id) = if e { (&g::ATTACK_FRAMES_E[n], g::ATTACK_CQAE[n], FR_NORMAL_E) } else { (&g::ATTACK_FRAMES_NORMAL[n], g::ATTACK_CQA_NORMAL[n], FR_NORMAL) };
        let info = ActInfo::new(Frames::CharLive { id, arg: n as i16 }, windup + table[0], windup + cqa, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let delay = check_skill_end(s, ci);
        let e = s.mod_active(ci, SKILL);
        let last = s.player.last.action;
        let lvl = s.talent(ci, 0);
        let (windup, hitmark, mult, name, r, table, id) = if e {
            // chargeWindupE
            let w = match last {
                Action::Attack => -4,
                Action::Charge | Action::Jump => -7,
                Action::Burst => -2,
                Action::Dash => -9,
                _ => 0,
            };
            (w, g::CHARGE_HITMARK_E, g::SKILL_CA_BONUS[s.talent(ci, 1)] * g::CHARGE[lvl], "Charge Attack (Windfavored)", 3.5, &g::CHARGE_FRAMES_E, FR_CHARGE_E)
        } else {
            // chargeWindupNormal
            let w = match last {
                Action::Attack => -2,
                Action::LowPlunge => -4,
                _ => 0,
            } + delay;
            (w, g::CHARGE_HITMARK_NORMAL, g::CHARGE[lvl], "Charge Attack", 2.5, &g::CHARGE_FRAMES_NORMAL, FR_CHARGE)
        };
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, mult);
        ai.abil = name;
        ai.pattern = s.circle_on_enemy(0.0, 0.0, r);
        let cbs = attack_cbs(s, ci, false);
        s.queue_attack(ai, cbs, windup + hitmark, windup + hitmark);
        let atkspd = s.stat(ci, st::ATKSPD);
        let x = st_mut(s, ci);
        x.fr_atkspd = atkspd;
        x.fr_windup = windup;
        ActInfo::new(Frames::CharLive { id, arg: 0 }, windup + table[0], windup + table[Action::Dash as usize], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if !s.mod_active(ci, SKILL) {
            return skill_activate(s, ci);
        }
        let delay = skill_end(s, ci);
        ActInfo::new(Frames::Char { id: FR_SKILL_END, arg: delay as i16 }, delay, 7, AnimState::Idle)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let delay = check_skill_end(s, ci);
        let e = s.mod_active(ci, SKILL);
        let lvl = s.talent(ci, 2);
        if e && s.chars[ci].cons >= 2 {
            let x = st(s, ci);
            let v = (((x.max_points - x.points) as f64) * 0.04).min(2.0);
            s.add_stat_mod(ci, C2, st::DMGP, v, g::BURST_FRAMES_E[0], true);
        }
        let (name, d) = if e { ("Kyougen: Five Ceremonial Plays (Windfavored)", 0) } else { ("Kyougen: Five Ceremonial Plays", delay) };
        for i in 0..5 {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST[lvl]);
            ai.abil = name;
            ai.pattern = s.circle_on_enemy(0.0, 0.0, 5.0);
            s.queue_attack(ai, 0, d + g::BURST_SNAPSHOT_DELAY, d + g::BURST_HITMARK + i * g::BURST_HITMARK_DELAY);
        }
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 5);
        if e {
            s.player.swap_cd = FALL;
            st_mut(s, ci).points = 0;
            let t = &g::BURST_FRAMES_E;
            let mut info = ActInfo::new(Frames::Table(t), t[0], t[Action::Walk as usize], AnimState::Burst);
            info.on_removed = OnRemoved::Char(ON_REMOVED_BURST_E);
            return info;
        }
        let t = &g::BURST_FRAMES_NORMAL;
        ActInfo::new(Frames::Char { id: FR_BURST, arg: delay as i16 }, delay + t[0], delay + t[Action::Attack as usize], AnimState::Burst)
    }

    fn on_removed(s: &mut Sim, ci: usize, _: u8, next: AnimState) {
        if next == AnimState::Swap {
            check_skill_end(s, ci);
        }
    }

    fn own_dash(s: &mut Sim, ci: usize, _: &Params) -> Option<ActInfo> {
        let delay = check_skill_end(s, ci);
        if s.mod_active(ci, SKILL) {
            // WindfavoredDash
            let t = &g::DASH_FRAMES_E;
            let info = ActInfo::new(Frames::Table(t), t[0], t[Action::Skill as usize], AnimState::Dash);
            if !a4(s, ci) {
                st_mut(s, ci).points -= 15;
            }
            return Some(info);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_DASH, arg: 0 }, delay);
        let len = s.chars[ci].dash_len + delay;
        Some(ActInfo::new(Frames::Const(len), len, len, AnimState::Dash))
    }

    fn own_jump(s: &mut Sim, ci: usize, _: &Params) -> Option<ActInfo> {
        let delay = check_skill_end(s, ci);
        if s.mod_active(ci, SKILL) {
            // WindfavoredJump
            let t = &g::E_JUMP_FRAMES;
            return Some(ActInfo::new(Frames::Table(t), t[Action::Jump as usize], t[Action::Skill as usize], AnimState::Jump));
        }
        let info = s.char_jump(ci);
        let f = delay + info.anim_len;
        Some(ActInfo::new(Frames::Const(f), f, f, AnimState::Jump))
    }

    fn own_walk(s: &mut Sim, ci: usize, p: &Params) -> Option<ActInfo> {
        let delay = check_skill_end(s, ci);
        let info = s.char_walk(p);
        let f = delay + info.anim_len;
        Some(ActInfo::new(Frames::Const(f), f, f, AnimState::Walk))
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let mut delay = check_skill_end(s, ci);
        s.delete_mod(ci, PLUNGE_AVAILABLE);
        if delay > 0 {
            delay = 7;
        }
        let lvl = s.talent(ci, 0);
        if p.get("collision").unwrap_or(0) > 0 {
            let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 0.0, g::PLUNGE[lvl]);
            ai.abil = "Plunge Collision";
            ai.pattern = s.circle_on_player(0.0, 0.0, 1.5);
            let d = g::LOW_PLUNGE_COLLISION_HITMARK + delay;
            s.queue_attack(ai, 0, d, d);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::LOW_PLUNGE[lvl]);
        ai.abil = "Low Plunge Attack";
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
        let d = delay + g::LOW_PLUNGE_HITMARK;
        s.queue_attack(ai, 0, d, d);
        let t = &g::LOW_PLUNGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::LOW_PLUNGE_HITMARK, AnimState::Plunge)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            // depleteSkydwellerPoints
            TASK_DEPLETE => {
                if s.mod_active(ci, SKILL) {
                    st_mut(s, ci).points -= 1;
                    s.add_task(Task::Char { c: ci as u8, id: TASK_DEPLETE, arg: 0 }, 6);
                }
            }
            // the plain dash, after the fall
            TASK_DASH => {
                s.apply_dash_cd();
                let len = s.chars[ci].dash_len;
                s.add_task(Task::DashStam { c: ci as u8 }, len - 1);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_A4 != 0 && s.mod_active(ci, SKILL) && !s.mod_active(ci, A4) && !s.mod_active(ci, A4_ICD) {
            s.add_status(ci, A4_ICD, 6, true);
            let r = s.rng.float64();
            if r > st(s, ci).a4_prob {
                st_mut(s, ci).a4_prob += 0.12;
            } else {
                st_mut(s, ci).a4_prob = 0.16;
                s.add_status(ci, A4, 20 * 60, true);
                if s.current_state() == AnimState::Dash {
                    a4(s, ci);
                }
            }
        }
        if cbs & CB_A1_ELECTRO != 0 && s.mod_active(ci, A1_ELECTRO) && !s.mod_active(ci, A1_ELECTRO_ICD) {
            s.add_status(ci, A1_ELECTRO_ICD, 12, true);
            s.add_energy(ci, 0.8);
        }
        if cbs & CB_C6 != 0 && s.mod_active(ci, SKILL) && st(s, ci).points > 0 {
            if st(s, ci).c6_count < 5 && !s.mod_active(ci, C6_ICD) && st(s, ci).points < 40 {
                s.add_status(ci, C6_ICD, 12, true);
                let x = st_mut(s, ci);
                x.c6_count += 1;
                x.points += 4;
            }
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_WANDERER_C6, icd::GROUP_WANDERER_C6, Element::Anemo, 25.0, ev.info.mult * 0.4);
            ai.abil = "Shugen: The Curtains’ Melancholic Sway";
            ai.pattern = s.circle_on_enemy(0.0, 0.0, 2.0);
            s.queue_attack(ai, 0, 8, 8);
        }
        if cbs & CB_PARTICLE != 0 && s.mod_active(ci, SKILL) && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 2 * 60, true);
            s.queue_particle(1.0, Element::Anemo, s.particle_delay[ci]);
        }
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_BURST => arg as i32 + g::BURST_FRAMES_NORMAL[next as usize],
            FR_SKILL_END => {
                if next == Action::LowPlunge {
                    7
                } else {
                    arg as i32
                }
            }
            _ => unreachable!(),
        }
    }

    fn frames_live(s: &Sim, ci: usize, id: u8, arg: i16, next: Action) -> i32 {
        let x = st(s, ci);
        let table = match id {
            FR_NORMAL => &g::ATTACK_FRAMES_NORMAL[arg as usize],
            FR_NORMAL_E => &g::ATTACK_FRAMES_E[arg as usize],
            FR_CHARGE => &g::CHARGE_FRAMES_NORMAL,
            FR_CHARGE_E => &g::CHARGE_FRAMES_E,
            _ => unreachable!(),
        };
        x.fr_windup + atk_spd_adjust(table[next as usize], x.fr_atkspd)
    }
}
