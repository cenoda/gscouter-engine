//! Iansan, from gcsim's internal/characters/iansan: normal attacks,
//! charged attack and Swift Stormflight (the charged attack in Nightsoul's
//! Blessing or after the skill, which also turns her next normal attack
//! into it), Thunderbolt Rush (the skill and its timed Nightsoul's
//! Blessing), The Three Principles of Power (the burst, its ATK bonus to
//! the active character and the Nightsoul points it restores from the
//! movement the rotation reports), the dash's and jump's points, A1, A4
//! and every constellation.
//!
//! Her Nightsoul's Blessing is the general one (`crate::nightsoul`): 54
//! points at most, draining 0.6 every 6 frames on her own queue, ending
//! at 0.2 points or after 16 s.

use crate::content::{Character, Event};
use crate::gamedata::chars::iansan as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// gcsim's nightsoulSrc and burstSrc.
    nightsoul_src: i32,
    burst_src: i32,
    particle_generated: bool,
    /// The burst's ATK bonus to the active character (burstBuff[ATK]).
    burst_atk: f64,
    /// The movement counted since the last restore (burstRestoreNS).
    burst_restore_ns: f64,
    points_overflow: f64,
    a1_increase: bool,
    c1_points: f64,
    c4_generated: bool,
    c4_stacks: i32,
}

const FAST_SKILL: ModKey = ModKey::new(g::FAST_SKILL);
const BURST: ModKey = ModKey::new(g::BURST_STATUS);
const BURST_BUFF: ModKey = ModKey::new(g::BURST_BUFF_STATUS);
const A1: ModKey = ModKey::new(g::A1_KEY);
const A1_ICD: ModKey = ModKey::new(g::A1_ICD_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_A1: u16 = 1 << 1;

/// exitNightsoul, as the blessing's timer callback.
const TASK_EXIT: u8 = 0;
/// enterNightsoul(MaxPoints), 3 frames after the skill.
const TASK_ENTER: u8 = 1;
/// nightsoulPointReduceTask, with its src.
const TASK_REDUCE: u8 = 2;
/// The burst's points, 40 frames after it.
const TASK_BURST_POINTS: u8 = 3;
/// restorePointsTask, with its src.
const TASK_RESTORE: u8 = 4;
/// updateATKBuff, with its src.
const TASK_ATK: u8 = 5;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];

impl Character for Char {
    const KEY: &'static str = "iansan";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { nightsoul_src: 0, burst_src: 0, particle_generated: false, burst_atk: 0.0, burst_restore_ns: 0.0, points_overflow: 0.0, a1_increase: false, c1_points: 0.0, c4_generated: false, c4_stacks: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].ns.max_points = 54.0;
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // burstInit: the ATK bonus on every member, while the burst lasts
        // and the member is active
        for i in 0..s.chars.len() {
            s.add_mod(i, Mod::owned(BURST_BUFF, ModKind::Stat, o, st::NONE, true, true, -1), -1);
        }
        s.subscribe(EventKind::Movement, o);
        // a1Init
        if asc >= 1 {
            s.subscribe(EventKind::NightsoulGenerate, o);
            s.subscribe(EventKind::NightsoulConsume, o);
        }
        // a4Init
        if asc >= 4 {
            s.subscribe(EventKind::NightsoulBurst, o);
        }
        // c2Init: the others' ATK +30% while A1's bonus lasts
        if cons >= 2 && asc >= 1 {
            for i in 0..s.chars.len() {
                if i != ci {
                    s.add_mod(i, Mod::owned(C2, ModKind::Stat, o, st::ATKP, false, false, -1), -1);
                }
            }
        }
        // c4Init
        if cons >= 4 {
            s.subscribe(EventKind::Burst, o);
        }
    }

    /// gcsim reads no params of hers.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    /// After the skill, a charged attack needs no normal attack before it.
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::Charge && s.mod_active(ci, FAST_SKILL) {
            return None;
        }
        crate::content::template_invalid(s, a)
    }

    /// The charged attack after the skill costs no stamina.
    fn action_stam(s: &Sim, ci: usize, a: Action) -> Option<f64> {
        (a == Action::Charge && s.mod_active(ci, FAST_SKILL)).then_some(0.0)
    }

    /// The dash uses 10 points in the blessing, a jump out of a dash 6.
    fn template_action(s: &mut Sim, ci: usize, a: Action) {
        match a {
            Action::Dash if s.ns_has_blessing(ci) => reduce_points(s, ci, 10.0),
            Action::Jump if s.ns_has_blessing(ci) && s.current_state() == AnimState::Dash => reduce_points(s, ci, 6.0),
            _ => {}
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.mod_active(ci, FAST_SKILL) {
            return Self::charge(s, ci, p);
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = g::ATTACK_DEF_HALT[n];
        ai.pattern = s.fan_on_player(0.0, g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n][0], g::ATTACK_FAN_ANGLES[n]);
        if n == 2 {
            ai.poise = 59.896;
            ai.blunt = true;
            ai.pattern = s.box_on_player(0.0, g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n][0], g::ATTACK_HITBOXES[n][1]);
        }
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    /// Swift Stormflight in the blessing or after the skill, otherwise the
    /// plain charged attack.
    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.ns_has_blessing(ci) || s.mod_active(ci, FAST_SKILL) {
            s.delete_mod(ci, FAST_SKILL);
            return swift(s, ci);
        }
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charged Attack";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = s.circle_on_target(0.8);
        let h = g::CHARGE_HITMARK;
        s.queue_attack(ai, 0, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Charge)
    }

    /// Thunderbolt Rush: the hit, Swift Stormflight ready, and her
    /// Nightsoul's Blessing 3 frames later.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL[lvl]);
        ai.abil = "Thunderbolt Rush";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = s.circle_on_target(0.8);
        let h = g::SKILL_HITMARK;
        s.queue_attack(ai, CB_PARTICLE, h, h);
        s.add_status(ci, FAST_SKILL, 5 * 60, true);
        s.add_task(Task::Char { c: ci as u8, id: TASK_ENTER, arg: 0 }, 3);
        s.st_mut::<State>(ci).particle_generated = false;
        s.set_cd(ci, Action::Skill, 16 * 60);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    /// The Three Principles of Power.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::BURST[lvl]);
        ai.abil = "The Three Principles of Power";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.poise = 180.0;
        ai.blunt = true;
        ai.hitlag_halt = 0.02 * 60.0;
        ai.hitlag_factor = 0.05;
        ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
        let h = g::BURST_HITMARK;
        s.queue_attack(ai, 0, h, h);
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_POINTS, arg: 0 }, 40);
        // c2OnBurst
        if s.chars[ci].cons >= 2 {
            a1_add_buff(s, ci);
        }
        // c4OnBurst
        if s.chars[ci].cons >= 4 {
            let st = s.st_mut::<State>(ci);
            st.c4_generated = false;
            st.c4_stacks = 0;
        }
        let dur = 12 * 60 + if s.chars[ci].cons >= 6 { 3 * 60 } else { 0 };
        s.add_status(ci, BURST, dur, true);
        s.set_cd(ci, Action::Burst, 18 * 60);
        s.consume_energy(ci, 6);
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        st.burst_src = f;
        st.burst_restore_ns = 0.0;
        update_atk_buff(s, ci, f);
        restore_points_task(s, ci, f);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_EXIT => exit_nightsoul(s, ci),
            TASK_ENTER => enter_nightsoul(s, ci, 54.0),
            TASK_REDUCE => {
                if s.st::<State>(ci).nightsoul_src != arg {
                    return;
                }
                reduce_points(s, ci, 0.6);
                if s.ns_points(ci) <= 0.2 {
                    return;
                }
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_REDUCE, arg }, 6);
            }
            TASK_BURST_POINTS => {
                if !s.ns_has_blessing(ci) {
                    enter_nightsoul(s, ci, 15.0);
                } else {
                    s.ns_generate(ci, 15.0);
                }
            }
            TASK_RESTORE => restore_points(s, ci, arg),
            TASK_ATK => update_atk_buff(s, ci, arg),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.st::<State>(ci).particle_generated {
            s.st_mut::<State>(ci).particle_generated = true;
            s.queue_particle(4.0, Element::Electro, s.particle_delay[ci]);
        }
        if cbs & CB_A1 != 0 {
            a1_add_buff(s, ci);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == BURST_BUFF {
            if !s.mod_active(ci, BURST) || s.player.active as usize != on {
                return false;
            }
            out[st::ATK] += s.st::<State>(ci).burst_atk;
            true
        } else if m.key == C2 {
            if !s.mod_active(ci, A1) {
                return false;
            }
            out[st::ATKP] += 0.3;
            true
        } else {
            false
        }
    }

    /// C6's DMG +25%.
    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C6 {
            ev.stats[st::DMGP] += 0.25;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // burstMovementRestore
            Event::Movement(d) => {
                if !s.mod_active(ci, BURST) || s.player.active as usize == ci {
                    return;
                }
                let st = s.st_mut::<State>(ci);
                st.burst_restore_ns = (st.burst_restore_ns + d).min(g::RESTORE_NS_CAP as f64);
            }
            // A1: the active character's points gained or used, while A1's
            // bonus lasts, make the next restore 4 points, once every 2.8 s
            Event::NightsoulGenerate { on, .. } | Event::NightsoulConsume { on, .. } => {
                if on != s.player.active as usize || !s.mod_active(ci, A1) || s.mod_active(ci, A1_ICD) {
                    return;
                }
                s.add_status(ci, A1_ICD, 168, true);
                s.st_mut::<State>(ci).a1_increase = true;
            }
            Event::NightsoulBurst(_) => s.add_status(ci, A4, 10 * 60, true),
            // C4: another member's burst while hers lasts
            Event::Burst => {
                if !s.mod_active(ci, BURST) || s.player.active as usize == ci || s.st::<State>(ci).c4_generated {
                    return;
                }
                let st = s.st_mut::<State>(ci);
                st.c4_stacks = 2;
                st.c4_generated = true;
            }
            _ => {}
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<crate::gcsl::Num, String>> {
        (fields[0] == "nightsoul").then(|| s.ns_condition(ci, fields))
    }
}

fn swift(s: &mut Sim, ci: usize) -> ActInfo {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_POLE_EXTRA_ATTACK, Element::Electro, 25.0, g::SWIFT[lvl]);
    ai.abil = "Swift Stormflight";
    ai.add_tags = add_tag::NIGHTSOUL;
    ai.poise = 120.0;
    ai.blunt = true;
    ai.ignore_infusion = true;
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = 0.05 * 60.0;
    // a 360-degree fan on the enemy, offset along its direction
    ai.pattern = s.circle_on_enemy(0.0, 0.5, 2.5);
    let h = g::SWIFT_HITMARK;
    let cbs = if s.chars[ci].ascension >= 1 { CB_A1 } else { 0 };
    s.queue_attack(ai, cbs, h, h);
    let t = &g::SWIFT_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Burst as usize], AnimState::Charge)
}

fn a1_add_buff(s: &mut Sim, ci: usize) {
    s.add_stat_mod(ci, A1, st::ATKP, 0.2, 15 * 60, true);
}

fn enter_nightsoul(s: &mut Sim, ci: usize, points: f64) {
    let f = s.f;
    s.st_mut::<State>(ci).nightsoul_src = f;
    s.ns_enter_timed(ci, points, 16 * 60, TASK_EXIT);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_REDUCE, arg: f }, 6);
}

fn exit_nightsoul(s: &mut Sim, ci: usize) {
    let st = s.st_mut::<State>(ci);
    st.nightsoul_src = -1;
    st.burst_src = -1;
    s.ns_exit(ci);
    s.ns_clear(ci);
    s.delete_mod(ci, BURST);
    s.delete_mod(ci, A1);
}

/// reduceNightsoulPoints, with C1's count of the points asked for.
fn reduce_points(s: &mut Sim, ci: usize, points: f64) {
    s.ns_consume(ci, points);
    c1(s, ci, points);
    if s.ns_points(ci) <= 0.2 {
        exit_nightsoul(s, ci);
    }
}

/// C1: every 6 points used, 15 energy, once every 18 s.
fn c1(s: &mut Sim, ci: usize, points: f64) {
    if s.chars[ci].cons < 1 || s.mod_active(ci, C1_ICD) {
        return;
    }
    s.st_mut::<State>(ci).c1_points += points;
    if s.st::<State>(ci).c1_points < 6.0 {
        return;
    }
    s.add_energy(ci, 15.0);
    s.add_status(ci, C1_ICD, 18 * 60, true);
    s.st_mut::<State>(ci).c1_points = 0.0;
}

/// restorePointsTask, every second on the fight's queue.
fn restore_points_task(s: &mut Sim, ci: usize, src: i32) {
    s.add_task(Task::Char { c: ci as u8, id: TASK_RESTORE, arg: src }, 60);
}

fn restore_points(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).burst_src != src {
        return;
    }
    if !s.mod_active(ci, BURST) {
        s.st_mut::<State>(ci).c4_stacks = 0;
        return;
    }
    let restore = s.st::<State>(ci).burst_restore_ns * g::RESTORE_NS_RATIO;
    let a1 = a1_points(s, ci);
    let c4 = c4_points(s, ci);
    let points = restore + a1 + c4;
    s.st_mut::<State>(ci).burst_restore_ns = 0.0;
    let overflow = (s.ns_points(ci) + points - s.chars[ci].ns.max_points).max(0.0);
    s.st_mut::<State>(ci).points_overflow = overflow;
    if overflow > 0.0 && s.chars[ci].cons >= 6 {
        // c6OnOverflow
        let active = s.player.active as usize;
        s.add_mod(active, Mod::owned(C6, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 3 * 60);
    }
    if points > 0.0 {
        s.ns_generate(ci, points);
    }
    if points >= 1.0 {
        a4_heal(s, ci);
    }
    restore_points_task(s, ci, src);
}

fn a1_points(s: &mut Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 1 || !s.mod_active(ci, A1) {
        return 0.0;
    }
    let st = s.st_mut::<State>(ci);
    if st.a1_increase {
        st.a1_increase = false;
        return 4.0;
    }
    1.0
}

fn c4_points(s: &mut Sim, ci: usize) -> f64 {
    if s.chars[ci].cons < 4 {
        return 0.0;
    }
    let st = s.st_mut::<State>(ci);
    let mut points = st.points_overflow * 0.5;
    if st.c4_stacks > 0 {
        points += 4.0;
        st.c4_stacks -= 1;
    }
    points
}

fn a4_heal(s: &mut Sim, ci: usize) {
    if !s.mod_active(ci, A4) || s.mod_active(ci, A4_ICD) {
        return;
    }
    let active = s.player.active as usize;
    let src = s.total_atk(ci) * 0.6;
    let bonus = s.stat(ci, st::HEAL);
    s.heal(ci, active, src, bonus);
    s.add_status(ci, A4_ICD, 168, true);
}

/// updateATKBuff, every 0.3 s on her queue while the burst lasts: 27% of
/// her ATK without extra mods at 42 points or more, 0.5% per point below,
/// up to the talent's cap.
fn update_atk_buff(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).burst_src != src {
        return;
    }
    if !s.mod_active(ci, BURST) {
        s.st_mut::<State>(ci).burst_atk = 0.0;
        return;
    }
    let pts = s.ns_points(ci);
    let rate = if pts < 42.0 { g::LOW_ATK * pts } else { g::HIGH_ATK };
    let atk = total_atk(&s.select_stats(ci, [st::BASEATK, st::ATKP, st::ATK], true));
    let lvl = s.talent(ci, 2);
    s.st_mut::<State>(ci).burst_atk = (atk * rate).min(g::MAX_ATK[lvl]);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ATK, arg: src }, 18);
}
