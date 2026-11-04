//! Jahoda, from gcsim's internal/characters/jahoda: normal attacks and
//! aimed shots; the skill's Shadow Pursuit, its Treasure Flask filling from
//! the enemy's aura in its area (filled or unfilled on a recast, when full
//! or at the end), and, in Ascendant Gleam, the Meowballs of a filled
//! flask; the burst's hit, heals and, in Ascendant Gleam, its robots that
//! take the enemy's element; A1 (by the party's most common element), A4
//! and every constellation.
//!
//! As gcsim's, the robots' damage is A1's DMG multiplier, which only a
//! pyro A1 (or C2) sets: other robots hit for 0 and still apply their
//! element.
//!
//! Its only library team (flins-ineffa-columbina-jahoda) needs Flins, so no
//! comparison reaches it yet: it was checked by hand-written teams with
//! Flins replaced by Xingqiu (C0, C6) or Bennett (C2, C6: pyro A1).

use crate::content::{Character, Owner};
use crate::gcsimdata::chars::jahoda as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    flask_check: Pattern,
    flask_absorb: Element,
    flask_gauge: i32,
    pursuit_duration: i32,
    skill_src: i32,
    /// The frame of the skill cast whose frames are asked for (the src its
    /// gcsim frames function holds).
    action_src: i32,
    meowball_src: i32,
    meowball_travel: i32,
    burst_check: Pattern,
    burst_src: i32,
    a1_highest: Element,
    a1_dmg_mult: f64,
    a1_heal_mult: f64,
    robot_count: i32,
    robot_interval: f64,
    c2_next: Element,
    moonsign_level: i32,
    /// Each robot's attack, made when it absorbed its element.
    robot_ai: [Option<AttackInfo>; 8],
}

const SHADOW_PURSUIT: ModKey = ModKey::new(g::SHADOW_PURSUIT_KEY);
const MEOWBALL: ModKey = ModKey::new(g::MEOWBALL_KEY);
const MEOWBALL_ENERGY_ICD: ModKey = ModKey::new(g::MEOWBALL_FLAT_ENERGY_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const A4: ModKey = ModKey::new("jahoda-a4");
const C6: ModKey = ModKey::new(g::C6_KEY);

const ELE_PRIORITY: [Element; 4] = [Element::Pyro, Element::Hydro, Element::Electro, Element::Cryo];

const CB_PARTICLE: u16 = 1 << 0;
const CB_MEOW_ENERGY: u16 = 1 << 1;
const CB_C1: u16 = 1 << 2;

const TASK_WINDUP: u8 = 0;
const TASK_FILL: u8 = 1;
const TASK_DRAIN: u8 = 2;
const TASK_MEOWBALL: u8 = 3;
const TASK_CONSUME: u8 = 4;
const TASK_HEAL_START: u8 = 5;
const TASK_HEAL: u8 = 6;
const TASK_ABSORB: u8 = 7;
const TASK_ROBOT: u8 = 8;

const FR_SKILL: u8 = 0;

impl Character for Char {
    const KEY: &'static str = "jahoda";
    const MOONSIGN: i32 = 1;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            flask_check: Pattern::Unknown,
            flask_absorb: Element::NoElement,
            flask_gauge: 0,
            pursuit_duration: 0,
            skill_src: 0,
            action_src: 0,
            meowball_src: 0,
            meowball_travel: 0,
            burst_check: Pattern::Unknown,
            burst_src: 0,
            a1_highest: Element::NoElement,
            a1_dmg_mult: 0.0,
            a1_heal_mult: 0.0,
            robot_count: 0,
            robot_interval: 0.0,
            c2_next: Element::NoElement,
            moonsign_level: 0,
            robot_ai: [None; 8],
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let level = s.chars.iter().map(|c| c.moonsign).sum::<i32>();
        let asc = s.chars[ci].ascension;
        let cons = s.chars[ci].cons;
        let count = |e: Element| s.chars.iter().filter(|c| c.element == e).count();
        let counts: Vec<usize> = ELE_PRIORITY.iter().map(|&e| count(e)).collect();
        let st = s.st_mut::<State>(ci);
        st.moonsign_level = level;
        st.a1_heal_mult = 1.0;
        // a1Init: the most common of pyro, hydro, electro and cryo, the
        // first in priority on a tie
        if asc >= 1 {
            let mut highest = 0;
            for (i, &e) in ELE_PRIORITY.iter().enumerate() {
                if counts[i] > highest {
                    highest = counts[i];
                    st.a1_highest = e;
                }
            }
            if highest == 0 {
                st.a1_highest = Element::NoElement;
            }
            // c2Init: the next most common
            if cons >= 2 && level >= 2 {
                let mut second = 0;
                for (i, &e) in ELE_PRIORITY.iter().enumerate() {
                    if e == st.a1_highest {
                        continue;
                    }
                    if counts[i] > second {
                        second = counts[i];
                        st.c2_next = e;
                    }
                }
                if second == 0 {
                    st.c2_next = Element::NoElement;
                }
            }
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let allowed: &[&str] = match a {
            Action::Attack | Action::Skill => &["travel"],
            Action::Aim => &["hold", "travel", "weakspot"],
            // gcsim validates no params for it and reads none
            Action::Burst => return None,
            _ => return Some(name.into()),
        };
        if let Some((k, _)) = p.0.iter().find(|(k, _)| !allowed.contains(&k.as_str())) {
            return Some(format!("{name} param {k}"));
        }
        if a == Action::Aim && !matches!(p.get("hold"), None | Some(0) | Some(1)) {
            return Some("aim[hold]".into());
        }
        None
    }

    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::Burst && s.mod_active(ci, SHADOW_PURSUIT) {
            return Some("burst called in skill state".into());
        }
        crate::content::template_invalid(s, a)
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
            let h = g::ATTACK_HITMARKS[n][i];
            s.queue_attack(ai, 0, h, h + travel);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::FULLAIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        ai.deployable = true;
        // its hitlag applies on weak spot hits only (HitlagOnHeadshotOnly)
        if p.get("weakspot") == Some(1) {
            ai.weak_point = true;
            ai.hitlag_halt = 0.12 * 60.0;
            ai.hitlag_factor = 0.01;
        }
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let h = g::AIMED_HITMARKS[hold];
        s.queue_attack(ai, 0, h, h + travel);
        // InitAbilSlice(hitmark): every next action after the hitmark
        let t = g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Const(t[0]), t[0], h, AnimState::Aim)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.mod_active(ci, SHADOW_PURSUIT) {
            let src = s.st::<State>(ci).skill_src;
            s.add_task(Task::Char { c: ci as u8, id: TASK_DRAIN, arg: src }, 0);
            let t = &g::SKILL_CANCEL_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill);
        }
        let travel = p.get("travel").map_or(13, |t| t as i32);
        let f = s.f;
        let check = s.circle_on_player(0.0, 0.0, 5.0);
        let st = s.st_mut::<State>(ci);
        st.meowball_travel = travel;
        st.skill_src = f;
        st.action_src = f;
        st.meowball_src = -1;
        st.flask_absorb = Element::NoElement;
        st.flask_gauge = 0;
        st.flask_check = check;
        st.pursuit_duration = g::SHADOW_PURSUIT_MAX_DURATION;
        s.delete_mod(ci, MEOWBALL);
        s.add_task(Task::Char { c: ci as u8, id: TASK_WINDUP, arg: f }, g::SKILL_WINDUP);
        let t = &g::SKILL_CANCEL_FRAMES;
        let len = g::SKILL_WINDUP + g::SHADOW_PURSUIT_MAX_DURATION + t[Action::Jump as usize];
        ActInfo::new(Frames::CharLive { id: FR_SKILL, arg: 0 }, len, t[Action::Dash as usize], AnimState::Skill)
    }

    /// The skill's frames: they grow with Shadow Pursuit while it lasts
    /// (but E -> E), and are fixed by its length once it ended.
    fn frames_live(s: &Sim, ci: usize, _: u8, _: i16, next: Action) -> i32 {
        let st = s.st::<State>(ci);
        let t = &g::SKILL_CANCEL_FRAMES;
        if st.action_src == st.skill_src {
            if next == Action::Skill {
                return g::SKILL_FRAMES[Action::Skill as usize];
            }
            return (s.f - st.action_src) + t[next as usize];
        }
        st.pursuit_duration + t[next as usize]
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let f = s.f;
        let check = s.circle_on_player(0.0, 0.0, 5.0);
        {
            let st = s.st_mut::<State>(ci);
            st.burst_src = f;
            st.burst_check = check;
            st.robot_interval = 140.0;
        }
        s.add_status(ci, BURST, g::BURST_DURATION, false);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST[lvl]);
        ai.abil = "Hidden Aces: Seven Tools of the Hunter";
        ai.pattern = s.circle_on_player(0.0, 1.0, 5.0);
        s.queue_attack(ai, 0, 0, g::BURST_HITMARK);
        s.st_mut::<State>(ci).robot_count = 2;
        // a1, and c2 with it
        if s.chars[ci].ascension >= 1 {
            let e = s.st::<State>(ci).a1_highest;
            apply_a1(s, ci, e);
            if s.chars[ci].cons >= 2 && s.st::<State>(ci).moonsign_level >= 2 {
                let e = s.st::<State>(ci).c2_next;
                apply_a1(s, ci, e);
            }
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_HEAL_START, arg: f }, g::FIRS_HEAL_TICK_DELAY);
        if s.st::<State>(ci).moonsign_level >= 2 {
            let n = s.st::<State>(ci).robot_count;
            for robot in 0..n {
                s.add_task(Task::Char { c: ci as u8, id: TASK_ABSORB, arg: f * 8 + robot }, g::FIRST_ROBOT_ABSORB_DELAY + g::ROBOT_ABSORPTION_INTERVAL * robot);
            }
        }
        s.set_cd_with_delay(ci, Action::Burst, g::BURST_CD, 1);
        s.consume_energy(ci, 13);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_WINDUP => {
                s.add_status(ci, SHADOW_PURSUIT, g::SHADOW_PURSUIT_MAX_DURATION, false);
                let src = s.st::<State>(ci).skill_src;
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_FILL, arg: src }, g::FIRST_FLASK_FILL_DELAY);
                s.add_task(Task::Char { c: ci as u8, id: TASK_DRAIN, arg: src }, g::SHADOW_PURSUIT_MAX_DURATION);
            }
            TASK_FILL => fill_flask(s, ci, arg),
            TASK_DRAIN => drain_flask(s, ci, arg),
            TASK_MEOWBALL => meowball_tick(s, ci, arg),
            TASK_CONSUME => {
                let st = s.st_mut::<State>(ci);
                if arg == st.meowball_src {
                    st.flask_gauge = (st.flask_gauge - 10).max(0);
                }
            }
            TASK_HEAL_START => {
                let mut i = 0;
                while i < g::BURST_DURATION - g::FIRS_HEAL_TICK_DELAY {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_HEAL, arg }, i);
                    i += g::HEAL_INTERVAL;
                }
            }
            TASK_HEAL => heal_tick(s, ci, arg),
            TASK_ABSORB => absorb_check(s, ci, arg / 8, arg % 8),
            TASK_ROBOT => robot_tick(s, ci, arg / 8, arg % 8),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 30, false);
            s.queue_particle(4.0, Element::Anemo, s.particle_delay[ci]);
        }
        if cbs & CB_MEOW_ENERGY != 0 && !s.mod_active(ci, MEOWBALL_ENERGY_ICD) {
            s.add_energy(ci, 2.0);
            s.add_status(ci, MEOWBALL_ENERGY_ICD, 210, true);
        }
        if cbs & CB_C1 != 0 && s.chars[ci].cons >= 1 {
            // half the Meowballs bounce, onto the only enemy
            if s.rng.float64() < 0.5 {
                return;
            }
            let lvl = s.talent(ci, 1);
            let st = s.st::<State>(ci);
            let (ele, travel) = (st.flask_absorb, st.meowball_travel);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_JAHODA_CONS, icd::GROUP_JAHODA_CONS, ele, 25.0, g::MEOWBALL[lvl]);
            ai.abil = "Meowball (C1)";
            ai.pattern = s.circle_on_target(4.0);
            s.queue_attack(ai, 0, 0, travel + g::C1_BOUNCE_HITMARK);
        }
    }

    fn mod_amount(_: &Sim, _: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C6 {
            out[st::CR] += 0.05;
            out[st::CD] += 0.40;
        }
        true
    }

}

fn apply_a1(s: &mut Sim, ci: usize, e: Element) {
    let st = s.st_mut::<State>(ci);
    match e {
        Element::Pyro => st.a1_dmg_mult = 1.3,
        Element::Hydro => st.a1_heal_mult = 1.2,
        Element::Electro => st.robot_count += 1,
        Element::Cryo => st.robot_interval *= 0.9,
        _ => {}
    }
}

/// enemyAuraInArea: the first element in priority on the enemy, if the
/// area reaches it.
fn aura_in_area(s: &Sim, area: Pattern, priority: &[Element]) -> Element {
    if !s.enemy_alive || s.hits_enemy(area) != Some(true) {
        return Element::NoElement;
    }
    for &e in priority {
        if s.enemy_aura_contains(e) {
            return e;
        }
    }
    Element::NoElement
}

fn fill_flask(s: &mut Sim, ci: usize, src: i32) {
    if src != s.st::<State>(ci).skill_src {
        return;
    }
    let st = s.st::<State>(ci);
    let absorbed = st.flask_absorb;
    let mut priority = [Element::NoElement; 5];
    let mut n = 0;
    if absorbed != Element::NoElement {
        priority[0] = absorbed;
        n = 1;
    }
    for e in ELE_PRIORITY {
        if e != absorbed {
            priority[n] = e;
            n += 1;
        }
    }
    let obj = aura_in_area(s, st.flask_check, &priority[..n]);
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    st.pursuit_duration = f - st.skill_src;
    if obj != Element::NoElement {
        let mut amount = g::FLASK_FILL_WEAK_VALUE;
        if st.flask_absorb == Element::NoElement {
            st.flask_absorb = obj;
            amount = g::FLASK_FILL_VALUE;
        } else if obj == st.flask_absorb {
            amount = g::FLASK_FILL_VALUE;
        }
        // fillFlaskGauge
        st.flask_gauge = (st.flask_gauge + amount).min(g::FLASK_GAUGE_MAX);
        if st.flask_gauge >= g::FLASK_GAUGE_MAX {
            let src = st.skill_src;
            s.add_task(Task::Char { c: ci as u8, id: TASK_DRAIN, arg: src }, g::DRAIN_FLASK_HITMARK);
        }
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_FILL, arg: src }, g::FLASK_FILL_INTERVAL);
}

fn drain_flask(s: &mut Sim, ci: usize, src: i32) {
    if src != s.st::<State>(ci).skill_src {
        return;
    }
    let f = s.f;
    s.st_mut::<State>(ci).pursuit_duration = f - src;
    // cancelPursuit
    if s.mod_active(ci, SHADOW_PURSUIT) {
        s.set_cd(ci, Action::Skill, g::SKILL_CD + g::SKILL_WINDUP);
        s.delete_mod(ci, SHADOW_PURSUIT);
        s.st_mut::<State>(ci).skill_src = -1;
    }
    let lvl = s.talent(ci, 1);
    let pattern = s.circle_on_player(0.0, 2.5, 5.0);
    if s.st::<State>(ci).flask_gauge >= g::FLASK_GAUGE_MAX {
        let st = s.st_mut::<State>(ci);
        st.flask_gauge = g::FLASK_GAUGE_MAX;
        st.meowball_src = f;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::FILLED_FLASK[lvl]);
        ai.abil = "Filled Treasure Flask";
        ai.pattern = pattern;
        s.queue_attack(ai, CB_PARTICLE, 0, g::FILL_HITMARK);
        if s.st::<State>(ci).moonsign_level >= 2 {
            c6(s, ci);
            s.add_task(Task::Char { c: ci as u8, id: TASK_MEOWBALL, arg: f }, g::FIRST_MEOWBALL_FIRST_HITMARK);
            let ticks = g::FLASK_GAUGE_MAX / 10;
            s.add_status(ci, MEOWBALL, g::FIRST_MEOWBALL_FIRST_HITMARK + (ticks - 1) * g::MEOWBALL_HITMARK_INTERVAL, false);
        }
    } else {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::UNFILLED_FLASK[lvl]);
        ai.abil = "Unfilled Treasure Flask";
        ai.pattern = pattern;
        s.queue_attack(ai, CB_PARTICLE, 0, g::UNFILL_HITMARK);
    }
}

fn meowball_tick(s: &mut Sim, ci: usize, src: i32) {
    if src != s.st::<State>(ci).meowball_src || !s.mod_active(ci, MEOWBALL) {
        return;
    }
    let st = s.st_mut::<State>(ci);
    if st.flask_gauge < 0 {
        st.flask_gauge = 0;
    }
    if st.flask_gauge <= 0 {
        return;
    }
    let (ele, travel) = (st.flask_absorb, st.meowball_travel);
    s.add_task(Task::Char { c: ci as u8, id: TASK_CONSUME, arg: src }, 0);
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, ele, 25.0, g::MEOWBALL[lvl]);
    ai.abil = "Meowball";
    ai.pattern = s.circle_on_target(4.0);
    s.queue_attack(ai, CB_MEOW_ENERGY | CB_C1, 0, travel);
    let next = s.st::<State>(ci).meowball_src;
    s.add_task(Task::Char { c: ci as u8, id: TASK_MEOWBALL, arg: next }, g::MEOWBALL_HITMARK_INTERVAL);
}

fn heal_tick(s: &mut Sim, ci: usize, src: i32) {
    if src != s.st::<State>(ci).burst_src {
        return;
    }
    let lvl = s.talent(ci, 2);
    let mult = s.st::<State>(ci).a1_heal_mult;
    let active = s.player.active as usize;
    if s.chars[active].hp_ratio > 0.7 {
        // a4: EM for the active character
        if s.chars[ci].ascension >= 4 {
            s.add_stat_mod(active, A4, st::EM, 100.0, 6 * 60, true);
        }
        // the additional heal, on the member with the lowest HP
        let mut low = None;
        let mut low_pct = 2.0;
        for i in 0..s.chars.len() {
            if s.current_hp(i) <= 0.0 {
                continue;
            }
            let r = s.chars[i].hp_ratio;
            if r < low_pct {
                low_pct = r;
                low = Some(i);
            }
        }
        if let Some(low) = low {
            let amt = (g::BURST_ADDITIONAL_HEAL_FLAT[lvl] + g::BURST_ADDITIONAL_HEAL_PP[lvl] * s.total_atk(ci)) * mult;
            let bonus = s.stat(ci, st::HEAL);
            s.heal(ci, low, amt, bonus);
        }
    }
    let amt = (g::BURST_HEAL_FLAT[lvl] + g::BURST_HEAL_PP[lvl] * s.total_atk(ci)) * mult;
    let bonus = s.stat(ci, st::HEAL);
    let active = s.player.active as usize;
    s.heal(ci, active, amt, bonus);
}

fn absorb_check(s: &mut Sim, ci: usize, src: i32, robot: i32) {
    if src != s.st::<State>(ci).burst_src || !s.mod_active(ci, BURST) {
        return;
    }
    let area = s.st::<State>(ci).burst_check;
    let ele = aura_in_area(s, area, &ELE_PRIORITY);
    if ele == Element::NoElement {
        let d = (s.st::<State>(ci).robot_interval / 3.0) as i32;
        s.add_task(Task::Char { c: ci as u8, id: TASK_ABSORB, arg: src * 8 + robot }, d);
        return;
    }
    let lvl = s.talent(ci, 2);
    let tag = match ele {
        Element::Pyro => icd::TAG_ELEMENTAL_BURST_PYRO,
        Element::Hydro => icd::TAG_ELEMENTAL_BURST_HYDRO,
        Element::Electro => icd::TAG_ELEMENTAL_BURST_ELECTRO,
        Element::Cryo => icd::TAG_ELEMENTAL_BURST_CRYO,
        _ => icd::TAG_ELEMENTAL_BURST,
    };
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, tag, icd::GROUP_JAHODA_BURST, ele, 25.0, 0.0);
    ai.abil = "Purrsonal Coordinated Assistance Robot DMG";
    ai.flat_dmg = g::BURST_SKILL[lvl] * s.total_atk(ci) * s.st::<State>(ci).a1_dmg_mult;
    s.st_mut::<State>(ci).robot_ai[robot as usize] = Some(ai);
    // c4
    if s.chars[ci].cons >= 4 {
        s.add_energy(ci, 4.0);
    }
    s.add_task(Task::Char { c: ci as u8, id: TASK_ROBOT, arg: src * 8 + robot }, g::FIRST_ROBOT_HITMARK_DELAY);
}

fn robot_tick(s: &mut Sim, ci: usize, src: i32, robot: i32) {
    if src != s.st::<State>(ci).burst_src || !s.mod_active(ci, BURST) {
        return;
    }
    let Some(mut ai) = s.st::<State>(ci).robot_ai[robot as usize] else { return };
    // the 3 closest enemies: the only one, while it lives
    if s.enemy_alive {
        ai.pattern = s.circle_on_target(1.2);
        s.queue_attack(ai, 0, 0, 0);
    }
    let d = s.st::<State>(ci).robot_interval as i32;
    s.add_task(Task::Char { c: ci as u8, id: TASK_ROBOT, arg: src * 8 + robot }, d);
}

/// C6: CRIT for the party's Moonsign members.
fn c6(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 6 {
        return;
    }
    for i in 0..s.chars.len() {
        if s.chars[i].moonsign < 1 {
            continue;
        }
        let m = Mod::owned(C6, ModKind::Stat, Owner::Char(ci as u8), st::NONE, true, true, -1);
        s.add_mod(i, m, 20 * 60);
    }
}
