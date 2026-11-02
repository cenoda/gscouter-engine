//! Chasca, from gcsim's internal/characters/chasca: normal attacks, aimed
//! shots, Spirit Reins, Shadow Hunt (her Nightsoul's Blessing, with its
//! tap shots, its dash and its Shadowhunt Shells charged by holding aim),
//! Soul Reaper's Galesplitting Soulseeker Shell, the low plunge after the
//! blessing ends, A1, A4 and every constellation.
//!
//! The skill enters a blessing without end with 80 points; the points run
//! down by 0.8 every 6 frames (a task of the fight's queue), 13.3 a dash,
//! and the blessing ends below 0.001 or on a second press of the skill,
//! starting the skill's cooldown of 6.5 s and a window of 26 frames for
//! the low plunge. While she has points she cannot be swapped out (gcsim's
//! ActionReady refuses the swap with SwapCD).
//!
//! The actions in the blessing end early when it ends (gcsim's
//! skillNextFrames): their frames are read as the fight evaluates them,
//! with `Frames::CharLive`. The first evaluation after the blessing ended
//! fixes the time it ended at (the closure's actionEnd), and from then the
//! frames are the time the action lasted to that point, the skill's cancel
//! frames and the action's extra delay.
//!
//! The bullets' elements are drawn as gcsim draws them: A1's conversion of
//! the third (a Float64, then an Intn among the party's distinct pyro,
//! hydro, cryo and electro), C1's second, and the last three from a pool of
//! the party's members of those elements, without replacement. The burst
//! shuffles its shells with Fisher-Yates on the engine's own Intn; gcsim's
//! Rand.Shuffle takes Go's unexported int31n, so the shuffle matches
//! gcsim's in distribution only (the engine reproduces none of gcsim's
//! random numbers anyway).
//!
//! The fight has one enemy: the bullets and shells target it (gcsim cycles
//! through the enemies in range), and bullets with no enemy within 10 of
//! the player are an error (gcsim divides by the count of zero).

use std::cell::Cell;

use crate::content::{Character, Event, EventKind, Owner};
use crate::gcsimdata::chars::chasca as g;
use crate::gcsimdata::icd;
use crate::gcsl::Num;
use crate::nightsoul::CB_EXIT;
use crate::sim::*;
use crate::types::*;

pub struct Char;

/// The Shadowhunt Shells of one fireBullets: their elements in the order
/// they fire, its frame, and whether C6's CRIT DMG applies.
#[derive(Clone, Copy)]
struct Fire {
    elems: [Element; 6],
    f: i32,
    c6: bool,
    /// gcsim's c2cb variable is set: a Shining shell of this fire went
    /// off, and every shell after it carries C2's callback.
    c2_armed: bool,
}

pub struct State {
    /// The party's pyro, hydro, cryo and electro members, in team order
    /// (gcsim's partyPHECTypes), and their distinct elements
    /// (partyPHECTypesUnique).
    phec: [Element; 4],
    n_phec: usize,
    uniq: [Element; 4],
    n_uniq: usize,
    /// gcsim's bulletsToFire and bulletsNext.
    to_fire: [Element; 6],
    next: [Element; 6],
    /// gcsim's bulletPool.
    pool: [Element; 4],
    n_pool: usize,
    bullets_charged: i32,
    /// The frame of the current charge, -1 for none (gcsim's aimSrc).
    aim_src: i32,
    skill_particle_icd: bool,
    c2_src: i32,
    c4_src: i32,
    c6_used: bool,
    /// The frame the blessing was entered, -1 when out of it (gcsim's
    /// nightsoulSrc).
    ns_src: i32,
    /// skillNextFrames of the current action: TimePassed at its start,
    /// the TimePassed it ended at (-1 until read), its own frames, and its
    /// attack speed when fixed at its start.
    fr_start: i32,
    fr_end: Cell<i32>,
    fr_inner: Frames,
    fr_atkspd: f64,
    /// The dash began in the blessing.
    dash_ns: bool,
    fires: [Fire; 2],
    n_fires: i32,
    /// The tap shots waiting for their hitmark, by parity.
    taps: [(AttackInfo, Pattern); 2],
    n_taps: i32,
    burst_f: i32,
}

const A1: ModKey = ModKey::new("chasca-a1");
const PLUNGE_AVAILABLE: ModKey = ModKey::new(g::PLUNGE_AVAILABLE_KEY);
const C6: ModKey = ModKey::new(g::C6KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;
/// C2's callback for the fire in slot 0 or 1.
const CB_C2: [u16; 2] = [1 << 1, 1 << 2];
const CB_C4: u16 = 1 << 3;

/// The blessing's points running down, with its src (the fight's queue).
const TASK_NS_REDUCE: u8 = 0;
/// A bullet loaded: aimSrc * 8 + its index from 1.
const TASK_BULLET: u8 = 1;
/// The charge's end: fire if aimSrc is still this one.
const TASK_FIRE_AT: u8 = 2;
/// fireBullets, unconditionally (the end of the blessing while aiming).
const TASK_FIRE: u8 = 3;
/// A Shadowhunt Shell: fire * 8 + its index.
const TASK_SHELL: u8 = 4;
/// A tap shot's hitmark, by its count.
const TASK_TAP: u8 = 5;
/// The burst's fall: the plunge window if out of the blessing.
const TASK_BURST_FALL: u8 = 6;

/// Frames of the skill, the blessing's dash, the charge, the burst in the
/// blessing and the tap shot, through skillNextFrames; and the normal
/// attack's (arg: counter * 16 + windup).
const FR_SKILL: u8 = 0;
const FR_DASH: u8 = 1;
const FR_AIM: u8 = 2;
const FR_BURST_NS: u8 = 3;
const FR_TAP: u8 = 4;
const FR_NORMAL: u8 = 5;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];

impl Character for Char {
    const KEY: &'static str = "chasca";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        let fire = Fire { elems: [Element::Anemo; 6], f: -1, c6: false, c2_armed: false };
        let tap = (AttackInfo::new(0, AttackTag::Normal, icd::TAG_CHASCA_TAP, icd::GROUP_DEFAULT, Element::Anemo, 25.0, 0.0), Pattern::Unknown);
        State {
            phec: [Element::NoElement; 4],
            n_phec: 0,
            uniq: [Element::NoElement; 4],
            n_uniq: 0,
            to_fire: [Element::NoElement; 6],
            next: [Element::NoElement; 6],
            pool: [Element::NoElement; 4],
            n_pool: 0,
            bullets_charged: 0,
            aim_src: 0,
            skill_particle_icd: false,
            c2_src: 0,
            c4_src: 0,
            c6_used: false,
            ns_src: 0,
            fr_start: 0,
            fr_end: Cell::new(-1),
            fr_inner: Frames::Const(0),
            fr_atkspd: 0.0,
            dash_ns: false,
            fires: [fire; 2],
            n_fires: 0,
            taps: [tap; 2],
            n_taps: 0,
            burst_f: 0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let elems: Vec<Element> = s.chars.iter().map(|c| c.element).collect();
        let st = st_mut(s, ci);
        for e in elems {
            if matches!(e, Element::Pyro | Element::Hydro | Element::Cryo | Element::Electro) {
                if !st.uniq[..st.n_uniq].contains(&e) {
                    st.uniq[st.n_uniq] = e;
                    st.n_uniq += 1;
                }
                st.phec[st.n_phec] = e;
                st.n_phec += 1;
            }
        }
        load_skill_hold_bullets(s, ci);
        // a1DMGBuff
        if s.chars[ci].ascension >= 1 {
            s.chars[ci].mods.push(Mod::owned(A1, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1));
        }
        // a4: OnNightsoulBurst
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::NightsoulBurst, Owner::Char(ci as u8));
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Aim | Action::Skill | Action::Burst | Action::LowPlunge => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let st = st(s, ci);
        let bullet = |i: usize| Some(Ok(Num::Int(st.next[i] as i64)));
        match fields[0].as_str() {
            "bullet1" => bullet(0),
            "bullet2" => bullet(1),
            "bullet3" => bullet(2),
            "bullet4" => bullet(3),
            "bullet5" => bullet(4),
            "bullet6" => bullet(5),
            "nightsoul" => Some(s.ns_condition(ci, fields)),
            _ => None,
        }
    }

    /// Faster N0 for Xingqiu's and Yelan's bursts in the blessing.
    fn start_delay(s: &Sim, ci: usize, key: usize) -> Option<i32> {
        if !s.ns_has_blessing(ci) {
            return None;
        }
        match key {
            1 => Some(0),
            2 => Some(5),
            _ => None,
        }
    }

    /// No swap while she has Nightsoul points.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Swap && s.ns_points(ci) > 0.0).then_some(Some(Failure::SwapCd))
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.ns_has_blessing(ci) {
            return attack_skill_tap(s, ci);
        }
        let windup = if matches!(s.current_state(), AnimState::Normal | AnimState::Aim) { 0 } else { 5 };
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        // gcsim ranges over the hitmarks' indices, not their values
        for hitmark in 0..g::ATTACK_HITMARKS[n].len() as i32 {
            s.queue_attack(ai, 0, windup + hitmark, windup + hitmark + travel);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        st_mut(s, ci).fr_atkspd = atkspd;
        let hm = g::ATTACK_HITMARKS[n];
        let info = ActInfo::new(
            Frames::CharLive { id: FR_NORMAL, arg: (n as i16) << 4 | windup as i16 },
            g::ATTACK_FRAMES[n][0] + windup,
            hm[hm.len() - 1] + windup,
            AnimState::Normal,
        );
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.ns_has_blessing(ci) {
            return aim_skill_hold(s, ci, p);
        }
        let hold = p.get("hold").unwrap_or(1);
        if hold != 0 && hold != 1 {
            fail(s, &format!("invalid hold param supplied, got {hold}"));
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Aim);
        }
        let hold = hold as usize;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::FULL_AIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        ai.deployable = true;
        ai.hitlag_factor = 0.01;
        // its hitlag applies on weak spot hits only (HitlagOnHeadshotOnly)
        if p.get("weakspot") == Some(1) {
            ai.weak_point = true;
            ai.hitlag_halt = 0.12 * 60.0;
        }
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        let hitmark = g::AIMED_HITMARKS[hold];
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        s.queue_attack(ai, 0, hitmark, hitmark + travel);
        let t = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Table(t), t[0], hitmark, AnimState::Aim)
    }

    /// Spirit Reins, Shadow Hunt: the Resonance hit and the blessing, or,
    /// in the blessing, its end.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.ns_has_blessing(ci) {
            exit_nightsoul(s, ci);
            let t = &g::SKILL_CANCEL_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::LowPlunge as usize], AnimState::Skill);
        }
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL_RESONANCE[lvl]);
        ai.abil = "Spirit Reins, Shadow Hunt";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.pattern = s.circle_on_player(0.0, 0.0, 5.5);
        s.queue_attack(ai, 0, g::SKILL_HITMARKS, g::SKILL_HITMARKS);
        enter_nightsoul(s, ci);
        let t = &g::SKILL_FRAMES;
        next_frames(s, ci, Frames::Table(t));
        ActInfo::new(Frames::CharLive { id: FR_SKILL, arg: 0 }, t[0], t[Action::Attack as usize], AnimState::Skill)
    }

    /// Soul Reaper's Galesplitting Soulseeker Shell, then six shells: the
    /// party's pyro, hydro, cryo and electro members twice, shuffled, as
    /// Radiant Soulseeker Shells, and anemo Soulseeker Shells after them.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST_GALESPLITTING[lvl]);
        ai.abil = "Galesplitting Soulseeker Shell";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.pattern = s.circle_on_target(6.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        ai.icd_tag = icd::TAG_CHASCA_BURST;
        ai.icd_group = icd::GROUP_CHASCA_BURST;
        ai.pattern = Pattern::Single;
        let st = st(s, ci);
        let n = st.n_phec * 2;
        let mut shells = [Element::NoElement; 8];
        for i in 0..n {
            shells[i] = st.phec[i % st.n_phec];
        }
        // gcsim's Rand.Shuffle (Fisher-Yates from the back)
        for i in (1..n).rev() {
            let j = s.rng.intn(i as i32 + 1) as usize;
            shells.swap(i, j);
        }
        let f = s.f;
        st_mut(s, ci).burst_f = f;
        // gcsim's c4cb variable, set at the first radiant shell, stays set
        // for the shells after it
        let mut c4 = 0;
        for i in 0..6 {
            if i < n {
                let e = shells[i];
                ai.abil = shell_name(Shell::Radiant, e);
                ai.mult = g::BURST_RADIANT[lvl];
                ai.element = e;
                if s.chars[ci].cons >= 4 {
                    c4 = CB_C4;
                }
            } else {
                ai.abil = "Soulseeker Shell";
                ai.mult = g::BURST_SOULSEEKER[lvl];
                ai.element = Element::Anemo;
            }
            let hm = g::BURST_SECONDARY_HITMARK[i];
            s.queue_attack(ai, c4, hm, hm);
        }
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 3);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_FALL, arg: 0 }, g::BURST_NS_FALL);
        if s.ns_has_blessing(ci) {
            let t = &g::BURST_FRAMES_NS;
            next_frames(s, ci, Frames::Table(t));
            return ActInfo::new(Frames::CharLive { id: FR_BURST_NS, arg: 0 }, t[0], g::BURST_NS_FALL, AnimState::Burst);
        }
        let t = &g::BURST_FRAMES_GROUNDED;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    /// The low plunge, only in the window after the blessing ends.
    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if !s.mod_active(ci, PLUNGE_AVAILABLE) {
            fail(s, "only plunge after skill ends");
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Plunge);
        }
        s.delete_mod(ci, PLUNGE_AVAILABLE);
        let lvl = s.talent(ci, 0);
        if p.get("collision").unwrap_or(0) > 0 {
            let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 0.0, g::PLUNGE[lvl]);
            ai.abil = "Plunge Collision";
            ai.pattern = s.circle_on_player(0.0, 0.0, 1.5);
            s.queue_attack(ai, 0, g::COLLISION_HITMARK, g::COLLISION_HITMARK);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::LOW_PLUNGE[lvl]);
        ai.abil = "Low Plunge Attack";
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
        s.queue_attack(ai, 0, g::LOW_PLUNGE_HITMARK, g::LOW_PLUNGE_HITMARK);
        let t = &g::LOW_PLUNGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::LOW_PLUNGE_HITMARK, AnimState::Plunge)
    }

    /// The blessing's dash costs 13.3 points (before the template's dash).
    fn template_action(s: &mut Sim, ci: usize, a: Action) {
        if a != Action::Dash {
            return;
        }
        let ns = s.ns_has_blessing(ci);
        st_mut(s, ci).dash_ns = ns;
        if ns {
            reduce_points(s, ci, 13.3);
        }
    }

    /// The blessing's dash ends with it, and anything may queue after a
    /// frame.
    fn dash_info(s: &mut Sim, ci: usize, mut info: ActInfo) -> ActInfo {
        if !st(s, ci).dash_ns {
            return info;
        }
        next_frames(s, ci, info.frames);
        info.frames = Frames::CharLive { id: FR_DASH, arg: 0 };
        info.can_queue_after = 1;
        info
    }

    /// A jump in the blessing is not implemented in gcsim.
    fn own_jump(s: &mut Sim, ci: usize, _: &Params) -> Option<ActInfo> {
        if !s.ns_has_blessing(ci) {
            return None;
        }
        fail(s, "chasca jump in nightsoul blessing not implemented");
        Some(ActInfo::new(Frames::Const(1), 1, 1, AnimState::Jump))
    }

    fn frames_live(s: &Sim, ci: usize, id: u8, arg: i16, next: Action) -> i32 {
        let st = st(s, ci);
        let (inner, extra) = match id {
            FR_NORMAL => {
                let (n, windup) = ((arg >> 4) as usize, (arg & 15) as i32);
                return atk_spd_adjust(g::ATTACK_FRAMES[n][next as usize], st.fr_atkspd) + windup;
            }
            FR_SKILL | FR_DASH | FR_BURST_NS => (s.eval_frames(ci, st.fr_inner, next), 0),
            FR_AIM => (arg as i32 + g::SKILL_AIM_FRAMES[next as usize], g::SKILL_AIM_FALL_DELAY),
            FR_TAP => (atk_spd_adjust(g::ATTACK_SKILL_TAP_FRAMES[next as usize], s.stat(ci, st::ATKSPD)), 0),
            _ => unreachable!(),
        };
        // skillNextFrames
        if s.ns_has_blessing(ci) {
            return inner;
        }
        if st.fr_end.get() < 0 {
            st.fr_end.set(s.chars[ci].time_passed);
        }
        st.fr_end.get() - st.fr_start + g::SKILL_CANCEL_FRAMES[next as usize] + extra
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NS_REDUCE => {
                if st(s, ci).ns_src != arg {
                    return;
                }
                reduce_points(s, ci, 0.8);
                s.add_task(Task::Char { c: ci as u8, id: TASK_NS_REDUCE, arg }, 6);
            }
            TASK_BULLET => {
                let (src, i) = (arg >> 3, (arg & 7) as usize);
                // the bullets still charge up to 10 frames after the
                // blessing ends
                if st(s, ci).aim_src != src {
                    return;
                }
                let st = st_mut(s, ci);
                st.bullets_charged += 1;
                if st.to_fire[i - 1] != Element::Anemo {
                    c6(s, ci);
                }
            }
            TASK_FIRE_AT => {
                if st(s, ci).aim_src == arg {
                    fire_bullets(s, ci);
                }
            }
            TASK_FIRE => fire_bullets(s, ci),
            TASK_SHELL => shell(s, ci, (arg >> 3) as usize & 1, (arg & 7) as usize),
            TASK_TAP => {
                if !s.ns_has_blessing(ci) {
                    return;
                }
                let (mut ai, pattern) = st(s, ci).taps[arg as usize & 1];
                ai.pattern = pattern;
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_BURST_FALL => {
                if !s.ns_has_blessing(ci) {
                    s.add_status(ci, PLUNGE_AVAILABLE, 26, true);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        // particleCB
        if cbs & CB_PARTICLE != 0 && !st(s, ci).skill_particle_icd {
            st_mut(s, ci).skill_particle_icd = true;
            s.queue_particle(5.0, Element::Anemo, s.particle_delay[ci]);
        }
        for slot in 0..2 {
            if cbs & CB_C2[slot] != 0 {
                let src = st(s, ci).fires[slot].f;
                if st(s, ci).c2_src != src {
                    st_mut(s, ci).c2_src = src;
                    aoe(s, ci, "Shining Shadowhunt Shell (C2)", ev.info.element);
                }
            }
        }
        if cbs & CB_C4 != 0 {
            s.add_energy(ci, 1.5);
            let src = st(s, ci).burst_f;
            if st(s, ci).c4_src != src {
                st_mut(s, ci).c4_src = src;
                aoe(s, ci, "Radiant Shadowhunt Shell (C4)", ev.info.element);
            }
        }
    }

    /// A1: Shining Shadowhunt Shells deal more DMG by the party's distinct
    /// pyro, hydro, cryo and electro (one more step from C2).
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.icd_tag != icd::TAG_CHASCA_SHINING {
            return;
        }
        let c2 = (s.chars[ci].cons >= 2) as usize;
        ev.stats[st::DMGP] += g::A1_DMG_BUFF[st(s, ci).n_uniq + c2];
    }

    /// A4: a Burning Shadowhunt Shell on each Nightsoul Burst.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::NightsoulBurst(_) = *e else { return };
        let n_uniq = st(s, ci).n_uniq;
        let elem = if n_uniq > 0 {
            let k = s.rng.intn(n_uniq as i32) as usize;
            st(s, ci).uniq[k]
        } else {
            Element::Anemo
        };
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, 1.5 * g::SKILL_SHADOWHUNT[lvl]);
        ai.abil = "Burning Shadowhunt Shell";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.deployable = true;
        if elem != Element::Anemo {
            ai.abil = shell_name(Shell::Burning, elem);
            ai.element = elem;
            ai.mult = 1.5 * g::SKILL_SHINING[lvl];
        }
        ai.pattern = Pattern::Single;
        s.queue_attack(ai, 0, 0, 60);
    }
}

#[inline]
fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

#[inline]
fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn fail(s: &mut Sim, what: &str) {
    if s.error.is_none() {
        s.error = Some(format!("chasca: {what}"));
    }
}

/// skillNextFrames for the action starting now, with its own frames.
fn next_frames(s: &mut Sim, ci: usize, inner: Frames) {
    let tp = s.chars[ci].time_passed;
    let st = st_mut(s, ci);
    st.fr_start = tp;
    st.fr_end.set(-1);
    st.fr_inner = inner;
}

enum Shell {
    Shining,
    Radiant,
    Burning,
}

/// gcsim's "<name> (<element>)".
fn shell_name(k: Shell, e: Element) -> &'static str {
    use Element::*;
    match (k, e) {
        (Shell::Shining, Pyro) => "Shining Shadowhunt Shell (pyro)",
        (Shell::Shining, Hydro) => "Shining Shadowhunt Shell (hydro)",
        (Shell::Shining, Cryo) => "Shining Shadowhunt Shell (cryo)",
        (Shell::Shining, Electro) => "Shining Shadowhunt Shell (electro)",
        (Shell::Radiant, Pyro) => "Radiant Soulseeker Shell (pyro)",
        (Shell::Radiant, Hydro) => "Radiant Soulseeker Shell (hydro)",
        (Shell::Radiant, Cryo) => "Radiant Soulseeker Shell (cryo)",
        (Shell::Radiant, Electro) => "Radiant Soulseeker Shell (electro)",
        (Shell::Burning, Pyro) => "Burning Shadowhunt Shell (pyro)",
        (Shell::Burning, Hydro) => "Burning Shadowhunt Shell (hydro)",
        (Shell::Burning, Cryo) => "Burning Shadowhunt Shell (cryo)",
        (Shell::Burning, Electro) => "Burning Shadowhunt Shell (electro)",
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------- nightsoul

fn enter_nightsoul(s: &mut Sim, ci: usize) {
    s.ns_enter_timed(ci, 80.0, -1, CB_EXIT);
    let f = s.f;
    st_mut(s, ci).ns_src = f;
    s.add_task(Task::Char { c: ci as u8, id: TASK_NS_REDUCE, arg: f }, 6);
    s.chars[ci].normal_hit_num = 1;
    s.chars[ci].normal_counter = 0;
    let st = st_mut(s, ci);
    st.skill_particle_icd = false;
    st.c6_used = false;
}

/// gcsim's reduceNightsoulPoints and checkNS.
fn reduce_points(s: &mut Sim, ci: usize, val: f64) {
    s.ns_consume(ci, val);
    if s.ns_points(ci) < 0.001 {
        exit_nightsoul(s, ci);
    }
}

fn exit_nightsoul(s: &mut Sim, ci: usize) {
    if !s.ns_has_blessing(ci) {
        return;
    }
    match s.current_state() {
        // the charge goes on for up to 10 frames
        AnimState::Aim => s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_FIRE, arg: 0 }, g::SKILL_AIM_CHARGE_DELAY),
        AnimState::Idle => s.player.swap_cd = 37,
        _ => {}
    }
    s.ns_exit(ci);
    s.ns_clear(ci);
    st_mut(s, ci).ns_src = -1;
    s.set_cd(ci, Action::Skill, 390);
    s.chars[ci].normal_hit_num = g::NORMAL_HIT_NUM;
    s.chars[ci].normal_counter = 0;
    s.add_status(ci, PLUNGE_AVAILABLE, 26, true);
}

// ---------------------------------------------------------------- the blessing's actions

/// The tap shot: anemo Nightsoul damage in a fan, if the blessing still
/// holds at its hitmark.
fn attack_skill_tap(s: &mut Sim, ci: usize) -> ActInfo {
    let n = s.chars[ci].normal_counter as usize;
    let lvl = s.talent(ci, 0);
    // gcsim takes the normal attack's multiplier
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_CHASCA_TAP, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::ATTACK[n][lvl]);
    ai.abil = NORMAL_NAMES[n];
    ai.add_tags = add_tag::NIGHTSOUL;
    let windup = if s.current_state() == AnimState::Normal { 0 } else { 7 };
    // NewCircleHitFanAngle(player, target, {0, -3}, 8, 120)
    let dir = calc_direction(s.player_pos, s.enemy_pos);
    let (x, y) = offset_point(s.enemy_pos, (0.0, -3.0), dir);
    let pattern = Pattern::Fan { x, y, r: 8.0, fan: 120.0, dx: dir.0, dy: dir.1 };
    let st = st_mut(s, ci);
    let k = st.n_taps;
    st.n_taps += 1;
    st.taps[k as usize & 1] = (ai, pattern);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_TAP, arg: k & 1 }, windup + g::ATTACK_SKILL_TAP_HITMARK);
    s.advance_normal(ci);
    next_frames(s, ci, Frames::Const(0));
    let t = &g::ATTACK_SKILL_TAP_FRAMES;
    ActInfo::new(Frames::CharLive { id: FR_TAP, arg: 0 }, t[0], 1, AnimState::Normal)
}

/// Holding aim in the blessing: loads up to six bullets, then fires them.
fn aim_skill_hold(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
    let count = p.get("bullets").unwrap_or(6);
    if count > 6 {
        fail(s, "bullets must be <= 6");
        return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Aim);
    }
    if count <= 0 {
        fail(s, "bullets must be > 0");
        return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Aim);
    }
    if s.mod_active(ci, C6) && count < 6 {
        fail(s, "bullets must be 6 when c6 instant charge is active");
        return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Aim);
    }
    let count = count as i32;
    load_skill_hold_bullets(s, ci);
    let src = s.f;
    st_mut(s, ci).aim_src = src;
    c6_add_buff(s, ci);
    // these actions have the windup in their frames to aim
    let windup = if matches!(s.current_state(), AnimState::Normal | AnimState::Aim | AnimState::Skill | AnimState::Burst) { 0 } else { 11 };
    st_mut(s, ci).bullets_charged = 0;
    for i in 1..=count {
        let delay = charge_time(s, ci, i) + windup;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BULLET, arg: src << 3 | i }, delay);
    }
    let charge_delay = charge_time(s, ci, count) + windup;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_FIRE_AT, arg: src }, charge_delay);
    next_frames(s, ci, Frames::Const(0));
    let anim = charge_delay + g::SKILL_AIM_FRAMES[0] + g::SKILL_CANCEL_FRAMES[0];
    ActInfo::new(Frames::CharLive { id: FR_AIM, arg: charge_delay as i16 }, anim, 1, AnimState::Aim)
}

/// gcsim's fireBullets: the charged bullets, from the last loaded, each
/// on its own hitmark.
fn fire_bullets(s: &mut Sim, ci: usize) {
    if st(s, ci).aim_src < 0 {
        return;
    }
    let c6 = c6_buff(s, ci);
    let charged = st(s, ci).bullets_charged;
    if charged > 0 && !s.enemy_within(s.circle_on_player(0.0, 0.0, 10.0)) {
        fail(s, "no enemy within 10 of the player for the Shadowhunt Shells");
        return;
    }
    let f = s.f;
    let st = st_mut(s, ci);
    let k = st.n_fires;
    st.n_fires += 1;
    let mut fire = Fire { elems: [Element::Anemo; 6], f, c6, c2_armed: false };
    for i in 0..charged as usize {
        fire.elems[i] = st.to_fire[charged as usize - 1 - i];
    }
    st.fires[k as usize & 1] = fire;
    for i in 0..charged {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SHELL, arg: (k & 1) << 3 | i }, g::SKILL_AIM_HITMARKS[i as usize]);
    }
    let st = st_mut(s, ci);
    st.bullets_charged = 0;
    st.aim_src = -1;
}

/// A Shadowhunt Shell, or a Shining one of another element, snapshot as
/// it fires.
fn shell(s: &mut Sim, ci: usize, slot: usize, i: usize) {
    let lvl = s.talent(ci, 1);
    let fire = st(s, ci).fires[slot];
    let elem = fire.elems[i];
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_CHASCA_SHADOWHUNT, icd::GROUP_CHASCA_SHADOWHUNT, Element::Anemo, 25.0, g::SKILL_SHADOWHUNT[lvl]);
    ai.abil = "Shadowhunt Shell";
    ai.add_tags = add_tag::NIGHTSOUL;
    ai.hitlag_factor = 0.01;
    if elem != Element::Anemo {
        ai.abil = shell_name(Shell::Shining, elem);
        ai.icd_tag = icd::TAG_CHASCA_SHINING;
        ai.icd_group = icd::GROUP_CHASCA_SHINING;
        ai.element = elem;
        ai.mult = g::SKILL_SHINING[lvl];
        // c2cb(bulletFireFrame), nil before C2
        st_mut(s, ci).fires[slot].c2_armed = s.chars[ci].cons >= 2;
    }
    ai.pattern = Pattern::Single;
    let mut cbs = CB_PARTICLE;
    if st(s, ci).fires[slot].c2_armed {
        cbs |= CB_C2[slot];
    }
    let mut ev = s.snapshot_event(ai, 0);
    if fire.c6 {
        ev.stats[st::CD] += 1.2;
    }
    s.queue_attack_with_snap(ev, cbs, 0);
}

/// C2's and C4's extra hit: 400% ATK of the element that hit, around the
/// enemy.
fn aoe(s: &mut Sim, ci: usize, abil: &'static str, elem: Element) {
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, elem, 25.0, 4.0);
    ai.abil = abil;
    ai.add_tags = add_tag::NIGHTSOUL;
    ai.pattern = s.circle_on_target(5.0);
    s.queue_attack(ai, 0, 0, 1);
}

// ---------------------------------------------------------------- bullets

/// gcsim's loadSkillHoldBullets: the next bullets become the ones to fire,
/// and the next ones are drawn.
fn load_skill_hold_bullets(s: &mut Sim, ci: usize) {
    let x = st_mut(s, ci);
    std::mem::swap(&mut x.to_fire, &mut x.next);
    reset_pool(x);
    x.next[0] = Element::Anemo;
    x.next[1] = Element::Anemo;
    let third = a1_conversion(s, ci);
    st_mut(s, ci).next[2] = third;
    // c1Conversion
    if s.chars[ci].cons >= 1 && third != Element::Anemo {
        let n = st(s, ci).n_uniq;
        let k = s.rng.intn(n as i32) as usize;
        let e = st(s, ci).uniq[k];
        st_mut(s, ci).next[1] = e;
    }
    let n_phec = st(s, ci).n_phec;
    for (i, need) in [(3, 3), (4, 2), (5, 1)] {
        let e = if n_phec < need { Element::Anemo } else { random_from_pool(s, ci) };
        st_mut(s, ci).next[i] = e;
    }
}

fn reset_pool(st: &mut State) {
    st.pool = st.phec;
    st.n_pool = st.n_phec;
}

fn random_from_pool(s: &mut Sim, ci: usize) -> Element {
    if st(s, ci).n_pool == 0 {
        reset_pool(st_mut(s, ci));
    }
    let n = st(s, ci).n_pool;
    let ind = s.rng.intn(n as i32) as usize;
    let st = st_mut(s, ci);
    let e = st.pool[ind];
    st.pool[ind] = st.pool[n - 1];
    st.n_pool -= 1;
    e
}

/// A1: the third bullet may take one of the party's distinct elements.
fn a1_conversion(s: &mut Sim, ci: usize) -> Element {
    if s.chars[ci].ascension < 1 {
        return Element::Anemo;
    }
    let n = st(s, ci).n_uniq;
    if n == 0 {
        return Element::Anemo;
    }
    let mut chance = g::A1_CONVERSION_CHANCE[n];
    if s.chars[ci].cons >= 1 {
        chance += 0.333;
    }
    if s.rng.float64() > chance {
        return Element::Anemo;
    }
    let k = s.rng.intn(n as i32) as usize;
    st(s, ci).uniq[k]
}

// ---------------------------------------------------------------- C6

/// C6: a bullet of another element loaded makes the next charge instant,
/// once every 3 s.
fn c6(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 6 || s.chars[ci].ascension < 1 || s.mod_active(ci, C6_ICD) {
        return;
    }
    s.add_status(ci, C6_ICD, 3 * 60, true);
    s.add_status(ci, C6, 3 * 60, true);
}

/// The charge starting now takes C6's instant charge.
fn c6_add_buff(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 6 || s.chars[ci].ascension < 1 || !s.mod_active(ci, C6) {
        return;
    }
    st_mut(s, ci).c6_used = true;
    s.delete_mod(ci, C6);
}

/// The instant charge's CRIT DMG +120% on the bullets it fires.
fn c6_buff(s: &mut Sim, ci: usize) -> bool {
    if s.chars[ci].cons < 6 || s.chars[ci].ascension < 1 || !st(s, ci).c6_used {
        return false;
    }
    st_mut(s, ci).c6_used = false;
    true
}

/// gcsim's c6ChargeTime: the frames to load `count` bullets.
fn charge_time(s: &Sim, ci: usize, count: i32) -> i32 {
    let i = count as usize - 1;
    if s.chars[ci].cons < 6 {
        return g::CUMU_SKILL_AIM_LOAD_FRAMES[i];
    }
    if st(s, ci).c6_used {
        return g::CUMU_SKILL_AIM_LOAD_FRAMES_C6_INSTANT[i];
    }
    g::CUMU_SKILL_AIM_LOAD_FRAMES_C6[i]
}
