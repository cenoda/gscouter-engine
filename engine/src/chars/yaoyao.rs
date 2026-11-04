//! Yaoyao, from gcsim's internal/characters/yaoyao: normal and charged
//! attacks, Raphanus Sky Cluster (the throwing Yuegui), Moonjade Descent
//! (the jumping Yuegui), A1, A4 and every constellation.
//!
//! As gcsim writes them:
//! - A Yuegui snapshots its radish when it is placed, and every radish it
//!   throws uses that snapshot. Its heal also reads the snapshot's max HP
//!   and healing bonus.
//! - While the burst is up, a Yuegui's radish takes the burst's attack and
//!   heal (the whole party) on the skill's snapshot.
//! - A Yuegui thinks 29 ticks after it is placed, then every 60. Each
//!   think reads the active character's HP and draws one random number
//!   when the enemy is within 8 of it (RandomEnemyWithinArea on the one
//!   enemy). Above 70% HP it throws at the enemy, otherwise at the player.
//!   The radish lands 13 frames later, and heals only when the player
//!   stands in it.
//! - A1's radishes snapshot when they leave, 12 frames after the dash or
//!   jump is seen.
//! - C6's Mega Radish heals with Yaoyao's max HP at the throw and gives
//!   neither C1 nor A4.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::yaoyao as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

/// A Yuegui's state: its snapshot (gcsim's yg.ai and yg.snap), where it
/// stands, and its throws so far.
#[derive(Clone, Copy)]
struct Yuegui {
    ev: AttackEvent,
    x: f64,
    y: f64,
    throwing: bool,
    throws: i32,
}

/// A radish in the air, between the throw and the char task 12 frames
/// later that sends it.
#[derive(Clone, Copy)]
struct Throw {
    x: f64,
    y: f64,
    /// The Yuegui's snapshot, or None for A1's (snapshot when it leaves).
    ev: Option<AttackEvent>,
    throwing: bool,
}

/// A radish's heal, a core task after it is sent.
#[derive(Clone, Copy)]
struct Heal {
    area: Pattern,
    src: f64,
    bonus: f64,
    party: bool,
    c6: bool,
}

const SLOTS: usize = 8;

#[derive(Default)]
pub struct State {
    /// Throwing Yuegui by gadget id (0 and 1, alternating: one lasts 600
    /// frames and the skill's cooldown is 900, so one is the most there
    /// is unless a cooldown is cut).
    throwing: [Option<Yuegui>; 2],
    next_throwing: usize,
    /// Jumping Yuegui by gadget id - 2 (gcsim's yueguiJumping).
    jumping: [Option<Yuegui>; 3],
    num_jumping: usize,
    throws: [Option<Throw>; SLOTS],
    heals: [Option<Heal>; SLOTS],
    /// gcsim's a4Srcs.
    a4_srcs: [i32; 4],
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const YUEGUI: ModKey = ModKey::new("yuegui");
const PARTICLE_ICD: ModKey = ModKey::new(g::SKILL_PARTICLE_ICD);
const A4: ModKey = ModKey::new(g::A4_STATUS);
const C1: ModKey = ModKey::new("yaoyao-c1");
const C1_ICD: ModKey = ModKey::new(g::C1_IC_DKEY);
const C2_ICD: ModKey = ModKey::new(g::C2_IC_DKEY);
const C4: ModKey = ModKey::new("yaoyao-c4");

const CB_PARTICLE: u16 = 1 << 0;
const CB_C2: u16 = 1 << 1;

const TASK_SPAWN_THROWING: u8 = 0;
const TASK_SPAWN_JUMPING: u8 = 1;
const TASK_REMOVE_BURST: u8 = 2;
const TASK_THROW: u8 = 3;
const TASK_HEAL: u8 = 4;
const TASK_A1_TICK: u8 = 5;
const TASK_A4: u8 = 6;

const GADGET_JUMPING: u8 = 2;

impl Character for Char {
    const KEY: &'static str = "yaoyao";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        // onExitField: a swap ends the burst
        s.subscribe(EventKind::Swap, Owner::Char(ci as u8));
    }

    /// gcsim's package reads no params.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let marks = g::ATTACK_HITMARKS[n];
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let hb = g::ATTACK_HITBOXES[n];
            let oy = g::ATTACK_OFFSETS[n];
            ai.pattern = if n == 0 { s.box_on_player(0.0, oy, hb[0], hb[1]) } else { s.fan_on_player(0.0, oy, hb[0], g::ATTACK_FAN_ANGLES[n]) };
            s.queue_attack(ai, 0, marks[i], marks[i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *marks.last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::NC[lvl]);
        ai.abil = "Charge";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        // NewCircleHit from the player on the primary target
        ai.pattern = s.circle_on_target(0.8);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_core_status(YUEGUI, 600 + g::YUEGUI_THROW_SPAWN);
        s.add_task(Task::Char { c: ci as u8, id: TASK_SPAWN_THROWING, arg: 0 }, g::SKILL_CD_START + g::YUEGUI_THROW_SPAWN);
        s.set_cd_with_delay(ci, Action::Skill, 15 * 60, g::SKILL_CD_START);
        if s.chars[ci].cons >= 4 {
            c4(s, ci);
        }
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.set_cd(ci, Action::Burst, 20 * 60);
        s.consume_energy(ci, 7);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::BURST_DMG[lvl]);
        ai.abil = "Moonjade Descent";
        ai.hitlag_halt = 0.02 * 60.0;
        ai.hitlag_factor = 0.05;
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
        s.queue_attack(ai, 0, g::BURST_INITIAL_HITMARK, g::BURST_INITIAL_HITMARK);
        for d in [104, 162, 221] {
            s.add_task(Task::Char { c: ci as u8, id: TASK_SPAWN_JUMPING, arg: 0 }, d);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_REMOVE_BURST, arg: 0 }, g::BURST_DUR);
        s.add_status(ci, BURST, g::BURST_DUR, false);
        if s.chars[ci].cons >= 4 {
            c4(s, ci);
        }
        if s.chars[ci].ascension >= 1 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1_TICK, arg: 0 }, 36);
        }
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SPAWN_THROWING => spawn_throwing(s, ci),
            TASK_SPAWN_JUMPING => spawn_jumping(s, ci),
            TASK_REMOVE_BURST => remove_burst(s, ci),
            TASK_THROW => send_radish(s, ci, arg as usize),
            TASK_HEAL => {
                let Some(h) = s.st_mut::<State>(ci).heals[arg as usize].take() else { unreachable!() };
                radish_heal(s, ci, h);
            }
            TASK_A1_TICK => {
                if !s.mod_active(ci, BURST) {
                    return;
                }
                if matches!(s.current_state(), AnimState::Dash | AnimState::Jump) {
                    a1_throw(s, ci);
                }
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1_TICK, arg: 0 }, 36);
            }
            TASK_A4 => {
                let (idx, src) = ((arg & 3) as usize, arg >> 2);
                if s.st::<State>(ci).a4_srcs[idx] != src || !s.mod_active(idx, A4) {
                    return;
                }
                let amt = 0.008 * s.max_hp(ci);
                let bonus = s.stat(ci, st::HEAL);
                s.heal(ci, idx, amt, bonus);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A4, arg }, 60);
            }
            _ => unreachable!(),
        }
    }

    /// A Yuegui thinks at its 29th tick, then every 60.
    fn gadget_tick(s: &mut Sim, ci: usize, id: u8, timer: i32) {
        if timer < 29 || (timer - 29) % 60 != 0 {
            return;
        }
        throw(s, ci, id);
    }

    /// The order gcsim appends them: particles, then C2.
    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 90, true);
            s.queue_particle(1.0, Element::Dendro, s.particle_delay[ci]);
        }
        if cbs & CB_C2 != 0 && !s.mod_active(ci, C2_ICD) {
            s.add_energy(ci, 3.0);
            s.add_status(ci, C2_ICD, 48, false);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Swap { .. } = *e {
            if s.mod_active(ci, BURST) {
                remove_burst(s, ci);
            }
        }
    }
}

fn skill_radish(s: &Sim, ci: usize) -> AttackInfo {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_YAOYAO_RADISH_SKILL, Element::Dendro, 25.0, g::SKILL_RADISH_DMG[lvl]);
    ai.abil = "Radish (Skill)";
    ai.def_halt = true;
    ai.deployable = true;
    ai.pattern = s.circle_on_target(g::RADISH_RAD);
    ai
}

fn burst_radish(s: &Sim, ci: usize) -> AttackInfo {
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_YAOYAO_RADISH_BURST, Element::Dendro, 25.0, g::BURST_RADISH_DMG[lvl]);
    ai.abil = "Radish (Burst)";
    ai.def_halt = true;
    ai.deployable = true;
    ai.pattern = s.circle_on_target(g::RADISH_RAD);
    ai
}

/// gcsim's getSkillHealInfo and getBurstHealInfo on a snapshot's stats:
/// the heal and its bonus.
fn radish_heal_info(s: &Sim, ci: usize, stats: &Stats, burst: bool) -> (f64, f64) {
    let hp = max_hp(stats);
    let (t, lvl) = if burst { (g::BURST_RADISH_HEALING, s.talent(ci, 2)) } else { (g::SKILL_RADISH_HEALING, s.talent(ci, 1)) };
    (t[0][lvl] * hp + t[1][lvl], stats[st::HEAL])
}

/// gcsim's newYueguiThrow, added at once.
fn spawn_throwing(s: &mut Sim, ci: usize) {
    let ev = s.snapshot_event(skill_radish(s, ci), 0);
    let (x, y) = s.point_from_player(0.0, 2.0);
    let slot = s.st::<State>(ci).next_throwing;
    if s.gadget_index(ci, slot as u8).is_some() {
        s.unsupported("yaoyao: a third throwing Yuegui at once (the skill's cooldown cut below 600 frames)");
        return;
    }
    let st = s.st_mut::<State>(ci);
    st.next_throwing ^= 1;
    st.throwing[slot] = Some(Yuegui { ev, x, y, throwing: true, throws: 0 });
    s.add_gadget(ci, slot as u8, 600, x, y, 0.5);
}

/// gcsim's newYueguiJump.
fn spawn_jumping(s: &mut Sim, ci: usize) {
    if !s.mod_active(ci, BURST) || s.st::<State>(ci).num_jumping >= 3 {
        return;
    }
    let ev = s.snapshot_event(burst_radish(s, ci), 0);
    let (x, y) = s.point_from_player(0.0, -2.0);
    let st = s.st_mut::<State>(ci);
    let n = st.num_jumping;
    st.jumping[n] = Some(Yuegui { ev, x, y, throwing: false, throws: 0 });
    st.num_jumping += 1;
    s.add_gadget(ci, GADGET_JUMPING + n as u8, -1, x, y, 0.5);
}

/// gcsim's removeBurst.
fn remove_burst(s: &mut Sim, ci: usize) {
    s.delete_mod(ci, BURST);
    for i in 0..3 {
        if s.st_mut::<State>(ci).jumping[i].take().is_some() {
            s.kill_gadget(ci, GADGET_JUMPING + i as u8);
        }
    }
    s.st_mut::<State>(ci).num_jumping = 0;
}

fn yuegui_mut(st: &mut State, id: u8) -> &mut Yuegui {
    let y = if id < GADGET_JUMPING { &mut st.throwing[id as usize] } else { &mut st.jumping[(id - GADGET_JUMPING) as usize] };
    y.as_mut().unwrap()
}

/// The one enemy, when its center is in the area: gcsim's
/// RandomEnemyWithinArea, which draws a random index among those found.
fn enemy_in(s: &mut Sim, area: Pattern) -> bool {
    if s.enemy_alive && s.enemy_within(area) {
        let _ = s.rng.intn(1);
        return true;
    }
    false
}

fn free_slot<T>(a: &[Option<T>; SLOTS]) -> Option<usize> {
    a.iter().position(|x| x.is_none())
}

/// gcsim's yuegui.throw.
fn throw(s: &mut Sim, ci: usize, id: u8) {
    let y = *yuegui_mut(s.st_mut::<State>(ci), id);
    let active = s.player.active as usize;
    let hp_ratio = s.chars[active].hp_ratio;
    let found = enemy_in(s, Pattern::Circle { x: y.x, y: y.y, r: g::SKILL_TARGETING_RAD as f64 });
    let (tx, ty) = if hp_ratio > 0.7 && found { s.enemy_pos } else { s.player_pos };
    let Some(k) = free_slot(&s.st::<State>(ci).throws) else {
        s.unsupported("yaoyao: more radishes in the air than the engine keeps");
        return;
    };
    s.st_mut::<State>(ci).throws[k] = Some(Throw { x: tx, y: ty, ev: Some(y.ev), throwing: y.throwing });
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_THROW, arg: k as i32 }, g::TRAVEL_DELAY - 1);
    if y.throwing && s.chars[ci].cons >= 6 && (y.throws == 2 || y.throws == 5) {
        c6(s, ci, &y.ev, tx, ty);
    }
    yuegui_mut(s.st_mut::<State>(ci), id).throws += 1;
}

/// The radish leaves: its attack (and its heal, a core task later) land
/// 1 frame after.
fn send_radish(s: &mut Sim, ci: usize, k: usize) {
    let Some(t) = s.st_mut::<State>(ci).throws[k].take() else { unreachable!() };
    let burst = s.mod_active(ci, BURST);
    let mut ev = match t.ev {
        // getInfos: the burst's attack on the Yuegui's snapshot while the
        // burst is up, else the Yuegui's own
        Some(mut ev) => {
            if burst {
                ev.info = burst_radish(s, ci);
            }
            ev
        }
        // A1's: a fresh snapshot
        None => {
            let ai = if burst { burst_radish(s, ci) } else { skill_radish(s, ci) };
            s.snapshot_event(ai, 0)
        }
    };
    let area = Pattern::Circle { x: t.x, y: t.y, r: g::RADISH_RAD };
    ev.info.pattern = area;
    let (src, bonus) = radish_heal_info(s, ci, &ev.stats, burst);
    let Some(h) = free_slot(&s.st::<State>(ci).heals) else {
        s.unsupported("yaoyao: more radishes in the air than the engine keeps");
        return;
    };
    s.st_mut::<State>(ci).heals[h] = Some(Heal { area, src, bonus, party: burst, c6: false });
    s.add_task(Task::Char { c: ci as u8, id: TASK_HEAL, arg: h as i32 }, 1);
    let mut cbs = if t.throwing { CB_PARTICLE } else { 0 };
    if s.chars[ci].cons >= 2 && burst {
        cbs |= CB_C2;
    }
    s.queue_attack_with_snap(ev, cbs, 1);
}

/// gcsim's heal and radishHeal: only when the player stands in the area.
fn radish_heal(s: &mut Sim, ci: usize, h: Heal) {
    if !s.player_within(h.area) {
        return;
    }
    if h.party {
        s.heal_info(ci, None, false, h.src, h.bonus);
    } else {
        let active = s.player.active as usize;
        s.heal(ci, active, h.src, h.bonus);
    }
    if h.c6 {
        return;
    }
    if s.chars[ci].cons >= 1 {
        c1(s);
        if !s.mod_active(ci, C1_ICD) {
            s.restore_stam(15.0);
            s.add_status(ci, C1_ICD, 5 * 60, false);
        }
    }
    if s.chars[ci].ascension >= 4 {
        let active = s.player.active as usize;
        s.add_status(active, A4, 5 * 60, true);
        let f = s.f;
        s.st_mut::<State>(ci).a4_srcs[active] = f;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A4, arg: f << 2 | active as i32 }, 60);
    }
}

/// gcsim's a1Throw: a radish on the enemy within 8 of the player.
fn a1_throw(s: &mut Sim, ci: usize) {
    let (px, py) = s.player_pos;
    if !enemy_in(s, Pattern::Circle { x: px, y: py, r: g::SKILL_TARGETING_RAD as f64 }) {
        return;
    }
    let (x, y) = s.enemy_pos;
    let Some(k) = free_slot(&s.st::<State>(ci).throws) else {
        s.unsupported("yaoyao: more radishes in the air than the engine keeps");
        return;
    };
    s.st_mut::<State>(ci).throws[k] = Some(Throw { x, y, ev: None, throwing: false });
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_THROW, arg: k as i32 }, g::TRAVEL_DELAY - 1);
}

/// C1: the active character healed gets +15% dendro DMG for 8 s.
fn c1(s: &mut Sim) {
    let active = s.player.active as usize;
    s.add_stat_mod(active, C1, st::DENDROP, 0.15, 8 * 60, false);
}

/// C4: EM of 0.3% of her max HP (at most 120) for 8.8 s.
fn c4(s: &mut Sim, ci: usize) {
    let em = (s.max_hp(ci) * 0.003).min(120.0);
    s.add_mod(ci, Mod::stat(C4, st::EM, em, true, true, -1), 528);
}

/// C6: the Mega Radish on the Yuegui's snapshot, 20 frames after the
/// throw, and its heal with her max HP now.
fn c6(s: &mut Sim, ci: usize, yev: &AttackEvent, x: f64, y: f64) {
    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, 0.75);
    ai.abil = "Mega Radish";
    ai.def_halt = true;
    ai.deployable = true;
    let area = Pattern::Circle { x, y, r: g::C6_MEGA_RADISH_RAD };
    ai.pattern = area;
    let src = s.max_hp(ci) * 0.075;
    let bonus = s.stat(ci, st::HEAL);
    let Some(h) = free_slot(&s.st::<State>(ci).heals) else {
        s.unsupported("yaoyao: more radishes in the air than the engine keeps");
        return;
    };
    s.st_mut::<State>(ci).heals[h] = Some(Heal { area, src, bonus, party: false, c6: true });
    s.add_task(Task::Char { c: ci as u8, id: TASK_HEAL, arg: h as i32 }, g::C6_TRAVEL_DELAY);
    let mut ev = *yev;
    ev.info = ai;
    s.queue_attack_with_snap(ev, 0, g::C6_TRAVEL_DELAY);
}
