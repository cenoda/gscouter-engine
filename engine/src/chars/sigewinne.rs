//! Sigewinne, from gcsim's internal/characters/sigewinne: normal attacks,
//! aimed shots (physical, charged, and charged with the Mini-Stration
//! Bubble), Rebound Hydrotherapy (press, `hold=1` and `hold=2`, its
//! bouncing bubble, heals, droplets and Spiritbreath Thorn), Super
//! Saturated Syringing (its ticks, the `ticks` early cancel and the swap
//! cooldown), the dash's `pickup_droplets`, the energy from a cleared Bond
//! of Life, A1, A4 and every constellation.
//!
//! Her Sourcewater Droplets are gadgets of Neuvillette's droplet id (two
//! per skill, at gcsim's random points); her pickups and his take both
//! kinds, as gcsim's getSourcewaterDroplets does (`neuvillette::droplets`).
//! Each one she takes is 10% of her max HP as Bond of Life.
//!
//! The bubble's hits use the snapshot of the skill's cast (gcsim's
//! QueueAttackWithSnap); its tier's DMG bonus, A1's hydro bonus and C6's
//! crit are attack mods, read as each hit lands.
//!
//! The burst's length grows with its ticks (a frames function reading
//! tickAnimLength in gcsim): the frames are read live (`Frames::CharLive`).

use crate::chars::neuvillette;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::sigewinne as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The skill's attack with its snapshot from the cast.
    skill_ev: Option<AttackEvent>,
    particle_generated: bool,
    last_summon_src: i32,
    bubble_limit: i32,
    tier: i32,
    /// Bond of Life gained since it was last cleared (gcsim's float32).
    collected: f32,
    early_cancelled: bool,
    tick_anim: i32,
    burst_max: i32,
    burst_start: i32,
    /// The `ticks` param of the burst under way (-1 without).
    max_tick: i32,
    last_swap: i32,
    /// gcsim's c2Shield: the shield of the skill or burst under way.
    c2_shield: bool,
    /// The convalescence tag: A1's flat damage left to give.
    conv: i32,
}

const SKILL_STATUS: ModKey = ModKey::new(g::SKILL_KEY);
const CONVALESCENCE: ModKey = ModKey::new(g::CONVALESCENCE_KEY);
const ALIGNED_ICD: ModKey = ModKey::new(g::SKILL_ALIGNED_ICD_KEY);
const TIER_MOD: ModKey = ModKey::new("sigewinne-bubble-tier");
const A1: ModKey = ModKey::new("sigewinne-a1");
const A4: ModKey = ModKey::new("sigewinne-a4");
const C2_RES: ModKey = ModKey::new("sigewinne-c2");
const C6: ModKey = ModKey::new("sigewinne-c6");
const C2_SHIELD: ModKey = ModKey::new("Bubbly Shield (C2)");

const SKILL_NAME: &str = "Rebound Hydrotherapy";

const CB_PARTICLE: u16 = 1 << 0;
const CB_C2: u16 = 1 << 1;

/// The droplets of a skill.
const TASK_DROPLETS: u8 = 0;
/// A bounce of the bubble: arg is the cast's frame * 16 + the bounce.
const TASK_BUBBLE: u8 = 1;
/// The burst starts its ticks after its charge.
const TASK_BURST_START: u8 = 2;
/// A tick: arg is the burst's start * 64 + the tick.
const TASK_BURST_TICK: u8 = 3;
/// The tick at the end of the burst's longest duration.
const TASK_BURST_LAST: u8 = 4;

const FR_BURST: u8 = 0;

const ON_REMOVED_SKILL: u8 = 0;
const ON_REMOVED_BURST: u8 = 1;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];

impl Character for Char {
    const KEY: &'static str = "sigewinne";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            skill_ev: None,
            particle_generated: false,
            last_summon_src: 0,
            bubble_limit: 5,
            tier: 0,
            collected: 0.0,
            early_cancelled: false,
            tick_anim: 0,
            burst_max: 241 - g::CHARGE_BURST_DUR,
            burst_start: 0,
            max_tick: -1,
            last_swap: 0,
            c2_shield: false,
            conv: 0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        if asc >= 1 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        if asc >= 4 {
            s.add_mod(ci, Mod::owned(A4, ModKind::Stat, o, st::HEAL, false, false, -1), -1);
        }
        let st = s.st_mut::<State>(ci);
        if cons >= 1 {
            st.bubble_limit += 3;
        }
        if cons >= 4 {
            st.burst_max = 425 - g::CHARGE_BURST_DUR;
        }
        s.add_mod(ci, Mod::owned(TIER_MOD, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        s.subscribe(EventKind::HpDebt, o);
        s.subscribe(EventKind::Swap, o);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let other = |keys: &[&str]| p.0.iter().find(|(k, _)| !keys.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"));
        match a {
            Action::Attack => other(&["travel"]),
            // gcsim indexes the skill's frames by hold: any other value panics
            Action::Skill if !matches!(p.get("hold"), None | Some(0..=2)) => Some(format!("{name}[hold] other than 0, 1 or 2")),
            Action::Skill => other(&["hold"]),
            Action::Burst => other(&["ticks"]),
            Action::Aim => other(&["hold", "travel", "weakspot"]),
            _ => Some(name.into()),
        }
    }

    /// gcsim's Aimed rejects any other hold.
    fn invalid(s: &Sim, _: usize, a: Action, p: &Params) -> Option<String> {
        if a == Action::Aim {
            if let Some(h) = p.get("hold") {
                if !(0..=2).contains(&h) {
                    return Some(format!("invalid hold param supplied, got {h}"));
                }
            }
        }
        crate::content::template_invalid(s, a)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match (fields[0].as_str(), fields.get(1).map(|x| x.as_str())) {
            ("tags", Some(g::CONVALESCENCE_KEY)) => Some(Ok(Num::Int(s.st::<State>(ci).conv as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if early_cancel_error(s, ci, "Normal Attack") {
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Normal);
        }
        let n = s.chars[ci].normal_counter as usize;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = if n != 0 { s.circle_on_target(1.0) } else { s.box_on_target(0.0, -0.5, 0.1, 1.0) };
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if early_cancel_error(s, ci, "Aimed shot") {
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Aim);
        }
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_SIGEWINNE, Element::Hydro, 25.0, g::FULLAIM[lvl]);
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
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let h = g::AIMED_HITMARKS[hold];
        s.queue_attack(ai, 0, h, h + travel);
        if hold == 2 {
            let mut mini = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_SIGEWINNE, Element::Hydro, 25.0, g::MINI_STRATION_BUBBLE_DMG[lvl]);
            mini.abil = "Mini-Stration Bubble";
            mini.hitlag_factor = 0.01;
            mini.pattern = s.box_on_target(0.0, 0.0, 0.3, 0.3);
            s.queue_attack(mini, 0, h + 1, h + 1 + 7 * travel);
        }
        let t = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Aim)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if early_cancel_error(s, ci, "Elemental Skill") {
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Skill);
        }
        let hold = p.get("hold").unwrap_or(0) as usize;
        let (hitmark, cd_start) = match hold {
            1 => (g::SKILL_SHORT_HOLD_HITMARK, g::SKILL_SHORT_HOLD_CD_START),
            2 => (g::SKILL_HOLD_HITMARK, g::SKILL_HOLD_CD_START),
            _ => (g::SKILL_PRESS_HITMARK, g::SKILL_PRESS_CD_START),
        };
        s.st_mut::<State>(ci).tier = hold as i32;
        // generateSkillSnapshot
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_SIGEWINNE, Element::Hydro, 25.0, 0.0);
        ai.abil = SKILL_NAME;
        ai.flat_dmg = g::BOLSTERING_BUBBLEBALM_DMG[lvl] * s.max_hp(ci);
        ai.hitlag_factor = 0.02;
        let ev = s.snapshot_event(ai, 0);
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        st.skill_ev = Some(ev);
        st.particle_generated = false;
        st.last_summon_src = f;
        s.add_status(ci, SKILL_STATUS, -1, false);
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD * 60, cd_start);
        s.add_task(Task::Char { c: ci as u8, id: TASK_DROPLETS, arg: 0 }, cd_start + g::SKILL_DROPLET_SPAWN_TIME_OFFSET);
        s.add_task(Task::Char { c: ci as u8, id: TASK_BUBBLE, arg: f * 16 }, hitmark);
        if s.chars[ci].ascension >= 1 {
            // a1Self
            s.add_status(ci, CONVALESCENCE, g::SKILL_CD * 60, true);
            s.st_mut::<State>(ci).conv = 10;
            s.add_mod(ci, Mod::owned(A1, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), g::SKILL_CD * 60);
        }
        add_c2_shield(s, ci);
        let t = &g::SKILL_FRAMES[hold];
        let mut info = ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Skill);
        info.on_removed = OnRemoved::Char(ON_REMOVED_SKILL);
        info
    }

    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if early_cancel_error(s, ci, "Elemental Burst") {
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Burst);
        }
        let ticks = match p.get("ticks") {
            None => -1,
            Some(t) => t.clamp(2, i32::MAX as i64) as i32,
        };
        s.player.swap_cd = i16::MAX as i32;
        let st = s.st_mut::<State>(ci);
        st.tick_anim = burst_hitmark(1);
        st.max_tick = ticks;
        let max = st.burst_max;
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_START, arg: 0 }, g::CHARGE_BURST_DUR);
        s.set_cd_with_delay(ci, Action::Burst, 18 * 60, 1);
        s.consume_energy(ci, 5);
        add_c2_shield(s, ci);
        let mut info = ActInfo::new(Frames::CharLive { id: FR_BURST, arg: 0 }, g::CHARGE_BURST_DUR + max, g::CHARGE_BURST_DUR, AnimState::Burst);
        info.on_removed = OnRemoved::Char(ON_REMOVED_BURST);
        info
    }

    fn frames_live(s: &Sim, ci: usize, _: u8, _: i16, next: Action) -> i32 {
        g::CHARGE_BURST_DUR + s.st::<State>(ci).tick_anim + g::END_LAG[next as usize]
    }

    fn on_removed(s: &mut Sim, ci: usize, id: u8, next: AnimState) {
        // the swap cooldown after a burst cut short by a dash or jump
        if id == ON_REMOVED_BURST && matches!(next, AnimState::Dash | AnimState::Jump) {
            let since = s.f - s.st::<State>(ci).last_swap;
            s.player.swap_cd = (60 - since).max(0);
        }
        remove_c2_shield(s, ci);
    }

    /// The dash: clears the early cancel and takes `pickup_droplets`
    /// droplets (the first in range, in the order of Go's rand.Perm),
    /// then the template's dash.
    fn own_dash(s: &mut Sim, ci: usize, p: &Params) -> Option<ActInfo> {
        s.st_mut::<State>(ci).early_cancelled = false;
        let want = p.get("pickup_droplets").unwrap_or(0);
        if want == 0 {
            return None;
        }
        let (found, n) = neuvillette::droplets(s, false);
        let k = (want.max(0) as usize).min(n);
        let perm = perm(s, k);
        let mut picked = [0usize; neuvillette::MAX_DROPLETS];
        for i in 0..k {
            picked[i] = found[perm[i]];
        }
        consume(s, ci, &mut picked[..k]);
        None
    }

    fn own_jump(s: &mut Sim, ci: usize, _: &Params) -> Option<ActInfo> {
        s.st_mut::<State>(ci).early_cancelled = false;
        None
    }

    fn own_walk(s: &mut Sim, ci: usize, _: &Params) -> Option<ActInfo> {
        if early_cancel_error(s, ci, "Walk") {
            return Some(ActInfo::new(Frames::Const(1), 1, 1, AnimState::Walk));
        }
        None
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_DROPLETS => {
                for _ in 0..2 {
                    let (cx, cy) = s.point_from_player(0.0, 1.5);
                    let (x, y) = s.random_point(cx, cy, 0.3, 1.0);
                    neuvillette::add_droplet(s, ci, x, y);
                }
            }
            TASK_BUBBLE => bubble(s, ci, arg / 16, arg % 16),
            TASK_BURST_START => {
                burst_find_droplets(s, ci);
                let f = s.f;
                s.st_mut::<State>(ci).burst_start = f;
                s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_TICK, arg: f * 64 + 1 }, burst_hitmark(1));
            }
            TASK_BURST_TICK => burst_tick(s, ci, arg / 64, arg % 64, false),
            TASK_BURST_LAST => burst_tick(s, ci, arg / 64, arg % 64, true),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.st::<State>(ci).particle_generated {
            s.st_mut::<State>(ci).particle_generated = true;
            s.queue_particle(g::PARTICLE_COUNT as f64, Element::Hydro, s.particle_delay[ci]);
        }
        if cbs & CB_C2 != 0 && s.chars[ci].cons >= 2 && ev.damage != 0.0 {
            s.enemy_add_resist(C2_RES, Element::Hydro, -0.35, 8 * 60);
        }
    }

    /// A4: healing bonus from the party's Bond of Life.
    fn mod_amount(s: &Sim, _: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let mut total = 0.0;
        for k in 0..s.chars.len() {
            total += s.hp_debt(k);
        }
        out[st::HEAL] += (total * g::A4_HP_DEBT_HEALING_BONUS_RATIO).min(g::A4_HEALING_BONUS_CAP);
        true
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == TIER_MOD {
            let tier = s.st::<State>(ci).tier;
            if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) && tier != 0 && ev.info.abil == SKILL_NAME {
                ev.stats[st::DMGP] += tier as f64 * g::BUBBLE_TIER_BUFF;
            }
        } else if m.key == A1 {
            ev.stats[st::HYDROP] += g::A1_DMG_BUFF;
        } else if m.key == C6 {
            if ev.info.tag == AttackTag::ElementalBurst {
                let hp = s.max_hp(ci);
                ev.stats[st::CD] += (hp * g::C6_C_DMG_HP_RATIO).min(g::C6_C_DMG_CAP);
                ev.stats[st::CR] += (hp * g::C6_C_RATE_HP_RATIO).min(g::C6_C_RATE_CAP);
            }
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // A1: another member's skill hit while it is off-field adds
            // flat damage from her max HP above 30,000
            Event::EnemyHit(ev) => {
                if !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
                    return;
                }
                let actor = ev.info.actor as usize;
                if actor == ci || s.player.active as usize == actor {
                    return;
                }
                if !s.mod_active(ci, CONVALESCENCE) || s.st::<State>(ci).conv == 0 {
                    return;
                }
                s.st_mut::<State>(ci).conv -= 1;
                let hp = (s.max_hp(ci) - g::BASE_CONVALESCENCE_HP as f64).max(0.0);
                let amt = if s.chars[ci].cons >= 1 {
                    (hp * g::C1_FLAT_CONVALESCENCE_INCREASE).min(g::C1_FLAT_CONVALESCENCE_CAP as f64)
                } else {
                    (hp * g::FLAT_CONVALESCENCE_INCREASE).min(g::FLAT_CONVALESCENCE_CAP as f64)
                };
                s.hit_flat += amt;
            }
            // energyBondClearMod
            Event::HpDebt { on, change } => {
                if on != ci {
                    return;
                }
                if change < 0.0 {
                    s.st_mut::<State>(ci).collected += -(change as f32);
                }
                if s.hp_debt(ci) > 0.0 {
                    return;
                }
                let collected = s.st::<State>(ci).collected;
                if collected < 0.0001 {
                    return;
                }
                let amt = (collected / g::HP_DEBT_ENERGY_RATIO as f32).min(5.0);
                s.st_mut::<State>(ci).collected = 0.0;
                s.add_energy(ci, amt as f64);
            }
            Event::Swap { next, .. } => {
                if next == ci {
                    s.st_mut::<State>(ci).last_swap = s.f;
                }
            }
            _ => {}
        }
    }
}

/// gcsim's error from an action after a burst cut short by `ticks`.
fn early_cancel_error(s: &mut Sim, ci: usize, what: &str) -> bool {
    if !s.st::<State>(ci).early_cancelled {
        return false;
    }
    if s.error.is_none() {
        s.error = Some(format!("sigewinne: Cannot early cancel Super Saturated Syringing with {what}"));
    }
    s.done = true;
    true
}

/// Go's rand.Perm(n).
fn perm(s: &mut Sim, n: usize) -> [usize; neuvillette::MAX_DROPLETS] {
    let mut m = [0usize; neuvillette::MAX_DROPLETS];
    for i in 0..n {
        let j = s.rng.intn(i as i32 + 1) as usize;
        m[i] = m[j];
        m[j] = i;
    }
    m
}

/// consumeDroplet for the droplets at these indices: each is removed and
/// is 10% of her max HP as Bond of Life.
fn consume(s: &mut Sim, ci: usize, idx: &mut [usize]) {
    idx.sort_unstable();
    for &k in idx.iter().rev() {
        s.gadgets.remove(k);
    }
    for _ in 0..idx.len() {
        let amt = s.max_hp(ci) * g::BO_L_PCT_PER_DROPLET;
        s.modify_hp_debt(ci, amt);
    }
}

/// burstFindDroplets: every droplet in range.
fn burst_find_droplets(s: &mut Sim, ci: usize) {
    let (found, n) = neuvillette::droplets(s, false);
    let perm = perm(s, n);
    let mut picked = [0usize; neuvillette::MAX_DROPLETS];
    for i in 0..n {
        picked[i] = found[perm[i]];
    }
    consume(s, ci, &mut picked[..n]);
}

fn add_c2_shield(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 2 {
        return;
    }
    let hp = 0.3 * s.max_hp(ci);
    let expiry = s.f + 15 * 60;
    s.add_shield(Shield { owner: ci as u8, key: C2_SHIELD, typ: ShieldType::SigewinneC2, target: ci as i8, ele: Element::Hydro, hp, expiry, em_bonus: 0.0 });
    s.st_mut::<State>(ci).c2_shield = true;
}

fn remove_c2_shield(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 2 || !s.st::<State>(ci).c2_shield {
        return;
    }
    let f = s.f;
    s.set_shield_expiry(ShieldType::SigewinneC2, f + 1);
    s.st_mut::<State>(ci).c2_shield = false;
}

/// bolsteringBubblebalm: one bounce of the skill's bubble.
fn bubble(s: &mut Sim, ci: usize, src: i32, tick: i32) {
    if src != s.st::<State>(ci).last_summon_src || !s.mod_active(ci, SKILL_STATUS) {
        return;
    }
    let Some(mut ev) = s.st::<State>(ci).skill_ev else { return };
    ev.info.pattern = s.circle_on_target(g::BUBBLE_RADIUS as f64);
    s.queue_attack_with_snap(ev, CB_PARTICLE | CB_C2, 0);
    surging_blade(s, ci);
    bubble_healing(s, ci);
    if s.chars[ci].cons >= 1 {
        s.st_mut::<State>(ci).conv += 1;
    }
    let limit = s.st::<State>(ci).bubble_limit;
    if tick == limit - 1 {
        bubble_final_healing(s, ci);
        s.delete_mod(ci, SKILL_STATUS);
        return;
    }
    // bubbleTierLoseTask
    if s.chars[ci].cons < 1 || tick > 2 {
        let st = s.st_mut::<State>(ci);
        st.tier = (st.tier - 1).max(0);
    }
    s.add_task(Task::Char { c: ci as u8, id: TASK_BUBBLE, arg: src * 16 + tick + 1 }, g::BUBBLE_HIT_INTERVAL);
}

/// The Spiritbreath Thorn, once in 10 s.
fn surging_blade(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, ALIGNED_ICD) {
        return;
    }
    s.add_status(ci, ALIGNED_ICD, g::SKILL_ALIGNED_ICD, true);
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 0.0, 0.0);
    ai.abil = "Spiritbreath Thorn (sigewinne)";
    ai.flat_dmg = g::SURGING_BLADE_DMG[lvl] * s.max_hp(ci);
    ai.hitlag_factor = 0.01;
    ai.pattern = s.circle_on_target(3.0);
    s.queue_attack(ai, 0, g::SKILL_ALIGNED_HITMARK, g::SKILL_ALIGNED_HITMARK);
}

/// The bounce's heal of every other member, each followed by C6.
fn bubble_healing(s: &mut Sim, ci: usize) {
    if !s.mod_active(ci, SKILL_STATUS) {
        return;
    }
    let lvl = s.talent(ci, 1);
    for k in 0..s.chars.len() {
        if k == ci {
            continue;
        }
        let bonus = s.st::<State>(ci).tier as f64 * g::BUBBLE_TIER_BUFF;
        let src = g::BOLSTERING_BUBBLEBALM_HEALING_PCT[lvl] * s.max_hp(ci) + g::BOLSTERING_BUBBLEBALM_HEALING_FLAT[lvl];
        let bonus = s.stat(ci, st::HEAL) + bonus;
        s.heal(ci, k, src, bonus);
        c6_crit(s, ci);
    }
}

/// The last bounce heals her.
fn bubble_final_healing(s: &mut Sim, ci: usize) {
    if !s.mod_active(ci, SKILL_STATUS) {
        return;
    }
    let bonus = s.st::<State>(ci).tier as f64 * g::BUBBLE_TIER_BUFF;
    let src = g::FINAL_BOUNCE_HEALING * s.max_hp(ci);
    let bonus = s.stat(ci, st::HEAL) + bonus;
    s.heal(ci, ci, src, bonus);
    c6_crit(s, ci);
}

fn c6_crit(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 6 {
        return;
    }
    s.add_mod(ci, Mod::owned(C6, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 15 * 60);
}

fn burst_hitmark(tick: i32) -> i32 {
    if tick == 1 {
        0
    } else {
        25
    }
}

/// burstTick: `last` is the tick queued for the end of the burst's
/// longest duration.
fn burst_tick(s: &mut Sim, ci: usize, src: i32, tick: i32, last: bool) {
    let st = s.st::<State>(ci);
    if st.burst_start != src || s.f > st.burst_start + st.burst_max {
        return;
    }
    if last {
        s.player.swap_cd = g::END_LAG[Action::Swap as usize];
        return;
    }
    let max_tick = st.max_tick;
    if tick == max_tick {
        burst_wave(s, ci);
        s.st_mut::<State>(ci).early_cancelled = true;
        s.player.swap_cd = g::END_LAG[Action::Swap as usize];
        return;
    }
    burst_wave(s, ci);
    if max_tick == -1 || tick < max_tick {
        let delay = burst_hitmark(tick + 1);
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        let next_len = f - st.burst_start + delay;
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_TICK, arg: src * 64 + tick + 1 }, delay);
        let st = s.st_mut::<State>(ci);
        if next_len > st.burst_max {
            let d = st.burst_max - st.tick_anim;
            st.tick_anim = st.burst_max;
            s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_LAST, arg: src * 64 + tick + 1 }, d);
        } else {
            st.tick_anim = next_len;
        }
    }
}

fn burst_wave(s: &mut Sim, ci: usize) {
    // gcsim reads the burst's damage at the normal attack's talent level
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_SIGEWINNE_BURST, Element::Hydro, 25.0, 0.0);
    ai.abil = "Super Saturated Syringing";
    ai.flat_dmg = g::BURST_DMG[lvl] * s.max_hp(ci);
    ai.hitlag_factor = 0.01;
    ai.pattern = s.box_on_player(0.0, 0.0, 4.0, 10.0);
    s.queue_attack(ai, CB_C2, 0, 0);
}
