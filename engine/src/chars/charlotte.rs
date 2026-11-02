//! Charlotte, from gcsim's internal/characters/charlotte: normal attacks,
//! the charged attack and its Spiritbreath Thorn, Framing: Freezing Point
//! Composition pressed and held (the photo, its particles and the marks it
//! leaves, which tick every 1.5 s), Still Photo: Comprehensive
//! Confirmation (the hit, the Kamera's eight ticks and their heals, all on
//! the cast's snapshot), A1's cooldown on kills of held-marked enemies, A4's
//! healing bonus or cryo DMG by the team's nations, the airborne plunges and
//! every constellation.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::charlotte as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// markCount: the enemies the current skill has marked.
    mark_count: i32,
    /// c2Hits: the enemies the current skill has hit, for C2.
    c2_hits: i32,
    /// The enemy's charlotte-mark tag: the frame it was last marked.
    mark_tag: i32,
    /// The last skill's hold param (the charged attack's windup after it).
    last_hold: i64,
    /// The charged attack's area, on the enemy as it is cast.
    charge_area: Pattern,
    /// A1's count of kills, reset 12 s after the first.
    a1_count: i32,
    /// A4's members from Fontaine and from elsewhere.
    a4_heal: i32,
    a4_cryo: i32,
    /// C4's count of hits, reset 20 s after the first.
    c4_count: i32,
    /// The burst's heals, on its snapshot.
    burst_heal: f64,
    burst_heal_dot: f64,
    burst_healp: f64,
    /// The Kamera's tick, on the burst's snapshot, and the heal's area.
    burst_tick: AttackEvent,
    burst_heal_area: Pattern,
    /// A heal of hers with one of the messages C1 reads is being given.
    c1_msg: bool,
}

const ZONE_FONTAINE: u8 = 5; // gcsim's info.ZoneFontaine

const PRESS_MARK: ModKey = ModKey::new(g::SKILL_PRESS_MARK_KEY);
const HOLD_MARK: ModKey = ModKey::new(g::SKILL_HOLD_MARK_KEY);
const ARKHE_ICD: ModKey = ModKey::new(g::ARKHE_ICD_KEYS);
const A4: ModKey = ModKey::new("charlotte-a4");
const C1: ModKey = ModKey::new(g::C1_STATUS_KEY);
const C2: ModKey = ModKey::new("charlotte-c2");
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const CB_PRESS_PARTICLE: u16 = 1 << 0;
const CB_HOLD_PARTICLE: u16 = 1 << 1;
const CB_PRESS_MARK: u16 = 1 << 2;
const CB_HOLD_MARK: u16 = 1 << 3;
const CB_C2: u16 = 1 << 4;

/// The charged attack's hit, at its hitmark.
const TASK_CHARGE: u8 = 0;
/// The Spiritbreath Thorn, 30 frames after it.
const TASK_ARKHE: u8 = 1;
/// A mark's tick, with src * 2 + (1 for the hold mark).
const TASK_MARK: u8 = 2;
/// The burst's party heal, as its hit lands; queues the ticks.
const TASK_BURST_START: u8 = 3;
/// A Kamera tick and its heal.
const TASK_BURST_TICK: u8 = 4;
/// A1's count reset.
const TASK_A1_RESET: u8 = 5;
/// C1's heal on a character (arg).
const TASK_C1_HEAL: u8 = 6;
/// C4's count reset.
const TASK_C4_RESET: u8 = 7;
/// C6's hit and heal.
const TASK_C6: u8 = 8;

/// The charged attack's frames, less its windup (arg).
const FRAMES_CHARGE: u8 = 0;
/// The held skill's frames, after its hold (arg).
const FRAMES_HOLD: u8 = 1;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];

const PLUNGE: plunge::Spec = plunge::Spec::catalyst(Element::Cryo, g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

impl Character for Char {
    const KEY: &'static str = "charlotte";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        let ai = AttackInfo::new(0, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 0.0);
        State {
            mark_count: 0,
            c2_hits: 0,
            mark_tag: 0,
            last_hold: 0,
            charge_area: Pattern::Single,
            a1_count: 0,
            a4_heal: 0,
            a4_cryo: 0,
            c4_count: 0,
            burst_heal: 0.0,
            burst_heal_dot: 0.0,
            burst_healp: 0.0,
            burst_tick: AttackEvent::new(ai, 0),
            burst_heal_area: Pattern::Single,
            c1_msg: false,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        if asc >= 1 {
            s.subscribe(EventKind::TargetDied, o);
        }
        if asc >= 4 {
            let (mut heal, mut cryo) = (0, 0);
            for i in 0..s.chars.len() {
                if i == ci {
                    continue;
                }
                let name = crate::gcsl::char_name(s.chars[i].key);
                let zone = crate::data::CHARS.iter().find(|d| d.key == name).map(|d| d.zone);
                if zone == Some(ZONE_FONTAINE) {
                    heal += 1;
                } else {
                    cryo += 1;
                }
            }
            let st = s.st_mut::<State>(ci);
            st.a4_heal = heal;
            st.a4_cryo = cryo;
            s.add_mod(ci, Mod::owned(A4, ModKind::Stat, o, st::NONE, false, false, -1), -1);
        }
        if cons >= 1 {
            s.subscribe(EventKind::Heal, o);
        }
        if cons >= 4 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        if cons >= 6 {
            s.subscribe(EventKind::EnemyDamage, o);
        }
    }

    /// gcsim validates hold on the skill and collision on the plunges, and
    /// reads no params of the other actions.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Burst => None,
            Action::Skill => p.0.iter().find(|(k, _)| k != "hold").map(|(k, _)| format!("{name} param {k}")),
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            _ => Some(name.into()),
        }
    }

    /// Plunges only while airborne (Xianyun's jump).
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.circle_on_target(g::ATTACK_RADIUS[n]);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], g::ATTACK_EARLIEST_CANCEL[n], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let windup = match s.current_state() {
            AnimState::Normal => 14,
            AnimState::Skill if s.st::<State>(ci).last_hold == 0 => 8,
            AnimState::Burst => 3,
            _ => 0,
        };
        // the enemy's position as she casts it
        s.st_mut::<State>(ci).charge_area = s.circle_on_target(g::CHARGE_RADIUS);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CHARGE, arg: 0 }, g::CHARGE_HITMARK - windup);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: FRAMES_CHARGE, arg: windup as i16 }, t[0] - windup, t[Action::Dash as usize] - windup, AnimState::Charge)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FRAMES_CHARGE => g::CHARGE_FRAMES[next as usize] - arg as i32,
            FRAMES_HOLD => arg as i32 + g::SKILL_HOLD_FRAMES[next as usize],
            _ => unreachable!(),
        }
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(0);
        let lvl = s.talent(ci, 1);
        let c2 = if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
        {
            let st = s.st_mut::<State>(ci);
            st.mark_count = 0;
            st.last_hold = hold;
            // makeC2CB
            if c2 != 0 {
                st.c2_hits = 0;
            }
        }
        if hold == 0 {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL_PRESS[lvl]);
            ai.abil = "Framing: Freezing Point Composition";
            ai.pattern = s.box_on_player(0.0, 0.0, g::SKILL_PRESS_BOX_X as f64, g::SKILL_PRESS_BOX_Y as f64);
            s.queue_attack(ai, CB_PRESS_PARTICLE | CB_PRESS_MARK | c2, 0, g::SKILL_PRESS_HITMARK);
            s.set_cd_with_delay(ci, Action::Skill, g::SKILL_PRESS_CD, g::SKILL_PRESS_DELAY);
            let t = &g::SKILL_PRESS_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill);
        }
        // hold=1 gives a hitmark of 111, hold=809 one of 919
        let hold = hold.clamp(1, 809) as i32;
        let hitmark = hold + g::SKILL_HOLD_HITMARK;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL_HOLD[lvl]);
        ai.abil = "Framing: Freezing Point Composition (Hold)";
        ai.pattern = s.box_on_player(g::SKILL_HOLD_OFFSET_X, 0.0, g::SKILL_HOLD_BOX_X, g::SKILL_HOLD_BOX_Y as f64);
        s.queue_attack(ai, CB_HOLD_PARTICLE | CB_HOLD_MARK | c2, 0, hitmark);
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_HOLD_CD, hitmark - 2);
        let t = &g::SKILL_HOLD_FRAMES;
        ActInfo::new(Frames::Char { id: FRAMES_HOLD, arg: hold as i16 }, hold + t[0], hold + t[Action::Burst as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 50.0, g::BURST[lvl]);
        ai.abil = g::BURST_INITIAL_ABIL;
        ai.pattern = s.circle_on_player(g::BURST_OFFSET_X as f64, g::BURST_OFFSET_Y, g::BURST_ATTACK_RADIUS as f64);
        let heal_area = s.circle_on_player(g::BURST_OFFSET_X as f64, g::BURST_OFFSET_Y, g::BURST_HEAL_RADIUS as f64);
        // c.Snapshot(&ai): the heals and the ticks are on it
        let snap = s.snapshot_event(ai, 0);
        let healp = snap.stats[st::HEAL];
        let atk = total_atk(&snap.stats);
        let heal = g::BURST_INITIAL_HEAL_FLAT[lvl] + atk * g::BURST_INITIAL_HEAL_PER[lvl];
        let heal_dot = g::BURST_DOT_HEAL_FLAT[lvl] + atk * g::BURST_DOT_HEAL_PER[lvl];
        s.queue_attack(ai, 0, 0, g::BURST_START);
        let mut tick = snap;
        tick.info.abil = g::BURST_DOT_ABIL;
        tick.info.mult = g::BURST_DOT[lvl];
        tick.info.icd_tag = icd::TAG_CHARLOTTE_KAMERA;
        tick.info.icd_group = icd::GROUP_CHARLOTTE_KAMERA;
        tick.info.durability = 25.0;
        let st = s.st_mut::<State>(ci);
        st.burst_heal = heal;
        st.burst_heal_dot = heal_dot;
        st.burst_healp = healp;
        st.burst_tick = tick;
        st.burst_heal_area = heal_area;
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_START, arg: 0 }, g::BURST_START);
        s.consume_energy(ci, g::BURST_CONSUME_DELAY);
        s.set_cd(ci, Action::Burst, g::BURST_CD);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_CHARGE => {
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::CHARGE[lvl]);
                ai.abil = "Charge Attack";
                ai.pattern = s.st::<State>(ci).charge_area;
                s.queue_attack(ai, 0, 0, 0);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ARKHE, arg: 0 }, 30);
            }
            TASK_ARKHE => {
                if s.mod_active(ci, ARKHE_ICD) {
                    return;
                }
                s.add_status(ci, ARKHE_ICD, 6 * 60, true);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 0.0, g::ARKHE[lvl]);
                ai.abil = "Spiritbreath Thorn (charlotte)";
                ai.ignore_infusion = true;
                // the charged attack's position
                ai.pattern = s.st::<State>(ci).charge_area;
                if let Pattern::Circle { x, y, .. } = ai.pattern {
                    ai.pattern = Pattern::Circle { x, y, r: g::ARKHE_RADIUS };
                }
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_MARK => {
                let (src, hold) = (arg >> 1, arg & 1 == 1);
                let key = if hold { HOLD_MARK } else { PRESS_MARK };
                if src != s.st::<State>(ci).mark_tag || !s.enemy_status_active(key) {
                    return;
                }
                let lvl = s.talent(ci, 1);
                let mult = if hold { g::SKILL_HOLD_MARK[lvl] } else { g::SKILL_PRESS_MARK[lvl] };
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_CHARLOTTE_MARK, icd::GROUP_CHARLOTTE_MARK, Element::Cryo, 25.0, mult);
                ai.abil = if hold { "Focused Impression Mark" } else { "Snappy Silhouette Mark" };
                ai.pattern = Pattern::Single;
                s.queue_attack(ai, 0, 0, 0);
                s.add_task(Task::Char { c: ci as u8, id: TASK_MARK, arg }, 90);
            }
            TASK_BURST_START => {
                let st = s.st::<State>(ci);
                let (heal, healp) = (st.burst_heal, st.burst_healp);
                own_heal(s, ci, None, heal, healp);
                for &v in g::BURST_TICK_FRAMES {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_TICK, arg: 0 }, v - g::BURST_START);
                }
            }
            TASK_BURST_TICK => {
                let st = s.st::<State>(ci);
                let (tick, area, heal, healp) = (st.burst_tick, st.burst_heal_area, st.burst_heal_dot, st.burst_healp);
                s.queue_attack_with_snap(tick, 0, 0);
                if !s.player_within(area) {
                    return;
                }
                let active = s.player.active as usize;
                own_heal(s, ci, Some(active), heal, healp);
            }
            TASK_A1_RESET => s.st_mut::<State>(ci).a1_count = 0,
            TASK_C1_HEAL => c1_heal(s, ci, arg as usize),
            TASK_C4_RESET => s.st_mut::<State>(ci).c4_count = 0,
            TASK_C6 => {
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 1.8);
                ai.abil = "A Summation of Interest (C6)";
                ai.hitlag_factor = 0.02;
                ai.def_halt = true;
                ai.deployable = true;
                // the enemy's position as it was damaged
                ai.pattern = s.circle_on_target(g::C6_ATTACK_RADIUS);
                s.queue_attack(ai, 0, 0, 0);
                if s.player_within(s.circle_on_target(g::C6_HEAL_RADIUS as f64)) {
                    let amt = s.total_atk(ci) * 0.42;
                    let bonus = s.stat(ci, st::HEAL);
                    let active = s.player.active as usize;
                    own_heal(s, ci, Some(active), amt, bonus);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PRESS_PARTICLE != 0 {
            s.queue_particle(g::SKILL_PRESS_PARTICLE_COUNT as f64, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_HOLD_PARTICLE != 0 {
            s.queue_particle(g::SKILL_HOLD_PARTICLE_COUNT as f64, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & (CB_PRESS_MARK | CB_HOLD_MARK) != 0 {
            mark_target(s, ci, cbs & CB_HOLD_MARK != 0);
        }
        if cbs & CB_C2 != 0 {
            let st = s.st_mut::<State>(ci);
            if st.c2_hits < 3 {
                st.c2_hits += 1;
                let v = 0.1 * st.c2_hits as f64;
                s.add_mod(ci, Mod::stat(C2, st::ATKP, v, false, true, -1), 12 * 60);
            }
        }
    }

    /// A4: healing bonus +5% for each other member from Fontaine, cryo DMG
    /// +5% for each from elsewhere.
    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key != A4 {
            return false;
        }
        let st = s.st::<State>(ci);
        out[st::HEAL] += 0.05 * st.a4_heal as f64;
        out[st::CRYOP] += 0.05 * st.a4_cryo as f64;
        true
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // A1: a kill of a held-marked enemy, up to 4 in 12 s, the
            // skill's cooldown -2 s
            Event::TargetDied(_) => {
                if !s.enemy_status_active(HOLD_MARK) || s.st::<State>(ci).a1_count == 4 {
                    return;
                }
                if s.st::<State>(ci).a1_count == 0 {
                    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1_RESET, arg: 0 }, 720);
                }
                s.st_mut::<State>(ci).a1_count += 1;
                s.reduce_cd(ci, Action::Skill, 120);
            }
            // C1: her burst's and C6's heals start Verification's heals
            Event::Heal { caller, on, .. } => {
                if caller != ci || !s.st::<State>(ci).c1_msg || s.mod_active(on, C1) {
                    return;
                }
                s.add_status(on, C1, 6 * 60, true);
                c1_heal(s, ci, on);
            }
            // C4: the burst's hits on a marked enemy, up to 5 in 20 s, DMG
            // +10% and 2 energy
            Event::EnemyHit(ev) => {
                if s.st::<State>(ci).c4_count == 5 {
                    return;
                }
                if !s.enemy_status_active(PRESS_MARK) && !s.enemy_status_active(HOLD_MARK) {
                    return;
                }
                if ev.info.abil != g::BURST_INITIAL_ABIL && ev.info.abil != g::BURST_DOT_ABIL {
                    return;
                }
                if s.st::<State>(ci).c4_count == 0 {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_C4_RESET, arg: 0 }, 1200);
                }
                s.hit_dmgp += 0.1;
                s.add_energy(ci, 2.0);
                s.st_mut::<State>(ci).c4_count += 1;
            }
            // C6: the active character's normal or charged attack on a
            // held-marked enemy, once every 6 s
            Event::EnemyDamage(ev) => {
                if s.mod_active(ci, C6_ICD) || !s.enemy_status_active(HOLD_MARK) {
                    return;
                }
                if ev.info.actor as usize != s.player.active as usize || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
                    return;
                }
                s.add_status(ci, C6_ICD, 6 * 60, true);
                s.add_task(Task::Char { c: ci as u8, id: TASK_C6, arg: 0 }, 14);
            }
            _ => {}
        }
    }
}

/// A heal of hers with a message C1 reads (the burst's and C6's).
fn own_heal(s: &mut Sim, ci: usize, target: Option<usize>, src: f64, bonus: f64) {
    let prev = s.st::<State>(ci).c1_msg;
    s.st_mut::<State>(ci).c1_msg = true;
    s.heal_info(ci, target, false, src, bonus);
    s.st_mut::<State>(ci).c1_msg = prev;
}

/// c1Heal: 80% of her ATK on the character every 2 s while C1's status on
/// it lasts (its own queue).
fn c1_heal(s: &mut Sim, ci: usize, on: usize) {
    if !s.mod_active(on, C1) {
        return;
    }
    let amt = s.total_atk(ci) * 0.8;
    let bonus = s.stat(ci, st::HEAL);
    let prev = s.st::<State>(ci).c1_msg;
    s.st_mut::<State>(ci).c1_msg = false;
    s.heal(ci, on, amt, bonus);
    s.st_mut::<State>(ci).c1_msg = prev;
    s.queue_char_task(on, Task::Char { c: ci as u8, id: TASK_C1_HEAL, arg: on as i32 }, 2 * 60);
}

/// skillPressMarkTargets and skillHoldMarkTargets: up to 5 enemies per
/// skill, the mark replacing the other, ticking every 1.5 s.
fn mark_target(s: &mut Sim, ci: usize, hold: bool) {
    if s.st::<State>(ci).mark_count == 5 {
        return;
    }
    s.st_mut::<State>(ci).mark_count += 1;
    if s.enemy_status_active(PRESS_MARK) {
        s.enemy_delete_status(PRESS_MARK);
    }
    if s.enemy_status_active(HOLD_MARK) {
        s.enemy_delete_status(HOLD_MARK);
    }
    let f = s.f;
    s.st_mut::<State>(ci).mark_tag = f;
    if hold {
        s.enemy_add_status(HOLD_MARK, 720 + 54, true);
    } else {
        s.enemy_add_status(PRESS_MARK, 360 + 48, true);
    }
    s.queue_enemy_task(Task::Char { c: ci as u8, id: TASK_MARK, arg: f << 1 | hold as i32 }, 90);
}
