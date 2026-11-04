//! Dehya, from gcsim's internal/characters/dehya: normal attacks, the
//! skill and its recast with the Fiery Sanctum field (its coordinated
//! attacks on enemies damaged inside it, and its mitigation of the damage
//! the player takes inside it into Redmane's Blood, which Dehya then takes
//! over 10 s), the burst's punches and kick (through attack and skill,
//! and a jump during the dash's window), her dash and jump, A1, A4, every
//! constellation, and the plunges from Xianyun's airborne state. gcsim has
//! no charged attack for her.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::dehya as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    has_recast: bool,
    has_c2_buff: bool,
    /// The field's area, where the player and enemies must be.
    skill_area: Pattern,
    /// The field's coordinated attack and its snapshot (gcsim's
    /// skillAttackInfo and skillSnapshot).
    skill_ai: AttackInfo,
    skill_snap: AttackEvent,
    redmanes_blood: f64,
    self_dot_queued: bool,
    sanctum_saved_dur: i32,
    sanctum_icd: i32,
    burst_counter: usize,
    burst_hit_src: i32,
    c1_ratio_e: f64,
    c1_ratio_q: f64,
    c6_count: i32,
}

const FIELD: ModKey = ModKey::new(g::DEHYA_FIELD_KEY);
const SKILL_ICD: ModKey = ModKey::new(g::SKILL_ICD_KEY);
const SELF_DOT: ModKey = ModKey::new(g::SKILL_SELF_DO_T_STATUS);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const KICK: ModKey = ModKey::new(g::KICK_KEY);
const JUMP_KICK_WINDOW: ModKey = ModKey::new(g::JUMP_KICK_WINDOW_KEY);
const A1_ICD: ModKey = ModKey::new(g::A1_ICD_KEY);
const A1_REDUCTION: ModKey = ModKey::new(g::A1_REDUCTION_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const C1: ModKey = ModKey::new("dehya-c1");
const C2: ModKey = ModKey::new("dehya-sanctum-dot-c2");
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const C6: ModKey = ModKey::new("dehya-c6");
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const CB_C4: u16 = 1 << 0;
const CB_C6: u16 = 1 << 1;

const TASK_PICKUP: u8 = 0;
const TASK_ADD_FIELD: u8 = 1;
const TASK_DELETE_FIELD: u8 = 2;
const TASK_RECAST_FIELD: u8 = 3;
const TASK_BURST_START: u8 = 4;
const TASK_PUNCH: u8 = 5;
const TASK_KICK: u8 = 6;
const TASK_KICK_FIELD: u8 = 7;
const TASK_C2_OFF: u8 = 8;
const TASK_SELF_DOT: u8 = 9;
const TASK_A4_HEAL: u8 = 10;

const FRAMES_DASH: u8 = 0;

const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

/// A circle of radius r on a point (gcsim's NewCircleHitOnTarget on a
/// point, without offset).
fn circle_at(p: (f64, f64), r: f64) -> Pattern {
    Pattern::Circle { x: p.0, y: p.1, r }
}

/// gcsim's a1Reduction: Redmane's Blood hurts 60% less for 6 s after the
/// field is picked up, once every 2 s.
fn a1_reduction(s: &mut Sim, ci: usize) {
    if s.chars[ci].ascension < 1 || s.mod_active(ci, A1_ICD) {
        return;
    }
    s.add_status(ci, A1_ICD, g::A1_ICD, true);
    s.add_status(ci, A1_REDUCTION, g::A1_REDUCTION_DURATION, true);
}

/// gcsim's pickUpField: the field's time and coordinated attack cooldown
/// are kept, the field goes a frame later.
fn pick_up_field(s: &mut Sim, ci: usize) {
    a1_reduction(s, ci);
    let icd = s.mod_duration(ci, g::SKILL_ICD_KEY);
    let dur = s.mod_duration(ci, g::DEHYA_FIELD_KEY) + g::SANCTUM_PICKUP_EXTENSION;
    let x = st_mut(s, ci);
    x.sanctum_icd = icd;
    x.sanctum_saved_dur = dur;
    s.add_task(Task::Char { c: ci as u8, id: TASK_DELETE_FIELD, arg: 0 }, 1);
}

/// gcsim's addField: the field for dur frames, and its coordinated attack
/// snapshot now.
fn add_field(s: &mut Sim, ci: usize, dur: i32) {
    s.add_status(ci, FIELD, dur, false);
    let lvl = s.talent(ci, 1);
    let flat = (st(s, ci).c1_ratio_e + g::SKILL_DOT_HP[lvl]) * s.max_hp(ci);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_DOT_ATK[lvl]);
    ai.abil = g::SKILL_DO_T_ABIL;
    ai.flat_dmg = flat;
    ai.hitlag_halt = 0.02 * 60.0;
    ai.hitlag_factor = 0.01;
    ai.deployable = true;
    ai.pattern = s.circle_on_target(4.5);
    let snap = s.snapshot_event(ai, 0);
    let x = st_mut(s, ci);
    x.skill_ai = ai;
    x.skill_snap = snap;
}

/// An attack with the stored snapshot (gcsim's QueueAttackWithSnap with
/// skillSnapshot).
fn queue_with_field_snap(s: &mut Sim, ci: usize, ai: AttackInfo, delay: i32) {
    let mut ev = st(s, ci).skill_snap;
    ev.info = ai;
    s.queue_attack_with_snap(ev, 0, delay);
}

/// gcsim's UseBurstAction: a punch or the kick while the burst lasts.
fn use_burst_action(s: &mut Sim, ci: usize) -> Option<ActInfo> {
    st_mut(s, ci).burst_hit_src += 1;
    let src = st(s, ci).burst_hit_src;
    if s.mod_active(ci, KICK) {
        return Some(burst_kick(s, ci, src));
    }
    if s.mod_active(ci, BURST) {
        let Some(&h) = g::PUNCH_HITMARKS.get(st(s, ci).burst_counter) else {
            if s.error.is_none() {
                s.error = Some("dehya: index out of range in punchHitmarks".into());
            }
            return Some(ActInfo::new(Frames::Const(1), 1, 1, AnimState::Idle));
        };
        s.add_task(Task::Char { c: ci as u8, id: TASK_PUNCH, arg: src }, h);
        return Some(ActInfo::new(Frames::Const(h), h, h, AnimState::Idle));
    }
    None
}

/// gcsim's burstKick: the kick, as an action (Idle: burst state would
/// give i-frames).
fn burst_kick(s: &mut Sim, ci: usize, src: i32) -> ActInfo {
    s.add_task(Task::Char { c: ci as u8, id: TASK_KICK, arg: src }, g::BURST_KICK_HITMARK);
    let t = &g::KICK_FRAMES;
    ActInfo::new(Frames::Table(t), t[Action::Attack as usize], t[Action::Swap as usize], AnimState::Idle)
}

/// gcsim's burstPunchFunc.
fn punch(s: &mut Sim, ci: usize, src: i32) {
    if st(s, ci).burst_hit_src != src || s.player.active as usize != ci {
        return;
    }
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BURST_PUNCH_ATK[lvl]);
    ai.abil = "Flame-Mane's Fist";
    ai.blunt = true;
    ai.poise = 50.0;
    ai.flat_dmg = (st(s, ci).c1_ratio_q + g::BURST_PUNCH_HP[lvl]) * s.max_hp(ci);
    ai.pattern = s.box_on_player(0.0, -2.8, 5.0, 7.8);
    let cons = s.chars[ci].cons;
    let cbs = if cons >= 4 { CB_C4 } else { 0 } | if cons >= 6 { CB_C6 } else { 0 };
    s.queue_attack(ai, cbs, 0, 0);
    if !s.mod_active(ci, BURST) {
        let x = st_mut(s, ci);
        x.burst_hit_src += 1;
        let src = x.burst_hit_src;
        s.add_status(ci, KICK, g::BURST_KICK_HITMARK, true);
        s.add_task(Task::Char { c: ci as u8, id: TASK_KICK, arg: src }, g::BURST_KICK_HITMARK);
        return;
    }
    let x = st_mut(s, ci);
    x.burst_counter += 1;
    x.burst_hit_src += 1;
    let src = x.burst_hit_src;
    s.add_task(Task::Char { c: ci as u8, id: TASK_PUNCH, arg: src }, g::BURST_PUNCH_SLOW_HITMARK);
}

/// gcsim's burstKickFunc.
fn kick(s: &mut Sim, ci: usize, src: i32) {
    if st(s, ci).burst_hit_src != src || s.player.active as usize != ci {
        return;
    }
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BURST_KICK_ATK[lvl]);
    ai.abil = "Incineration Drive";
    ai.blunt = true;
    ai.poise = 100.0;
    ai.flat_dmg = (st(s, ci).c1_ratio_q + g::BURST_KICK_HP[lvl]) * s.max_hp(ci);
    ai.pattern = s.circle_on_player(0.0, 1.0, 6.5);
    let cbs = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
    s.queue_attack(ai, cbs, 0, 0);
    let dur = st(s, ci).sanctum_saved_dur;
    if dur > 0 {
        // the field back a frame later, not to set itself off
        st_mut(s, ci).sanctum_saved_dur = 0;
        s.add_task(Task::Char { c: ci as u8, id: TASK_KICK_FIELD, arg: dur }, 1);
    }
}

/// gcsim's skillSelfDoT: a tick of Redmane's Blood on Dehya.
fn self_dot(s: &mut Sim, ci: usize) {
    if !s.mod_active(ci, SELF_DOT) {
        st_mut(s, ci).self_dot_queued = false;
        return;
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SELF_DOT, arg: 0 }, g::SKILL_SELF_DO_T_INTERVAL);
    // not during her burst's i-frames
    let active = s.player.active as usize;
    if active == ci && s.current_state() == AnimState::Burst {
        return;
    }
    let x = st_mut(s, ci);
    let mut dmg = x.redmanes_blood * g::SKILL_SELF_DO_T_RATIO;
    x.redmanes_blood = (x.redmanes_blood - dmg).max(0.0);
    if s.mod_active(ci, A1_REDUCTION) {
        dmg *= 1.0 - g::A1_REDUCTION_MULT;
    }
    // gcsim snapshots the hit (which drops her expired stat mods), hands
    // it to OnPlayerHit as hers, then to the shields and her HP
    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 0.0, 0.0);
    ai.abil = g::SKILL_SELF_DO_T_ABIL;
    ai.flat_dmg = dmg;
    let _ = s.snapshot_event(ai, 0);
    s.player_hit_event(ci, Element::Physical, AttackTag::None);
    let left = s.shields_on_damage(ci, active, dmg, Element::Physical);
    if left > 0.0 {
        s.drain_info(ci, left, true, g::SKILL_SELF_DO_T_ABIL);
    }
}

impl Character for Char {
    const KEY: &'static str = "dehya";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        let ai = AttackInfo::new(0, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 0.0, 0.0);
        State {
            has_recast: false,
            has_c2_buff: false,
            skill_area: Pattern::Circle { x: 0.0, y: 0.0, r: 0.0 },
            skill_ai: ai,
            skill_snap: AttackEvent::new(ai, 0),
            redmanes_blood: 0.0,
            self_dot_queued: false,
            sanctum_saved_dur: 0,
            sanctum_icd: 0,
            burst_counter: 0,
            burst_hit_src: 0,
            c1_ratio_e: 0.0,
            c1_ratio_q: 0.0,
            c6_count: 0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        s.subscribe(EventKind::Swap, o);
        s.pre_drain_subs.push(ci as u8);
        s.subscribe(EventKind::EnemyDamage, o);
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::Drain, o);
        }
        let cons = s.chars[ci].cons;
        if cons >= 1 {
            s.add_mod(ci, Mod::stat(C1, st::HPP, 0.2, false, false, -1), -1);
            let x = st_mut(s, ci);
            x.c1_ratio_e = 0.036;
            x.c1_ratio_q = 0.06;
        }
        if cons >= 2 {
            s.add_mod(ci, Mod::owned(C2, ModKind::Attack, o, st::NONE, false, false, -1), -1);
            s.subscribe(EventKind::PlayerHit, o);
        }
        if cons >= 6 {
            s.add_mod(ci, Mod::owned(C6, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::Jump && s.player.active as usize == ci && s.mod_active(ci, KICK) && !(s.mod_active(ci, BURST) && s.mod_active(ci, JUMP_KICK_WINDOW)) {
            return Some("can't jump cancel burst kick".into());
        }
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    /// The recast while the field is out, and attack and skill as punches
    /// or the kick during the burst, whatever the cooldown.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        if a == Action::Skill && s.mod_active(ci, FIELD) && !st(s, ci).has_recast {
            return Some(None);
        }
        if (a == Action::Skill || a == Action::Attack) && (s.mod_active(ci, BURST) || s.mod_active(ci, KICK)) {
            return Some(None);
        }
        None
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if let Some(info) = use_burst_action(s, ci) {
            return info;
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.blunt = true;
        ai.poise = g::ATTACK_POISE_DMG[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        let (off, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
        ai.pattern = if n == 1 || n == 3 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if let Some(info) = use_burst_action(s, ci) {
            return info;
        }
        let mut need_pickup = false;
        if s.mod_active(ci, FIELD) {
            if st(s, ci).has_recast {
                need_pickup = true;
            } else {
                st_mut(s, ci).has_recast = true;
                return skill_recast(s, ci);
            }
        }
        let x = st_mut(s, ci);
        x.has_recast = false;
        x.has_c2_buff = false;
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL[lvl]);
        ai.abil = "Molten Inferno";
        ai.blunt = true;
        ai.poise = 50.0;
        ai.flat_dmg = st(s, ci).c1_ratio_e * s.max_hp(ci);
        let mut snap = s.snapshot_event(ai, 0);
        let pos = s.point_from_player(0.0, 0.8);
        let x = st_mut(s, ci);
        x.skill_snap = snap;
        x.skill_area = circle_at(pos, 10.0);
        snap.info.pattern = circle_at(pos, 5.0);
        s.queue_attack_with_snap(snap, 0, g::SKILL_HITMARK);
        // the initial hit does not set the field off
        s.add_status(ci, SKILL_ICD, g::SKILL_HITMARK + 1, false);
        if need_pickup {
            s.add_task(Task::Char { c: ci as u8, id: TASK_PICKUP, arg: 0 }, g::SKILL_HITMARK - 1);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_ADD_FIELD, arg: g::DEHYA_FIELD_DURATION }, g::SKILL_HITMARK + 1);
        s.set_cd_with_delay(ci, Action::Skill, 20 * 60, 18);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let x = st_mut(s, ci);
        x.c6_count = 0;
        x.sanctum_saved_dur = 0;
        if s.mod_active(ci, FIELD) {
            pick_up_field(s, ci);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_START, arg: 0 }, g::BURST_PUNCH1_HITMARK);
        s.consume_energy(ci, 15);
        s.set_cd_with_delay(ci, Action::Burst, 18 * 60, 1);
        let h = g::BURST_PUNCH1_HITMARK;
        ActInfo::new(Frames::Const(h), h, h, AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_PICKUP => pick_up_field(s, ci),
            TASK_ADD_FIELD => add_field(s, ci, arg),
            TASK_DELETE_FIELD => s.delete_mod(ci, FIELD),
            TASK_RECAST_FIELD => {
                // C2: the recreated field lasts 6 s longer
                if s.chars[ci].cons >= 2 {
                    st_mut(s, ci).sanctum_saved_dur += 360;
                }
                let dur = st(s, ci).sanctum_saved_dur;
                add_field(s, ci, dur);
            }
            TASK_BURST_START => {
                let x = st_mut(s, ci);
                x.burst_hit_src = 0;
                x.burst_counter = 0;
                s.add_status(ci, BURST, g::BURST_DURATION as i32, true);
                punch(s, ci, 0);
            }
            TASK_PUNCH => punch(s, ci, arg),
            TASK_KICK => kick(s, ci, arg),
            TASK_KICK_FIELD => {
                let icd = st(s, ci).sanctum_icd;
                s.add_status(ci, SKILL_ICD, icd, false);
                add_field(s, ci, arg);
            }
            TASK_C2_OFF => st_mut(s, ci).has_c2_buff = false,
            TASK_SELF_DOT => self_dot(s, ci),
            TASK_A4_HEAL => {
                if arg == 5 {
                    return;
                }
                let src = g::A4_DO_T_HEAL_RATIO * s.max_hp(ci);
                let bonus = s.stat(ci, st::HEAL);
                s.heal(ci, ci, src, bonus);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A4_HEAL, arg: arg + 1 }, g::A4_DO_T_HEAL_INTERVAL);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        // C4: energy and HP on the burst's hits
        if cbs & CB_C4 != 0 && !s.mod_active(ci, C4_ICD) {
            s.add_status(ci, C4_ICD, 12, true);
            s.add_energy(ci, 1.5);
            let src = 0.025 * s.max_hp(ci);
            let bonus = s.stat(ci, st::HEAL);
            s.heal(ci, ci, src, bonus);
        }
        // C6: a punch's crit raises the burst's CRIT DMG and lengthens it
        if cbs & CB_C6 != 0 && ev.crit && st(s, ci).c6_count != 4 && !s.mod_active(ci, C6_ICD) {
            s.add_status(ci, C6_ICD, 12, true);
            st_mut(s, ci).c6_count += 1;
            s.extend_mod(ci, BURST, 30);
        }
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C2 {
            if ev.info.abil == g::SKILL_DO_T_ABIL && st(s, ci).has_c2_buff {
                ev.stats[st::DMGP] += 0.5;
            }
        } else if ev.info.tag == AttackTag::ElementalBurst {
            // C6
            ev.stats[st::CR] += 0.1;
            ev.stats[st::CD] += 0.15 * st(s, ci).c6_count as f64;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // leaving the field ends the burst and puts the field back
            Event::Swap { .. } => {
                if !s.mod_active(ci, BURST) && !s.mod_active(ci, KICK) {
                    return;
                }
                s.delete_mod(ci, BURST);
                let dur = st(s, ci).sanctum_saved_dur;
                if dur > 0 {
                    st_mut(s, ci).sanctum_saved_dur = 0;
                    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ADD_FIELD, arg: dur }, g::BURST_KICK_HITMARK);
                }
            }
            // the field's coordinated attack on an enemy damaged in it
            Event::EnemyDamage(ev) => {
                if !s.mod_active(ci, FIELD) || s.mod_active(ci, SKILL_ICD) || ev.damage == 0.0 || !s.enemy_within(st(s, ci).skill_area) {
                    return;
                }
                // tied to the construct, so not hitlag extended
                s.add_status(ci, SKILL_ICD, 150, false);
                let mut ai = st(s, ci).skill_ai;
                ai.pattern = s.circle_on_target(4.5);
                queue_with_field_snap(s, ci, ai, 2);
                if st(s, ci).has_c2_buff {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_C2_OFF, arg: 0 }, 3);
                }
                s.queue_particle(1.0, Element::Pyro, s.particle_delay[ci]);
            }
            // A4: healing when a drain leaves her below 40% HP
            Event::Drain { amount, .. } => {
                if amount <= 0.0 || s.chars[ci].hp_ratio >= g::A4_THRESHOLD || s.mod_active(ci, A4_ICD) {
                    return;
                }
                s.add_status(ci, A4_ICD, g::A4_ICD, true);
                let src = g::A4_INITIAL_HEAL_RATIO * s.max_hp(ci);
                let bonus = s.stat(ci, st::HEAL);
                s.heal(ci, ci, src, bonus);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A4_HEAL, arg: 0 }, g::A4_DO_T_HEAL_INTERVAL);
            }
            // C2: the active character hit inside the field
            Event::PlayerHit { on, .. } => {
                if on == s.player.active as usize && s.mod_active(ci, FIELD) && s.player_within(st(s, ci).skill_area) {
                    st_mut(s, ci).has_c2_buff = true;
                }
            }
            _ => {}
        }
    }

    /// The field's mitigation: part of the damage the player takes in it
    /// goes into Redmane's Blood instead.
    fn pre_drain(s: &mut Sim, ci: usize, _: usize, amount: f64, external: bool, abil: &'static str) -> f64 {
        if !external || amount <= 0.0 || !s.mod_active(ci, FIELD) || !s.player_within(st(s, ci).skill_area) || abil == g::SKILL_SELF_DO_T_ABIL {
            return amount;
        }
        if st(s, ci).redmanes_blood >= 2.0 * s.max_hp(ci) {
            return amount;
        }
        let mitigation = amount * g::SKILL_MITIGATION[s.talent(ci, 1)];
        st_mut(s, ci).redmanes_blood += mitigation;
        s.add_status(ci, SELF_DOT, g::SKILL_SELF_DO_T_DURATION, true);
        if !st(s, ci).self_dot_queued {
            st_mut(s, ci).self_dot_queued = true;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SELF_DOT, arg: 0 }, g::SKILL_SELF_DO_T_START as i32);
        }
        (amount - mitigation).max(0.0)
    }

    /// The burst's dash: a jump in its window turns into the kick.
    fn template_action(s: &mut Sim, ci: usize, a: Action) {
        if a == Action::Dash && s.mod_active(ci, BURST) {
            s.add_status(ci, JUMP_KICK_WINDOW, g::JUMP_KICK_WINDOW_DURATION, false);
        }
    }

    fn dash_info(s: &mut Sim, ci: usize, info: ActInfo) -> ActInfo {
        let len = info.anim_len;
        let can_queue = if s.mod_active(ci, BURST) { g::BURST_DASH_DURATION } else { len };
        ActInfo::new(Frames::CharLive { id: FRAMES_DASH, arg: len as i16 }, len, can_queue, AnimState::Dash)
    }

    /// The dash's frames: a jump comes early while the burst lasts.
    fn frames_live(s: &Sim, ci: usize, _: u8, arg: i16, next: Action) -> i32 {
        if s.mod_active(ci, BURST) && next == Action::Jump {
            g::BURST_DASH_DURATION
        } else {
            arg as i32
        }
    }

    fn own_jump(s: &mut Sim, ci: usize, _: &Params) -> Option<ActInfo> {
        if s.mod_active(ci, BURST) && s.mod_active(ci, JUMP_KICK_WINDOW) {
            st_mut(s, ci).burst_hit_src += 1;
            s.delete_mod(ci, JUMP_KICK_WINDOW);
            let src = st(s, ci).burst_hit_src;
            return Some(burst_kick(s, ci, src));
        }
        // a jump otherwise ends the burst and puts the field back
        if s.mod_active(ci, BURST) {
            st_mut(s, ci).burst_hit_src = -1;
            s.delete_mod(ci, BURST);
            let dur = st(s, ci).sanctum_saved_dur;
            if dur > 0 {
                st_mut(s, ci).sanctum_saved_dur = 0;
                add_field(s, ci, dur);
            }
        }
        None
    }
}

/// gcsim's skillRecast: Ranging Flame picks the field up and puts it down
/// where Dehya stands.
fn skill_recast(s: &mut Sim, ci: usize) -> ActInfo {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_REPOSITION[lvl]);
    ai.abil = "Ranging Flame";
    ai.blunt = true;
    ai.poise = 50.0;
    ai.flat_dmg = st(s, ci).c1_ratio_e * s.max_hp(ci);
    ai.hitlag_halt = 0.02 * 60.0;
    ai.hitlag_factor = 0.01;
    pick_up_field(s, ci);
    let icd = st(s, ci).sanctum_icd;
    s.add_status(ci, SKILL_ICD, g::SKILL_RECAST_HITMARK + icd, false);
    let pos = s.point_from_player(0.0, 0.5);
    st_mut(s, ci).skill_area = circle_at(pos, 10.0);
    ai.pattern = circle_at(pos, 6.0);
    queue_with_field_snap(s, ci, ai, g::SKILL_RECAST_HITMARK);
    s.add_task(Task::Char { c: ci as u8, id: TASK_RECAST_FIELD, arg: 0 }, g::SKILL_RECAST_HITMARK + 1);
    let t = &g::SKILL_RECAST_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
}
