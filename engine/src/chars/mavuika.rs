//! Mavuika, from gcsim's internal/characters/mavuika: Fighting Spirit (her
//! burst's resource, from the party's Nightsoul points and normal attacks),
//! her Nightsoul's Blessing (no end of its own: points drain every 6 frames
//! on her queue, faster on Flamestrider and in its charged attack, none
//! during the burst), the skill's two armaments (the Rings of Searing
//! Radiance, which strike every 2 s, and Flamestrider, with its own normal
//! attacks, sprint, jump, plunge and spinning charged attack), the skill's
//! hold and recast, the burst (Sunfell Slice, Crucible of Death and Life's
//! bonuses by the Fighting Spirit spent), A1, A4, her normal and charged
//! attacks and plunges out of Flamestrider, and constellations 1 to 6.
//!
//! Flamestrider's charged attack follows gcsim's model of its spin: the
//! hitbox (radius 3, 4 from her) turns once every 45 frames, and each
//! target in reach (the enemy, dendro cores and the Bogglecat Box) is hit
//! on the frames of the spin that cross it, at most once every 42 frames;
//! a hold, the release (`final`), the windup a previous action skips
//! (`buffered`), the finisher when Nightsoul or the 375 frames run out, and
//! the dash that cancels it (`charge_cancel`) are gcsim's.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::mavuika as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

/// A target of the spinning charged attack: the enemy (-1) or a gadget's
/// key.
type Target = i64;
const ENEMY: Target = -1;

#[derive(Clone, Copy, Default)]
struct Hittable {
    target: Target,
    one_tick: bool,
    coll: [i32; 2],
}

/// The hittable targets, at most the enemy and the gadgets.
struct Hittables {
    list: [Hittable; 16],
    len: usize,
}

impl Hittables {
    fn new() -> Self {
        Hittables { list: [Hittable::default(); 16], len: 0 }
    }
    fn push(&mut self, h: Hittable) {
        self.list[self.len] = h;
        self.len += 1;
    }
    fn iter(&self) -> impl Iterator<Item = &Hittable> {
        self.list[..self.len].iter()
    }
}

/// Frames on which a spin hits, at most a handful.
struct HitFrames {
    list: [i32; 32],
    len: usize,
}

/// gcsim's ChargeState.
#[derive(Default)]
struct ChargeState {
    start_frame: i32,
    c_atk_frames: i32,
    skipped_windup_f: i32,
    /// LastHit by target (a missing target reads 0).
    last_hit: Vec<(Target, i32)>,
    facing: f64,
    src_frame: i32,
}

impl ChargeState {
    fn reset(&mut self) {
        self.start_frame = 0;
        self.c_atk_frames = 0;
        self.skipped_windup_f = 0;
        self.last_hit.clear();
        self.facing = 0.0;
        self.src_frame = 0;
    }
    fn last_hit(&self, t: Target) -> i32 {
        self.last_hit.iter().find(|x| x.0 == t).map_or(0, |x| x.1)
    }
    fn add_last_hit(&mut self, t: Target, v: i32) {
        match self.last_hit.iter_mut().find(|x| x.0 == t) {
            Some(x) => x.1 += v,
            None => self.last_hit.push((t, v)),
        }
    }
}

pub struct State {
    fighting_spirit: f64,
    nightsoul_src: i32,
    bike: bool,
    ring_src: i32,
    burst_stacks: f64,
    a4_src: i32,
    a4_stacks: i32,
    c6_src: i32,
    saved_normal_counter: i32,
    ca: ChargeState,
    can_bike_plunge: bool,
    charge_cancel: bool,
    /// The OnDendroCore hook of the spinning charged attack is subscribed.
    ca_hooked: bool,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const ENERGY_NA_ICD: ModKey = ModKey::new(g::ENERGY_NAICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const SKILL_RECAST_CD: ModKey = ModKey::new(g::SKILL_RECAST_CD_KEY);
const CDC_LOCKOUT: ModKey = ModKey::new(g::CDC_LOCKOUT_STATUS);
const A1: ModKey = ModKey::new(g::A1_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const A4_BUF: ModKey = ModKey::new(g::A4_BUF_KEY);
const C1: ModKey = ModKey::new(g::C1_KEY);
const C2_BASE_ATK: ModKey = ModKey::new("mavuika-c2-base-atk");
const C2_DEF: ModKey = ModKey::new("mavuika-c2");
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_C6_RING: u16 = 1 << 1;

// tasks; arg is the src where gcsim checks one
const TASK_REDUCE: u8 = 0;
const TASK_RING: u8 = 1;
const TASK_A4_DECAY: u8 = 2;
const TASK_BURST_NS: u8 = 3;
const TASK_BURST_STATUS: u8 = 4;
const TASK_BURST_HIT: u8 = 5;
const TASK_RECAST_STATUS: u8 = 6;
const TASK_C2_RING: u8 = 7;
const TASK_C6_RING: u8 = 8;
/// arg: the combo's index, plus 8 with the burst's hitboxes.
const TASK_BIKE_NA: u8 = 9;
/// arg: the target (-1 for the enemy, else a gadget's key).
const TASK_CA_HIT: u8 = 10;
const TASK_CA_AUTO_FINAL: u8 = 11;
const TASK_CAF_HIT: u8 = 12;
const TASK_CAF_NS_EXIT: u8 = 13;
const TASK_CA_UNHOOK: u8 = 14;
const TASK_CDC_LOCKOUT: u8 = 15;
const TASK_BIKE_PLUNGE_OFF: u8 = 16;
const TASK_DASH_HIT: u8 = 17;

// frames ids
const FR_SKILL: u8 = 0;
const FR_SKILL_HOLD: u8 = 1;
const FR_SKILL_REFRESH: u8 = 2;
/// arg: travel, plus 256 with charge_cancel.
const FR_DASH: u8 = 3;
/// arg: durationCA * 32 + the finisher's delay (under 32).
const FR_BIKE_CHARGE: u8 = 4;
/// arg: caFrames.
const FR_BIKE_CHARGE_FINAL: u8 = 5;

const REMOVED_BIKE_CHARGE: u8 = 0;

const NORMAL_NAMES_RING: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];
const BIKE_NAMES: [&str; 5] = ["Flamestrider Normal 0", "Flamestrider Normal 1", "Flamestrider Normal 2", "Flamestrider Normal 3", "Flamestrider Normal 4"];

/// The plunges out of Flamestrider, from Xianyun's airborne state (or the
/// bike's jump once Nightsoul ran out): physical.
const PLUNGE: plunge::Spec = plunge::Spec::physical(
    g::LOW_PLUNGE_HITMARK,
    g::HIGH_PLUNGE_HITMARK,
    g::COLLISION_HITMARK,
    &g::LOW_PLUNGE_FRAMES,
    &g::HIGH_PLUNGE_FRAMES,
    g::LOW_PLUNGE_POISE_DMG,
    g::HIGH_PLUNGE_POISE_DMG,
    g::LOW_PLUNGE_RADIUS,
    g::HIGH_PLUNGE_RADIUS,
    g::LOW_PLUNGE,
    g::HIGH_PLUNGE,
    g::COLLISION,
);

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn on_bike_ns(s: &Sim, ci: usize) -> bool {
    st(s, ci).bike && s.ns_has_blessing(ci)
}

fn char_task(s: &mut Sim, ci: usize, id: u8, arg: i32, delay: i32) {
    s.queue_char_task(ci, Task::Char { c: ci as u8, id, arg }, delay);
}

fn core_task(s: &mut Sim, ci: usize, id: u8, arg: i32, delay: i32) {
    s.add_task(Task::Char { c: ci as u8, id, arg }, delay);
}

fn fail(s: &mut Sim, what: &str) {
    if s.error.is_none() {
        s.error = Some(format!("mavuika: {what}"));
    }
}

// ---------------------------------------------------------------- nightsoul

/// gcsim's reduceNightsoulPoints: none during the burst.
fn reduce_points(s: &mut Sim, ci: usize, val: f64) {
    if s.mod_active(ci, BURST) || val == 0.0 {
        return;
    }
    s.ns_consume(ci, val);
    if s.ns_points(ci) < 0.001 {
        exit_nightsoul(s, ci);
    }
}

fn exit_nightsoul(s: &mut Sim, ci: usize) {
    if !s.ns_has_blessing(ci) {
        return;
    }
    s.ns_exit(ci);
    s.ns_clear(ci);
    st_mut(s, ci).nightsoul_src = -1;
    let n = if st(s, ci).bike && s.current_state() == AnimState::Normal { st(s, ci).saved_normal_counter.min(3) } else { 0 };
    s.chars[ci].normal_counter = n;
    s.chars[ci].normal_hit_num = g::NORMAL_HIT_NUM;
}

/// gcsim's enterNightsoulOrRegenerate.
fn enter_or_regenerate(s: &mut Sim, ci: usize, points: f64) {
    if !s.ns_has_blessing(ci) {
        s.ns_enter(ci, points);
        let f = s.f;
        st_mut(s, ci).nightsoul_src = f;
        char_task(s, ci, TASK_REDUCE, f, 6);
        return;
    }
    s.ns_generate(ci, points);
}

// ---------------------------------------------------------------- armaments

fn enter_bike(s: &mut Sim, ci: usize) {
    st_mut(s, ci).bike = true;
    s.chars[ci].normal_hit_num = g::BIKE_HIT_NUM;
    s.chars[ci].normal_counter = 0;
    // C6: the rings strike from Flamestrider too
    if s.chars[ci].cons >= 6 {
        let f = s.f;
        st_mut(s, ci).c6_src = f;
        char_task(s, ci, TASK_C6_RING, f, 180);
    }
}

fn exit_bike(s: &mut Sim, ci: usize) {
    let f = s.f;
    let st = st_mut(s, ci);
    st.bike = false;
    st.ring_src = f;
    s.chars[ci].normal_hit_num = g::NORMAL_HIT_NUM;
    char_task(s, ci, TASK_RING, f, 2 * 60);
    c2_ring(s, ci);
}

/// gcsim's isRingFollowing (for C2).
fn ring_following(s: &Sim, ci: usize) -> bool {
    if s.chars[ci].cons < 6 {
        return !st(s, ci).bike && s.ns_has_blessing(ci);
    }
    true
}

/// C2: enemies near the rings lose 20% DEF, every 0.3 s while they follow.
fn c2_ring(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 2 || !ring_following(s, ci) {
        return;
    }
    if s.enemy_within(s.circle_on_player(0.0, 1.0, 6.0)) {
        s.enemy_add_def(C2_DEF, -0.2, 30);
    }
    char_task(s, ci, TASK_C2_RING, 0, 18);
}

fn c2_flat(s: &Sim, ci: usize, k: f64) -> f64 {
    if s.chars[ci].cons < 2 || !st(s, ci).bike {
        return 0.0;
    }
    k * s.total_atk(ci)
}

// ---------------------------------------------------------------- fighting spirit and the burst

fn gain_fighting_spirit(s: &mut Sim, ci: usize, val: f64) {
    let eff = if s.chars[ci].cons >= 1 { 1.25 } else { 1.0 };
    let st = st_mut(s, ci);
    st.fighting_spirit = (st.fighting_spirit + val * eff).min(g::MAX_FIGHTING_SPIRIT as f64);
    // C1: 40% ATK for 10 s
    if s.chars[ci].cons >= 1 {
        s.add_stat_mod(ci, C1, st::ATKP, 0.4, 10 * 60, true);
    }
}

/// The burst's bonus to an attack, by the Fighting Spirit it spent.
fn burst_bonus(s: &Sim, ci: usize, table: &[f64]) -> f64 {
    if !s.mod_active(ci, BURST) {
        return 0.0;
    }
    st(s, ci).burst_stacks * table[s.talent(ci, 2)] * s.total_atk(ci)
}

// ---------------------------------------------------------------- the spinning charged attack

/// gcsim's GetRemainingNightSoulDuration: 1.1 points every 6 frames.
fn remaining_ns_duration(s: &Sim, ci: usize) -> i32 {
    let since = (s.f - st(s, ci).nightsoul_src) % 6;
    let mut d = (s.ns_points(ci) / 1.1).ceil() as i32 * 6 - since;
    if s.mod_active(ci, BURST) {
        d += s.mod_duration(ci, BURST.name);
    }
    d
}

/// gcsim's GetCAFDelay: the 20 frames of a spin where the finisher cannot
/// start.
fn caf_delay(spin_frame: i32) -> i32 {
    if spin_frame < 10 {
        10 - spin_frame
    } else if spin_frame >= 35 {
        55 - spin_frame
    } else {
        0
    }
}

/// gcsim's DirectionOffsetToPrimaryTarget.
fn direction_offset_to_target(s: &Sim) -> f64 {
    let d = calc_direction(s.player_pos, s.enemy_pos);
    if d == DEFAULT_DIR {
        return 0.0;
    }
    let angle = d.0.atan2(d.1).to_degrees();
    (360.0 - angle + 360.0) % 360.0
}

/// gcsim's ConvertAngleToFrame.
fn angle_to_frame(theta: f64) -> i32 {
    let theta = (theta + 360.0) % 360.0;
    let fr = g::BIKE_SPIN_QUADRANT_FRAMES;
    let (quadrant, spin, mut acc) = if theta >= 270.0 || theta < 0.0 {
        (3, 0, 0)
    } else if theta >= 180.0 {
        (2, 1, fr[0])
    } else if theta >= 90.0 {
        (1, 2, fr[1] + fr[0])
    } else {
        (0, 3, fr[2] + fr[1] + fr[0])
    };
    if acc > 0 {
        acc -= 1;
    }
    let start = quadrant as f64 * 90.0;
    let offset = fr[spin] as f64 + (theta - start) / g::BIKE_SPIN_QUADRANT_ANGULAR_VELOCITY[spin];
    acc + offset.round() as i32
}

/// gcsim's BikeHitboxIntersectionAngles for a circle at (x, y) of radius
/// r: the spin's frames that cross it, None when it never does.
fn intersection_frames(s: &Sim, x: f64, y: f64, r: f64, offset: f64) -> Option<[i32; 2]> {
    let hr = g::BIKE_CHARGE_ATTACK_HITBOX_RADIUS as f64;
    let so = g::BIKE_CHARGE_ATTACK_SPIN_OFFSET;
    let (inner, outer) = (so - hr, so + hr);
    let (px, py) = (x - s.player_pos.0, y - s.player_pos.1);
    let dist = (px * px + py * py).sqrt();
    if dist + r <= inner || dist - r >= outer {
        return None;
    }
    if r - dist > inner {
        return Some([0, g::BIKE_CHARGE_ATTACK_SPIN_FRAMES]);
    }
    let sum = hr + r;
    let cos_m = (so * so + dist * dist - sum * sum) / (2.0 * so * dist);
    let angle = py.atan2(px).to_degrees();
    let theta_m = cos_m.acos().to_degrees();
    let angle = (angle - offset + 360.0) % 360.0;
    Some([angle_to_frame(angle + theta_m), angle_to_frame(angle - theta_m)])
}

/// The facing the spin is measured from: taken at the start of the
/// charged attack.
fn facing(s: &mut Sim, ci: usize) -> f64 {
    if st(s, ci).ca.c_atk_frames == 0 {
        let f = direction_offset_to_target(s);
        st_mut(s, ci).ca.facing = f;
        f
    } else {
        st(s, ci).ca.facing
    }
}

/// gcsim's BuildBikeChargeAttackHittableTargetList: the enemy within 8,
/// then the dendro cores and Bogglecat Boxes within 8.
fn hittables(s: &mut Sim, ci: usize) -> Hittables {
    let mut out = Hittables::new();
    let area = s.circle_on_player(0.0, 0.0, 8.0);
    if s.enemy_alive && s.enemy_within(area) {
        let off = facing(s, ci);
        if let Some(coll) = intersection_frames(s, s.enemy_pos.0, s.enemy_pos.1, s.target_radius, off) {
            out.push(Hittable { target: ENEMY, one_tick: false, coll });
        }
    }
    for k in 0..s.gadgets.len() {
        let gd = s.gadgets[k];
        if !gd.enemy_camp || area.contains_point(gd.x, gd.y) != Some(true) {
            continue;
        }
        if let Some(h) = gadget_hittable(s, ci, &gd) {
            out.push(h);
        }
    }
    out
}

/// gcsim's IsGadgetHittable.
fn gadget_hittable(s: &mut Sim, ci: usize, gd: &Gadget) -> Option<Hittable> {
    let off = facing(s, ci);
    intersection_frames(s, gd.x, gd.y, gd.r, off).map(|coll| Hittable { target: gd.key as Target, one_tick: true, coll })
}

/// gcsim's CalculateValidCollisionFrames.
fn collision_frames(s: &Sim, ci: usize, duration: i32, coll: [i32; 2], last_hit: i32) -> HitFrames {
    let ca = &st(s, ci).ca;
    let spin = g::BIKE_CHARGE_ATTACK_SPIN_FRAMES;
    let icd = g::BIKE_CHARGE_ATTACK_ICD;
    let mut out = HitFrames { list: [0; 32], len: 0 };
    let mut current = g::BIKE_CHARGE_ATTACK_STARTUP_HITMARK;
    let since_start = s.f - (ca.start_frame - ca.skipped_windup_f);
    let since_last = since_start - last_hit;
    if since_start >= current {
        current = since_start;
        if since_last < icd {
            current += icd - since_last;
        }
    }
    let mut total = current;
    current %= spin;
    let (cs, ce) = (coll[0], coll[1]);
    while total <= duration + ca.c_atk_frames {
        let mut valid = -1;
        if cs <= ce {
            if current > ce {
                current -= spin;
                total += cs - current;
                current = cs;
            } else if current < cs {
                total += cs - current;
                current = cs;
            }
        } else if current < cs && current > ce {
            total += cs - current;
            current = cs;
        }
        if cs <= ce {
            if current >= cs && current <= ce {
                valid = total - since_start;
            }
        } else if current >= cs || current <= ce {
            valid = total - since_start;
        }
        if s.f == ca.start_frame {
            valid += ca.skipped_windup_f;
        }
        if valid >= 0 && valid <= duration && out.len < out.list.len() {
            out.list[out.len] = valid;
            out.len += 1;
        }
        total += icd;
        current = (current + icd) % spin;
    }
    out
}

/// Queues a spin's hit on a target, on the fight's queue.
fn queue_ca_hit(s: &mut Sim, ci: usize, t: Target, delay: i32) {
    core_task(s, ci, TASK_CA_HIT, t as i32, delay);
}

/// gcsim's HoldBikeChargeAttack.
fn hold_ca(s: &mut Sim, ci: usize, frames: i32, skipped: i32, hs: &Hittables) {
    for h in hs.iter() {
        let last = st(s, ci).ca.last_hit(h.target);
        if h.one_tick && last > 0 {
            continue;
        }
        let hits = collision_frames(s, ci, frames, h.coll, last);
        let mut new_last = -1;
        for &f in &hits.list[..hits.len] {
            queue_ca_hit(s, ci, h.target, f - skipped);
            new_last = f;
        }
        if new_last >= 0 {
            let c_atk = st(s, ci).ca.c_atk_frames;
            st_mut(s, ci).ca.add_last_hit(h.target, new_last + (c_atk - last));
        }
    }
}

/// gcsim's CountBikeChargeAttack: the hits on all targets until the
/// primary target's `max_hits`-th, and that duration.
fn count_ca(s: &mut Sim, ci: usize, max_hits: i32, skipped: i32, hs: &Hittables, ns_dur: i32) -> i32 {
    let c_atk = st(s, ci).ca.c_atk_frames;
    let mut dur = (ns_dur + skipped).min(g::BIKE_CHARGE_ATTACK_MAXIMUM_DURATION - c_atk);
    if st(s, ci).ca.start_frame == s.f {
        dur = dur.max(g::BIKE_CHARGE_ATTACK_MINIMUM_DURATION);
    }
    let mut count = 0;
    for h in hs.iter() {
        if h.target != ENEMY {
            continue;
        }
        let last = st(s, ci).ca.last_hit(h.target);
        if h.one_tick && last > 0 {
            continue;
        }
        let hits = collision_frames(s, ci, dur, h.coll, last);
        for &f in &hits.list[..hits.len] {
            count += 1;
            if count >= max_hits {
                dur = f;
                break;
            }
        }
        if count >= max_hits {
            break;
        }
    }
    for h in hs.iter() {
        let last = st(s, ci).ca.last_hit(h.target);
        let hits = collision_frames(s, ci, dur, h.coll, last);
        let mut new_last = -1;
        for &f in &hits.list[..hits.len] {
            queue_ca_hit(s, ci, h.target, f - skipped);
            new_last = f;
        }
        if new_last >= 0 {
            let c_atk = st(s, ci).ca.c_atk_frames;
            st_mut(s, ci).ca.add_last_hit(h.target, new_last + (c_atk - last));
        }
    }
    dur
}

/// gcsim's GetSkippedWindupFrames.
fn skipped_windup(s: &mut Sim, ci: usize, buffered: i32) -> i32 {
    let max = g::MAX_BUFFERED_BIKE_CHARGE_FRAMES;
    let state = s.current_state();
    let skipped = match state {
        AnimState::Dash => {
            if st(s, ci).charge_cancel {
                st_mut(s, ci).charge_cancel = false;
                return max;
            }
            max
        }
        AnimState::Normal => max,
        AnimState::Charge if st(s, ci).ca.start_frame == s.f => max,
        AnimState::Burst => {
            if buffered == 0 {
                0
            } else {
                max
            }
        }
        AnimState::Skill if s.mod_active(ci, SKILL_RECAST_CD) => {
            if s.mod_duration(ci, SKILL_RECAST_CD.name) > 45 {
                13
            } else {
                max
            }
        }
        AnimState::Plunge => 13,
        _ => 0,
    };
    skipped.min(buffered)
}

/// gcsim's BikeCharge.
fn bike_charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
    let mut duration = p.get("hold").unwrap_or(0) as i32;
    let fin = p.get("final").unwrap_or(0);
    let buffered = p.get("buffered").map_or(g::MAX_BUFFERED_BIKE_CHARGE_FRAMES, |v| v as i32);
    let hs = hittables(s, ci);
    let mut skipped = 0;
    if s.current_state() != AnimState::Charge || st(s, ci).ca.start_frame == 0 {
        let f = s.f;
        let st = st_mut(s, ci);
        st.ca.reset();
        st.ca.start_frame = f;
        for h in hs.iter() {
            st.ca.last_hit.push((h.target, 0));
        }
        st.ca_hooked = true;
        skipped = skipped_windup(s, ci, buffered);
        // a windup not wholly skipped counts as a normal attack for n0
        // abilities (Xingqiu's, Yelan's)
        if skipped < 15 {
            s.emit(Event::StateChange(AnimState::Normal));
        }
        st_mut(s, ci).ca.skipped_windup_f = skipped;
        s.delete_mod(ci, CDC_LOCKOUT);
        char_task(s, ci, TASK_CDC_LOCKOUT, f, 79 - skipped);
    }
    let f = s.f;
    st_mut(s, ci).ca.src_frame = f;
    let src = f;
    let ns_dur = remaining_ns_duration(s, ci);
    if fin == 1 {
        return bike_charge_final(s, ci, 0, skipped);
    }
    let c_atk = st(s, ci).ca.c_atk_frames;
    if duration > 0 && c_atk > 0 {
        duration = duration.min(g::BIKE_CHARGE_ATTACK_SPIN_FRAMES).min(ns_dur).min(g::BIKE_CHARGE_ATTACK_MAXIMUM_DURATION - c_atk);
        hold_ca(s, ci, duration, skipped, &hs);
    } else {
        if hs.len == 0 {
            fail(s, "no valid targets within flamestrider area");
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Charge);
        }
        if !hs.iter().any(|h| h.target == ENEMY) {
            fail(s, "primary target is not within flamestrider area");
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Charge);
        }
        duration = count_ca(s, ci, 1, skipped, &hs, ns_dur);
    }
    st_mut(s, ci).ca.c_atk_frames += duration;
    duration -= skipped;
    let c_atk = st(s, ci).ca.c_atk_frames;
    if duration >= ns_dur || c_atk >= g::BIKE_CHARGE_ATTACK_MAXIMUM_DURATION {
        return bike_charge_final(s, ci, duration, skipped);
    }
    let min_spin = caf_delay(c_atk % g::BIKE_CHARGE_ATTACK_SPIN_FRAMES);
    char_task(s, ci, TASK_CA_AUTO_FINAL, src, duration + 1);
    let mut info = ActInfo::new(
        Frames::Char { id: FR_BIKE_CHARGE, arg: (duration * 32 + min_spin) as i16 },
        duration + min_spin + g::BIKE_CHARGE_FINAL_FRAMES[0],
        duration,
        AnimState::Charge,
    );
    info.on_removed = OnRemoved::Char(REMOVED_BIKE_CHARGE);
    info
}

/// gcsim's BikeChargeAttackFinal.
fn bike_charge_final(s: &mut Sim, ci: usize, ca_frames: i32, skipped: i32) -> ActInfo {
    let elapsed = st(s, ci).ca.c_atk_frames + ca_frames;
    let min_spin = if elapsed > 0 { caf_delay(elapsed % g::BIKE_CHARGE_ATTACK_SPIN_FRAMES) } else { g::BIKE_CHARGE_ATTACK_MINIMUM_DURATION };
    let ca_frames = ca_frames + min_spin;
    let hit = g::BIKE_CHARGE_FINAL_HITMARK + ca_frames;
    let hs = hittables(s, ci);
    hold_ca(s, ci, min_spin, skipped, &hs);
    let src = st(s, ci).ca.src_frame;
    char_task(s, ci, TASK_CAF_HIT, src, hit);
    let ns_dur = remaining_ns_duration(s, ci);
    if ns_dur <= hit {
        char_task(s, ci, TASK_CAF_NS_EXIT, 0, ns_dur);
    }
    core_task(s, ci, TASK_CA_UNHOOK, 0, ca_frames);
    let t = &g::BIKE_CHARGE_FINAL_FRAMES;
    ActInfo::new(Frames::Char { id: FR_BIKE_CHARGE_FINAL, arg: ca_frames as i16 }, t[0] + ca_frames, t[Action::Dash as usize] + ca_frames, AnimState::Charge)
}

/// gcsim's GetBikeChargeAttackAttackInfo, built as the hit lands.
fn ca_cyclic_info(s: &Sim, ci: usize) -> AttackInfo {
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_MAVUIKA_FLAMESTRIDER, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_CHARGE[s.talent(ci, 1)]);
    ai.abil = "Flamestrider Charged Attack (Cyclic)";
    ai.add_tags = add_tag::NIGHTSOUL;
    ai.blunt = true;
    ai.poise = 60.0;
    ai.hitlag_factor = 0.01;
    ai.deployable = true;
    ai.ignore_infusion = true;
    ai.flat_dmg = burst_bonus(s, ci, g::BURST_CA_BONUS) + c2_flat(s, ci, 0.9);
    ai
}

// ---------------------------------------------------------------- attacks and skills

fn particle_and_ns(ai: &mut AttackInfo) {
    ai.add_tags = add_tag::NIGHTSOUL;
}

/// The skill's damage: on Flamestrider (hold or refresh), or the rings.
fn skill_hit(s: &mut Sim, ci: usize, bike: bool) {
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL[s.talent(ci, 1)]);
    particle_and_ns(&mut ai);
    if bike {
        ai.abil = "The Named Moment (Flamestrider)";
        ai.blunt = true;
        ai.poise = 75.0;
        ai.hitlag_factor = 0.05;
        ai.pattern = s.circle_on_player(0.0, 1.0, 6.0);
    } else {
        ai.abil = "The Named Moment";
        ai.pattern = s.circle_on_player(0.0, 0.5, 5.0);
    }
    s.queue_attack(ai, CB_PARTICLE, g::SKILL_HITMARK, g::SKILL_HITMARK);
}

/// gcsim's getSkillCastActionInfo: out of a plunge, longer.
fn skill_cast_info(s: &Sim, ci: usize, id: u8, t: &'static FrameTable, queue: Action) -> ActInfo {
    let pf = if s.current_state() == AnimState::Plunge { if st(s, ci).bike { 19 } else { 14 } } else { 0 };
    ActInfo::new(Frames::Char { id, arg: pf as i16 }, t[0] + pf, t[queue as usize] + pf, AnimState::Skill)
}

fn skill_table(id: u8) -> &'static FrameTable {
    match id {
        FR_SKILL => &g::SKILL_FRAMES,
        FR_SKILL_HOLD => &g::SKILL_FRAMES_HOLD,
        _ => &g::SKILL_BIKE_REFRESH_FRAMES,
    }
}

/// The bike dash's frames (gcsim's dashFrames, rewritten by each dash).
fn dash_frames(travel: i32, cc: bool, next: Action) -> i32 {
    match next {
        Action::Charge if cc => travel,
        Action::Charge | Action::Skill | Action::Burst => travel.max(20),
        Action::Attack => travel.max(18),
        Action::Swap | Action::Jump => travel,
        _ => 24,
    }
}

fn bike_plunge(s: &mut Sim, ci: usize, t: &'static FrameTable, delay: i32) -> ActInfo {
    let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_MAVUIKA_FLAMESTRIDER, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_PLUNGE[s.talent(ci, 1)]);
    ai.abil = "Flamestrider Plunge";
    ai.add_tags = add_tag::NIGHTSOUL;
    ai.blunt = true;
    ai.poise = g::BIKE_PLUNGE_POISE_DMG;
    ai.hitlag_factor = 0.1;
    ai.pattern = s.circle_on_player(0.0, 1.0, g::BIKE_PLUNGE_RADIUS);
    s.queue_attack(ai, 0, delay, delay);
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Plunge)
}

/// Out of Flamestrider the plunges queue after their dash frames.
fn plunge_xy(s: &mut Sim, ci: usize, high: bool, p: &Params) -> ActInfo {
    plunge::standard(s, ci, high, p, &PLUNGE, 0)
}

impl Character for Char {
    const KEY: &'static str = "mavuika";
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        let fs = cs.params.iter().find(|p| p.0 == "start_energy").map_or(g::MAX_FIGHTING_SPIRIT as i64, |p| p.1);
        let fs = fs.clamp(0, g::MAX_FIGHTING_SPIRIT as i64);
        State {
            fighting_spirit: fs as f64,
            nightsoul_src: 0,
            bike: false,
            ring_src: 0,
            burst_stacks: 0.0,
            a4_src: 0,
            a4_stacks: 0,
            c6_src: 0,
            saved_normal_counter: 0,
            ca: ChargeState::default(),
            can_bike_plunge: false,
            charge_cancel: false,
            ca_hooked: false,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // onExitField
        s.subscribe(EventKind::Swap, o);
        // burstInit
        s.subscribe(EventKind::NightsoulConsume, o);
        s.subscribe(EventKind::EnemyDamage, o);
        // a1
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::NightsoulBurst, o);
        }
        // c1Init: 80 points at most, 120 with C1
        s.chars[ci].ns.max_points = if s.chars[ci].cons >= 1 { 120.0 } else { 80.0 };
        // c2Init: 200 base ATK in Nightsoul's Blessing
        if s.chars[ci].cons >= 2 {
            s.chars[ci].mods.push(Mod::owned(C2_BASE_ATK, ModKind::Stat, o, st::BASEATK, false, false, -1));
        }
        // a4Init: DMG to the active character while A4 lasts
        if s.chars[ci].ascension >= 4 {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(A4_BUF, ModKind::Attack, o, st::NONE, false, false, -1));
            }
        }
        // the spinning charged attack's OnDendroCore hook, live only while
        // gcsim has it subscribed
        s.subscribe(EventKind::DendroCore, o);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let known: &[&str] = match a {
            Action::Attack | Action::Burst | Action::Jump => &[],
            Action::Charge => &["hold", "final", "buffered"],
            Action::Skill => &["hold", "recast"],
            Action::Dash => &["collision", "travel", "charge_cancel"],
            Action::LowPlunge | Action::HighPlunge => return plunge::unknown_param(a, p, &[]),
            _ => return Some(name.into()),
        };
        p.0.iter().find(|(k, _)| !known.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
    }

    /// gcsim's NextQueueItemIsValid (a dash that cancels a charged attack
    /// must be followed by Flamestrider's charged attack) and its plunge
    /// errors.
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if st(s, ci).charge_cancel {
            if a == Action::Swap {
                return Some("cannot swap, Mavuika must perform a Biked charge attack after a dash cancel".into());
            }
            if !s.ns_has_blessing(ci) {
                return Some("nightsoul blessing expired before Mavuika could perform charge cancel".into());
            }
            if s.current_state() != AnimState::Dash {
                return Some("cannot allow Mavuika to go into idle during charge-cancelled dash".into());
            }
            if a != Action::Charge {
                return Some(format!("cannot perform action {}, Mavuika must perform a Biked charge attack after a dash cancel", crate::gcsl::action_name(a)));
            }
        }
        match a {
            Action::LowPlunge if s.player.airborne != Airborne::Xianyun && !st(s, ci).can_bike_plunge => Some("low_plunge can only be used while airborne".into()),
            Action::HighPlunge => plunge::grounded(s, a),
            _ => crate::content::template_invalid(s, a),
        }
    }

    fn action_ready(s: &Sim, ci: usize, a: Action, p: &Params) -> Option<Option<Failure>> {
        match a {
            Action::Burst if !s.ignore_burst_energy && st(s, ci).fighting_spirit < 100.0 => Some(Some(Failure::InsufficientEnergy)),
            Action::Skill if p.get("recast").unwrap_or(0) != 0 && !s.mod_active(ci, SKILL_RECAST_CD) => Some(None),
            Action::Charge if on_bike_ns(s, ci) => {
                // canBeginBikedCharge
                if !can_charge_now(s, ci, p) {
                    return Some(Some(Failure::InsufficientStamina));
                }
                None
            }
            _ => None,
        }
    }

    fn action_stam(s: &Sim, ci: usize, a: Action) -> Option<f64> {
        if on_bike_ns(s, ci) && matches!(a, Action::Charge | Action::Dash) {
            return Some(0.0);
        }
        (a == Action::Charge).then_some(50.0)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "nightsoul" => Some(s.ns_condition(ci, fields)),
            "fightingspirit" => Some(Ok(Num::Float(st(s, ci).fighting_spirit))),
            _ => None,
        }
    }

    /// AnimationStartDelay: none on Flamestrider in the blessing, else 22
    /// (the data's).
    fn start_delay(s: &Sim, ci: usize, key: usize) -> Option<i32> {
        (on_bike_ns(s, ci) && (key == 1 || key == 2)).then_some(0)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if on_bike_ns(s, ci) {
            return bike_attack(s, ci);
        }
        let n = s.chars[ci].normal_counter as usize;
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][s.talent(ci, 0)]);
        ai.abil = NORMAL_NAMES_RING[n];
        ai.blunt = true;
        ai.poise = g::ATTACK_POISE_DMG[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        let (hb, off) = (g::ATTACK_HITBOXES[n], g::ATTACK_OFFSETS[n]);
        ai.pattern = if hb.len() == 2 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
        for &d in g::ATTACK_HITMARKS[n] {
            s.queue_attack(ai, 0, d, d);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if on_bike_ns(s, ci) {
            return bike_charge(s, ci, p);
        }
        if s.current_state() == AnimState::Dash && st(s, ci).charge_cancel {
            fail(s, "can only cancel a dash with a biked charge");
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Charge);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[s.talent(ci, 0)]);
        ai.abil = "Charge";
        ai.blunt = true;
        ai.poise = 120.0;
        ai.hitlag_halt = 0.15 * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.pattern = s.circle_on_player(0.0, 0.3, 3.3);
        let h = g::CHARGE_HITMARK;
        s.queue_attack(ai, 0, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(0);
        if p.get("recast").unwrap_or(0) != 0 {
            if hold > 0 {
                fail(s, "cannot hold E while recasting");
            } else if !s.ns_has_blessing(ci) {
                fail(s, "cannot recast E while not in nightsoul blessing");
            } else if s.chars[ci].cd[0].available >= 1 {
                fail(s, "cannot recast E while skill is available");
            } else {
                return skill_recast(s, ci);
            }
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Skill);
        }
        let info = if on_bike_ns(s, ci) {
            // skillBikeRefresh
            skill_hit(s, ci, true);
            s.set_cd_with_delay(ci, Action::Skill, 15 * 60, 18);
            s.add_status(ci, SKILL_RECAST_CD, g::SKILL_RECAST_CD, false);
            skill_cast_info(s, ci, FR_SKILL_REFRESH, &g::SKILL_BIKE_REFRESH_FRAMES, Action::Swap)
        } else if hold > 0 {
            skill_hit(s, ci, true);
            enter_bike(s, ci);
            s.set_cd_with_delay(ci, Action::Skill, 15 * 60, 18);
            core_task(s, ci, TASK_RECAST_STATUS, 0, 24);
            skill_cast_info(s, ci, FR_SKILL_HOLD, &g::SKILL_FRAMES_HOLD, Action::Swap)
        } else {
            skill_hit(s, ci, false);
            exit_bike(s, ci);
            s.set_cd_with_delay(ci, Action::Skill, 15 * 60, 18);
            skill_cast_info(s, ci, FR_SKILL, &g::SKILL_FRAMES, Action::Attack)
        };
        let max = s.chars[ci].ns.max_points;
        enter_or_regenerate(s, ci, max);
        info
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let st = st_mut(s, ci);
        st.burst_stacks = st.fighting_spirit;
        st.fighting_spirit = 0.0;
        enter_bike(s, ci);
        char_task(s, ci, TASK_BURST_NS, 0, 87);
        char_task(s, ci, TASK_BURST_STATUS, 0, g::BURST_HITMARK - 1);
        char_task(s, ci, TASK_BURST_HIT, 0, g::BURST_HITMARK);
        s.set_cd(ci, Action::Burst, 18 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if on_bike_ns(s, ci) {
            return bike_plunge(s, ci, &g::HIGH_BIKE_PLUNGE_FRAMES, g::HIGH_PLUNGE_HITMARK);
        }
        plunge_xy(s, ci, true, p)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.ns_has_blessing(ci) {
            return bike_plunge(s, ci, &g::LOW_BIKE_PLUNGE_FRAMES, g::LOW_PLUNGE_HITMARK);
        }
        plunge_xy(s, ci, false, p)
    }

    /// Flamestrider's sprint (gcsim's Dash on the bike in the blessing).
    fn own_dash(s: &mut Sim, ci: usize, p: &Params) -> Option<ActInfo> {
        if !on_bike_ns(s, ci) {
            return None;
        }
        let collision = p.get("collision").unwrap_or(1);
        let mut travel = (p.get("travel").unwrap_or(6) as i32).clamp(6, 24);
        if collision == 0 {
            travel = 2;
        }
        st_mut(s, ci).charge_cancel = false;
        let mut cc = p.get("charge_cancel").unwrap_or(0);
        if s.mod_active(ci, CDC_LOCKOUT) {
            cc = 0;
        }
        if cc != 0 {
            st_mut(s, ci).charge_cancel = true;
        }
        if collision != 0 {
            char_task(s, ci, TASK_DASH_HIT, 0, travel);
        }
        reduce_points(s, ci, 10.0);
        if s.current_state() == AnimState::Normal {
            st_mut(s, ci).saved_normal_counter = s.chars[ci].normal_counter;
        }
        s.apply_dash_cd();
        let arg = travel + if cc != 0 { 256 } else { 0 };
        Some(ActInfo::new(Frames::Char { id: FR_DASH, arg: arg as i16 }, 24, travel, AnimState::Dash))
    }

    /// Flamestrider's jump, from walking on the bike in the blessing.
    fn own_jump(s: &mut Sim, ci: usize, _: &Params) -> Option<ActInfo> {
        if s.mod_active(ci, XIANYUN_AIRBORNE) || !on_bike_ns(s, ci) || s.current_state() != AnimState::Walk {
            return None;
        }
        st_mut(s, ci).can_bike_plunge = true;
        let t = &g::BIKE_JUMP_FRAMES;
        char_task(s, ci, TASK_BIKE_PLUNGE_OFF, 0, t[0]);
        Some(ActInfo::new(Frames::Table(t), t[0], t[Action::LowPlunge as usize], AnimState::Jump))
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        let arg = arg as i32;
        match id {
            FR_SKILL | FR_SKILL_HOLD | FR_SKILL_REFRESH => skill_table(id)[next as usize] + arg,
            FR_DASH => dash_frames(arg & 255, arg >= 256, next),
            FR_BIKE_CHARGE => {
                let (dur, min_spin) = (arg.div_euclid(32), arg.rem_euclid(32));
                if g::BIKE_CHARGE_FRAMES[next as usize] == 0 {
                    dur
                } else {
                    dur + min_spin + g::BIKE_CHARGE_FINAL_FRAMES[next as usize]
                }
            }
            FR_BIKE_CHARGE_FINAL => g::BIKE_CHARGE_FINAL_FRAMES[next as usize] + arg,
            _ => unreachable!(),
        }
    }

    fn on_removed(s: &mut Sim, ci: usize, id: u8, next: AnimState) {
        if id == REMOVED_BIKE_CHARGE && next != AnimState::Charge {
            let st = st_mut(s, ci);
            st.ca.reset();
            st.ca_hooked = false;
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_REDUCE => {
                if st(s, ci).nightsoul_src != arg {
                    return;
                }
                let mut val = 0.5;
                if st(s, ci).bike {
                    val += 0.4;
                    if s.current_state() == AnimState::Charge {
                        val += 0.2;
                    }
                }
                reduce_points(s, ci, val);
                char_task(s, ci, TASK_REDUCE, arg, 6);
            }
            TASK_RING => {
                if st(s, ci).ring_src != arg || st(s, ci).bike || !s.ns_has_blessing(ci) {
                    return;
                }
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_RING[s.talent(ci, 1)]);
                ai.abil = "Rings of Searing Radiance";
                ai.add_tags = add_tag::NIGHTSOUL;
                ai.pattern = s.circle_on_player(0.0, 1.0, 6.0);
                let cb = if s.chars[ci].cons >= 6 { CB_C6_RING } else { 0 };
                s.queue_attack(ai, cb, 0, 0);
                reduce_points(s, ci, 3.0);
                char_task(s, ci, TASK_RING, arg, 2 * 60);
            }
            TASK_A4_DECAY => {
                let st = st(s, ci);
                if st.a4_src != arg || !s.mod_active(ci, A4) || st.a4_stacks <= 0 {
                    return;
                }
                let rate = if s.chars[ci].cons >= 4 { 0 } else { 1 };
                st_mut(s, ci).a4_stacks -= rate;
                char_task(s, ci, TASK_A4_DECAY, arg, 60);
            }
            TASK_BURST_NS => enter_or_regenerate(s, ci, 10.0),
            TASK_BURST_STATUS => s.add_status(ci, BURST, g::BURST_DURATION as i32, true),
            TASK_BURST_HIT => {
                // A4: 20 stacks of DMG that decay
                if s.chars[ci].ascension >= 4 {
                    s.add_status(ci, A4, g::A4_DUR, true);
                    let f = s.f;
                    let st = st_mut(s, ci);
                    st.a4_stacks = 20;
                    st.a4_src = f;
                    char_task(s, ci, TASK_A4_DECAY, f, 60);
                }
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BURST[s.talent(ci, 2)]);
                ai.abil = "Sunfell Slice";
                ai.add_tags = add_tag::NIGHTSOUL;
                ai.blunt = true;
                ai.poise = 150.0;
                ai.hitlag_factor = 0.01;
                ai.flat_dmg = burst_bonus(s, ci, g::BURST_Q_BONUS) + c2_flat(s, ci, 1.2);
                ai.pattern = s.circle_on_player(0.0, 2.5, 7.0);
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_RECAST_STATUS => s.add_status(ci, SKILL_RECAST_CD, g::SKILL_RECAST_CD, false),
            TASK_C2_RING => c2_ring(s, ci),
            TASK_C6_RING => {
                if st(s, ci).c6_src != arg || !st(s, ci).bike || !s.ns_has_blessing(ci) {
                    return;
                }
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 0.0, 4.0);
                ai.abil = "Rings of Searing Radiance (C6)";
                ai.add_tags = add_tag::NIGHTSOUL;
                ai.pattern = s.circle_on_player(0.0, 1.0, 6.0);
                s.queue_attack(ai, 0, 0, 0);
                char_task(s, ci, TASK_C6_RING, arg, 180);
            }
            TASK_BIKE_NA => {
                let n = (arg & 7) as usize;
                let hb = if arg & 8 != 0 { g::BIKE_ATTACK_BURST_HITBOXES[n] } else { g::BIKE_ATTACK_HITBOXES[n] };
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_MAVUIKA_FLAMESTRIDER, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_ATTACK[n][s.talent(ci, 0)]);
                ai.abil = BIKE_NAMES[n];
                ai.add_tags = add_tag::NIGHTSOUL;
                ai.blunt = true;
                ai.poise = g::BIKE_ATTACK_POISE_DMG[n];
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = g::BIKE_ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
                ai.ignore_infusion = true;
                let off = g::BIKE_ATTACK_OFFSETS[n];
                ai.pattern = if hb.len() == 2 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
                ai.flat_dmg = burst_bonus(s, ci, g::BURST_NA_BONUS) + c2_flat(s, ci, 0.6);
                s.queue_attack(ai, 0, 0, 0);
                reduce_points(s, ci, 1.0);
            }
            TASK_CA_HIT => {
                let mut ai = ca_cyclic_info(s, ci);
                ai.pattern = if arg < 0 { Pattern::Single } else { Pattern::Gadget { key: arg as u32 } };
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_CA_AUTO_FINAL => {
                if st(s, ci).ca.src_frame == arg {
                    bike_charge_final(s, ci, 0, 0);
                }
            }
            TASK_CAF_HIT => {
                if s.player.active as usize != ci || st(s, ci).ca.src_frame != arg || !st(s, ci).bike {
                    return;
                }
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_MAVUIKA_FLAMESTRIDER, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_CHARGE_FINAL[s.talent(ci, 1)]);
                ai.abil = "Flamestrider Charged Attack (Final)";
                ai.add_tags = add_tag::NIGHTSOUL;
                ai.blunt = true;
                ai.poise = 120.0;
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = 0.04 * 60.0;
                ai.ignore_infusion = true;
                ai.flat_dmg = burst_bonus(s, ci, g::BURST_CA_BONUS) + c2_flat(s, ci, 0.9);
                let r = if s.mod_active(ci, BURST) { 4.5 } else { 4.0 };
                ai.pattern = s.circle_on_player(0.0, 2.0, r);
                s.queue_attack(ai, 0, 0, 0);
                st_mut(s, ci).ca.reset();
                s.delete_mod(ci, CDC_LOCKOUT);
            }
            TASK_CAF_NS_EXIT => {
                let pts = s.ns_points(ci);
                s.ns_consume(ci, pts);
                exit_nightsoul(s, ci);
            }
            TASK_CA_UNHOOK => st_mut(s, ci).ca_hooked = false,
            TASK_CDC_LOCKOUT => {
                if st(s, ci).ca.start_frame != arg {
                    return;
                }
                s.add_status(ci, CDC_LOCKOUT, 87 - 79 + 1, true);
                char_task(s, ci, TASK_CDC_LOCKOUT, arg, g::BIKE_CHARGE_ATTACK_SPIN_FRAMES);
            }
            TASK_BIKE_PLUNGE_OFF => st_mut(s, ci).can_bike_plunge = false,
            TASK_DASH_HIT => {
                let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_MAVUIKA_FLAMESTRIDER, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_DASH[s.talent(ci, 1)]);
                ai.abil = "Flamestrider Sprint";
                ai.add_tags = add_tag::NIGHTSOUL;
                ai.blunt = true;
                ai.poise = 75.0;
                ai.deployable = true;
                ai.pattern = s.circle_on_player(0.0, 1.0, 1.2);
                s.queue_attack(ai, 0, 0, 0);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 30, true);
            s.queue_particle(5.0, Element::Pyro, s.particle_delay[ci]);
        }
        // C6: a ring's hit adds a blow on the enemy, once every 0.5 s
        if cbs & CB_C6_RING != 0 && !s.mod_active(ci, C6_ICD) {
            s.add_status(ci, C6_ICD, 30, true);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 0.0, 2.0);
            ai.abil = "Flamestrider (C6)";
            ai.add_tags = add_tag::NIGHTSOUL;
            ai.blunt = true;
            ai.poise = 75.0;
            ai.pattern = s.circle_on_target(6.0);
            s.queue_attack(ai, 0, 3, 3);
        }
    }

    /// C2: 200 base ATK in the blessing.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if !s.ns_has_blessing(ci) {
            return false;
        }
        out[st::BASEATK] += 200.0;
        true
    }

    /// A4: the active character's DMG by the burst's Fighting Spirit, as
    /// the stacks decay.
    fn attack_mod(s: &Sim, ci: usize, on: usize, _: &Mod, ev: &mut AttackEvent) {
        if s.player.active as usize != on || !s.mod_active(ci, A4) {
            return;
        }
        let st = st(s, ci);
        let c4 = if s.chars[ci].cons >= 4 { 0.1 } else { 0.0 };
        let dmg = (st.burst_stacks * 0.002 + c4) * st.a4_stacks as f64 / 20.0;
        ev.stats[st::DMGP] += dmg;
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // onExitField: any swap ends the burst, and Flamestrider
            Event::Swap { .. } => {
                s.delete_mod(ci, BURST);
                if on_bike_ns(s, ci) {
                    exit_bike(s, ci);
                }
            }
            Event::NightsoulConsume { amount, .. } => {
                if amount < 0.0000001 {
                    return;
                }
                gain_fighting_spirit(s, ci, amount);
            }
            Event::EnemyDamage(ev) => {
                if ev.on_player || ev.info.tag != AttackTag::Normal || s.mod_active(ci, ENERGY_NA_ICD) {
                    return;
                }
                s.add_status(ci, ENERGY_NA_ICD, 6, true);
                gain_fighting_spirit(s, ci, 1.5);
            }
            // A1: 30% ATK for 10 s on a Nightsoul Burst
            Event::NightsoulBurst(_) => s.add_stat_mod(ci, A1, st::ATKP, 0.3, 10 * 60, true),
            Event::DendroCore { key } => dendro_core(s, ci, key),
            _ => {}
        }
    }
}

/// Flamestrider's normal attacks: the hit at its hitmark on her queue,
/// with the burst's bonus as it lands, and 1 point.
fn bike_attack(s: &mut Sim, ci: usize) -> ActInfo {
    let n = s.chars[ci].normal_counter;
    let delay = g::BIKE_ATTACK_HITMARKS[n as usize];
    let arg = n + if s.mod_active(ci, BURST) { 8 } else { 0 };
    char_task(s, ci, TASK_BIKE_NA, arg, delay);
    let atkspd = s.stat(ci, st::ATKSPD);
    let table = &g::BIKE_ATTACK_FRAMES[n as usize];
    let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], delay, AnimState::Normal);
    s.advance_normal(ci);
    st_mut(s, ci).saved_normal_counter = s.chars[ci].normal_counter;
    info
}

/// gcsim's skillRecast: between the rings and Flamestrider.
fn skill_recast(s: &mut Sim, ci: usize) -> ActInfo {
    s.add_status(ci, SKILL_RECAST_CD, g::SKILL_RECAST_CD, false);
    let (t, q): (&'static FrameTable, Action) = if !st(s, ci).bike {
        enter_bike(s, ci);
        (&g::SKILL_RECAST_FRAMES_TO_BIKE, Action::Attack)
    } else {
        exit_bike(s, ci);
        (&g::SKILL_RECAST_FRAMES_TO_RING, Action::Attack)
    };
    ActInfo::new(Frames::Table(t), t[0], t[q as usize], AnimState::Skill)
}

/// canBeginBikedCharge from ActionReady, which takes the state mutably in
/// gcsim; the engine's hook cannot, so it reads without clearing the
/// charge cancel (the dash's own charged attack clears it again).
fn can_charge_now(s: &Sim, ci: usize, p: &Params) -> bool {
    if s.current_state() != AnimState::Charge || st(s, ci).ca.start_frame == 0 {
        let buffered = p.get("buffered").map_or(g::MAX_BUFFERED_BIKE_CHARGE_FRAMES, |v| v as i32);
        let max = g::MAX_BUFFERED_BIKE_CHARGE_FRAMES;
        let skipped = match s.current_state() {
            AnimState::Dash | AnimState::Normal => max,
            AnimState::Charge if st(s, ci).ca.start_frame == s.f => max,
            AnimState::Burst => {
                if buffered == 0 {
                    0
                } else {
                    max
                }
            }
            AnimState::Skill if s.mod_active(ci, SKILL_RECAST_CD) => {
                if s.mod_duration(ci, SKILL_RECAST_CD.name) > 45 {
                    13
                } else {
                    max
                }
            }
            AnimState::Plunge => 13,
            _ => 0,
        }
        .min(buffered);
        if remaining_ns_duration(s, ci) < max - skipped {
            return false;
        }
    }
    true
}

/// The charged attack's OnDendroCore hook: a core that appears in the
/// spin's reach is hit on the frames left.
fn dendro_core(s: &mut Sim, ci: usize, key: u32) {
    if !st(s, ci).ca_hooked {
        return;
    }
    // gcsim's check, as written
    if !st(s, ci).bike && !s.ns_has_blessing(ci) {
        return;
    }
    let Some(k) = s.gadgets.iter().position(|gd| gd.key == key) else { return };
    let gd = s.gadgets[k];
    if let Some(h) = gadget_hittable(s, ci, &gd) {
        let ca = &st(s, ci).ca;
        let remaining = ca.c_atk_frames - (s.f - ca.start_frame);
        let hits = collision_frames(s, ci, remaining, h.coll, 0);
        for &f in &hits.list[..hits.len] {
            queue_ca_hit(s, ci, h.target, f);
        }
        let f = s.f;
        st_mut(s, ci).ca.add_last_hit(h.target, f);
    }
}

