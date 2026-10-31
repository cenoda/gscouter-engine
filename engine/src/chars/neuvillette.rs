//! Neuvillette, from gcsim's internal/characters/neuvillette: normal
//! attacks, the short charged attack and Equitable Judgment (its legal
//! evaluation, ticks, HP drain and `ticks` early cancel), O Tears, I Shall
//! Repay with its Spiritbreath Thorn, O Tides, I Have Returned, the
//! Sourcewater Droplets they leave, A1, A4 and every constellation.
//!
//! Droplets are gadgets of radius 1 at gcsim's random points (its
//! CalcRandomPointFromCenter, with the draws in gcsim's order). A charged
//! attack takes, in a random order (Go's rand.Perm), up to three whose
//! centers are in the box or the fan in front of the player
//! (getSourcewaterDroplets: gcsim's IsWithinArea is PointInShape, the
//! center only); C6 takes the first within 15. At most twelve exist; a
//! thirteenth removes the oldest. Hydro Traveler's and Sigewinne's
//! droplets, which gcsim's charged attack takes too, are not ported.
//!
//! The judgment's length grows while it lasts (a frames function that
//! reads tickAnimLength in gcsim): the frames are the action's own, from
//! an argument updated whenever that length or the evaluation's changes.
//!
//! A1 counts the reactions gcsim lists: bloom, hydro crystallize,
//! electro-charged, frozen, hydro swirl and vaporize on the enemy, and
//! Lunar-Bloom, Lunar-Charged and Lunar-Crystallize, which the engine does
//! not have (nothing ported enables them: lunar-bloom is an error). A4
//! starts at 0: gcsim computes it in Init, before the starting HP is set,
//! from a ratio of 0, and then only on his drains and heals.

use crate::content::{Character, Event};
use crate::gcsl::Num;
use crate::gcsimdata::chars::neuvillette as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The frame he was last swapped in (for the swap cooldown after an
    /// early cancel).
    last_swap: i32,
    judge_start: i32,
    judge_dur: i32,
    tick_anim: i32,
    tick_anim_c6: i32,
    early_cancelled: bool,
    a1_base: usize,
    a4: f64,
    /// The judgment's flat damage (gcsim's chargeAi.FlatDmg).
    charge_flat: f64,
    /// windup + 3 + the legal evaluation left: the judgment's frames
    /// before its tick length.
    frames_base: i32,
    /// The windup and ticks param of the judgment just started, for its
    /// evaluation task.
    windup: i32,
    ticks: i32,
    /// The next non-final tick of the current judgment, and its limit.
    tick: i32,
    max_tick: i32,
    /// The burst's flat damage of its first hit and its waterfalls, from
    /// his max HP as it was cast.
    burst_flat: [f64; 2],
}

const DROPLET: u8 = 0;
const DROPLET_DURATION: i32 = 878;
const DROPLET_LIMIT: usize = 12;

const A4: ModKey = ModKey::new("neuvillette-a4");
const C2: ModKey = ModKey::new("neuvillette-c2");
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const ALIGNED_ICD: ModKey = ModKey::new(g::SKILL_ALIGNED_ICD_KEY);

/// A1's statuses, in gcsim's order. The lunar ones are never added.
const A1_KEYS: [ModKey; 9] = [
    ModKey::new("neuvillette-a1-bloom"),
    ModKey::new("neuvillette-a1-lunarbloom"),
    ModKey::new("neuvillette-a1-crystallize-hydro"),
    ModKey::new("neuvillette-a1-electro-charged"),
    ModKey::new("neuvillette-a1-lunar-charged"),
    ModKey::new("neuvillette-a1-lunar-crystallize"),
    ModKey::new("neuvillette-a1-frozen"),
    ModKey::new("neuvillette-a1-swirl-hydro"),
    ModKey::new("neuvillette-a1-vaporize"),
];

const CB_DROPLETS: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;
const CB_C6: u16 = 1 << 2;

const TASK_LEGAL: u8 = 0;
const TASK_SHORT: u8 = 1;
const TASK_TICK: u8 = 2;
const TASK_TICK_LAST: u8 = 3;
const TASK_HP: u8 = 4;
const TASK_C6_CHECK: u8 = 5;
const TASK_DROPLET_HEAL: u8 = 6;
const TASK_BURST: u8 = 7;
const TASK_THORN: u8 = 8;

const FR_JUDGE: u8 = 0;
const FR_SHORT: u8 = 1;

const ON_REMOVED_JUDGE: u8 = 0;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];
const C6_NAME: &str = "Charged Attack: Equitable Judgment (C6)";

impl Character for Char {
    const KEY: &'static str = "neuvillette";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        if asc >= 1 {
            s.subscribe(EventKind::Reaction, o);
        }
        if asc >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Stat, o, st::HYDROP, true, false, -1));
            s.subscribe(EventKind::Drain, o);
            // updateA4 in Init reads a ratio of 0
            s.st_mut::<State>(ci).a4 = a4_value(0.0);
        }
        if cons >= 1 && asc >= 1 {
            s.st_mut::<State>(ci).a1_base = 1;
        }
        if cons >= 2 && asc >= 1 {
            s.chars[ci].mods.push(Mod::owned(C2, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        // A4's and C4's OnHeal handlers, both from his Init: one
        // subscription runs them in that order
        if asc >= 4 || cons >= 4 {
            s.subscribe(EventKind::Heal, o);
        }
        s.subscribe(EventKind::Swap, o);
        // early cancels: dash and jump clear it, walk is an error
        s.subscribe(EventKind::StateChange, o);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            Action::Charge => p
                .0
                .iter()
                .find(|(k, _)| k != "short" && k != "ticks")
                .map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    /// gcsim's ActionStam: his charged attacks cost no stamina up front.
    fn charge_stam(_: &Sim, _: usize) -> Option<f64> {
        Some(0.0)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "droplets" => Some(Ok(Num::Int(droplets(s, ci, false).1 as i64))),
            "droplets-c6" => Some(Ok(Num::Int(droplets(s, ci, true).1 as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let table = &g::ATTACK_FRAMES[n];
        if s.st::<State>(ci).early_cancelled {
            early_cancel_error(s, "Normal Attack");
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Normal);
        }
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.circle_on_target(g::ATTACK_HITBOXES[n]);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], table[Action::Swap as usize], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.st::<State>(ci).early_cancelled {
            early_cancel_error(s, "Charged Attack");
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Charge);
        }
        // a windup out of dash, jump, walk and swap, or from idle
        let windup = match s.current_state() {
            AnimState::Idle | AnimState::Dash | AnimState::Jump | AnimState::Walk | AnimState::Swap => 14,
            _ => 0,
        };
        if p.get("short").unwrap_or(0) != 0 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SHORT, arg: windup }, windup + 3);
            let t = &g::CHARGE_FRAMES;
            return ActInfo::new(Frames::Char { id: FR_SHORT, arg: windup as i16 }, windup + t[0], windup + t[Action::Dash as usize], AnimState::Charge);
        }
        let ticks = match p.get("ticks") {
            None => -1,
            Some(t) => t.max(1).min(i32::MAX as i64) as i32,
        };
        let st = s.st_mut::<State>(ci);
        st.judge_dur = 0;
        st.tick_anim = tick_delay(1);
        st.windup = windup;
        st.ticks = ticks;
        // the evaluation's length is 0 until it starts
        st.frames_base = windup + 3;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_LEGAL, arg: 0 }, windup + 3);
        let arg = (windup + 3 + tick_delay(1)) as i16;
        let mut info = ActInfo::new(Frames::Char { id: FR_JUDGE, arg }, 1200, windup + 3 + g::END_LAG[Action::Dash as usize], AnimState::Charge);
        info.on_removed = OnRemoved::Char(ON_REMOVED_JUDGE);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.st_mut::<State>(ci).early_cancelled = false;
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
        ai.abil = "O Tears, I Shall Repay";
        ai.flat_dmg = g::SKILL[lvl] * s.max_hp(ci);
        // at the enemy's position
        ai.pattern = s.circle_on_target(6.0);
        let h = g::SKILL_HITMARKS[0];
        s.queue_attack(ai, CB_DROPLETS | CB_PARTICLE, h, h);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_THORN, arg: 0 }, h);
        s.set_cd_with_delay(ci, Action::Skill, 12 * 60, 20);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Charge as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.st_mut::<State>(ci).early_cancelled = false;
        let lvl = s.talent(ci, 2);
        let hp = s.max_hp(ci);
        s.st_mut::<State>(ci).burst_flat = [g::BURST[lvl] * hp, g::BURST_WATERFALL[lvl] * hp];
        for i in 0..3 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST, arg: i }, g::BURST_HITMARKS[i as usize]);
        }
        s.set_cd(ci, Action::Burst, 18 * 60);
        s.consume_energy(ci, 4);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_JUDGE => arg as i32 + g::END_LAG[next as usize],
            _ => arg as i32 + g::CHARGE_FRAMES[next as usize],
        }
    }

    /// The swap cooldown after a judgment cancelled by a skill, burst,
    /// dash or jump.
    fn on_removed(s: &mut Sim, ci: usize, _: u8, next: AnimState) {
        if matches!(next, AnimState::Skill | AnimState::Burst | AnimState::Dash | AnimState::Jump) {
            let since = s.f - s.st::<State>(ci).last_swap;
            s.player.swap_cd = (60 - since).max(0);
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_LEGAL => legal_eval(s, ci),
            TASK_SHORT => short_charge(s, ci, arg),
            TASK_TICK => judgment_tick(s, ci, arg, false),
            TASK_TICK_LAST => judgment_tick(s, ci, arg, true),
            TASK_HP => consume_hp(s, ci, arg),
            TASK_C6_CHECK => c6_droplet_check(s, ci, arg),
            TASK_DROPLET_HEAL => {
                let src = s.max_hp(ci) * 0.16;
                let bonus = s.stat(ci, st::HEAL);
                s.heal(ci, ci, src, bonus);
            }
            TASK_BURST => burst_tick(s, ci, arg as usize),
            TASK_THORN => thorn(s, ci),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_DROPLETS != 0 {
            skill_droplets(s, ci);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD as i32, true);
            s.queue_particle(g::PARTICLE_COUNT as f64, Element::Hydro, s.particle_delay[ci]);
        }
        if cbs & CB_C6 != 0 {
            c6_cb(s, ci);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::HYDROP] += s.st::<State>(ci).a4;
        true
    }

    /// C2: CRIT DMG +14% per A1 stack on the judgment and its C6 hits.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.abil.contains(g::CHARGE_JUDGEMENT_NAME) {
            ev.stats[st::CD] += 0.14 * count_a1(s, ci) as f64;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Reaction(r, ev) => {
                if ev.on_player {
                    return;
                }
                let k = match r {
                    Reaction::Bloom => 0,
                    Reaction::CrystallizeHydro => 2,
                    Reaction::ElectroCharged => 3,
                    Reaction::Frozen => 6,
                    Reaction::SwirlHydro => 7,
                    Reaction::Vaporize => 8,
                    _ => return,
                };
                s.add_status(ci, A1_KEYS[k], 30 * 60, true);
            }
            Event::Drain { on, amount, .. } => {
                if amount > 0.0 && on == ci {
                    update_a4(s, ci);
                }
            }
            Event::Heal { on, amount, overheal, .. } => {
                // A4, then C4, in gcsim's order of subscription
                let asc = s.chars[ci].ascension;
                if asc >= 4 && amount > 0.0 && (amount - overheal).abs() > 1e-9 && on == ci {
                    update_a4(s, ci);
                }
                if s.chars[ci].cons >= 4 && s.player.active as usize == ci && on == ci && !s.mod_active(ci, C4_ICD) {
                    s.add_status(ci, C4_ICD, 4 * 60, true);
                    let (x, y) = random_point(s, 0.0, 8.0, 1.3, 3.0);
                    add_droplet(s, ci, x, y);
                }
            }
            Event::Swap { next, .. } => {
                if next == ci {
                    s.st_mut::<State>(ci).last_swap = s.f;
                }
            }
            Event::StateChange(next) => {
                if s.player.active as usize != ci {
                    return;
                }
                match next {
                    AnimState::Dash | AnimState::Jump => s.st_mut::<State>(ci).early_cancelled = false,
                    AnimState::Walk if s.st::<State>(ci).early_cancelled => early_cancel_error(s, "Walk"),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// gcsim's error from an action after a judgment cut short by `ticks`.
fn early_cancel_error(s: &mut Sim, what: &str) {
    if s.error.is_none() {
        s.error = Some(format!("neuvillette: Cannot early cancel Charged Attack: Equitable Judgement with {what}"));
    }
    s.done = true;
}

fn a4_value(ratio: f64) -> f64 {
    ((ratio - 0.3) * 0.6).clamp(0.0, 0.3)
}

fn update_a4(s: &mut Sim, ci: usize) {
    let r = s.chars[ci].hp_ratio;
    s.st_mut::<State>(ci).a4 = a4_value(r);
}

fn count_a1(s: &Sim, ci: usize) -> usize {
    if s.chars[ci].ascension < 1 {
        return 0;
    }
    let mut n = s.st::<State>(ci).a1_base;
    for k in A1_KEYS {
        if s.mod_active(ci, k) {
            n += 1;
        }
        if n == 3 {
            break;
        }
    }
    n
}

fn tick_delay(tick: i32) -> i32 {
    match tick {
        1 => 6,
        2 => 22,
        _ => 25,
    }
}

/// The judgment's frames, when it is still the action under way.
fn sync_frames(s: &mut Sim, ci: usize) {
    let st = s.st::<State>(ci);
    let arg = (st.frames_base + st.tick_anim).min(i16::MAX as i32) as i16;
    if let Some(a) = s.player.anim.as_mut() {
        if a.char as usize == ci && matches!(a.frames, Frames::Char { id: FR_JUDGE, .. }) {
            a.frames = Frames::Char { id: FR_JUDGE, arg };
        }
    }
}

// ------------------------------------------------------------ droplets

/// Whether a gadget is one of his droplets (`.gadgets.sourcewaterdroplet`).
pub fn is_droplet(s: &Sim, gd: &Gadget) -> bool {
    s.chars[gd.owner as usize].kind == crate::chars::CharKind::Neuvillette && gd.id == DROPLET
}

/// gcsim's CalcRandomPointFromCenter around (cx, cy).
fn random_point(s: &mut Sim, cx: f64, cy: f64, min: f64, max: f64) -> (f64, f64) {
    loop {
        let x = -1.0 + s.rng.float64() * 2.0;
        let y = -1.0 + s.rng.float64() * 2.0;
        if x * x + y * y <= 1.0 {
            let r = (min * min + s.rng.float64() * (max * max - min * min)).sqrt();
            let (x, y) = if x == 0.0 && y == 0.0 { (0.0, 1.0) } else { (x, y) };
            let factor = r / (x * x + y * y).sqrt();
            return (x * factor + cx, y * factor + cy);
        }
    }
}

/// sourcewaterdroplet.New: a thirteenth removes the oldest (the first of
/// the earliest frame).
fn add_droplet(s: &mut Sim, ci: usize, x: f64, y: f64) {
    let mut count = 0;
    let mut oldest: Option<usize> = None;
    for (k, gd) in s.gadgets.iter().enumerate() {
        if gd.owner as usize == ci && gd.id == DROPLET {
            count += 1;
            if oldest.is_none_or(|o| gd.src < s.gadgets[o].src) {
                oldest = Some(k);
            }
        }
    }
    if count == DROPLET_LIMIT {
        s.gadgets.remove(oldest.unwrap());
    }
    s.add_gadget(ci, DROPLET, DROPLET_DURATION, x, y, 1.0);
}

/// getSourcewaterDroplets: centers in the 8 by 18 box from 7 behind the
/// player, or the 80 degree fan of radius 14 ahead; with c6, within 15.
fn in_area(x: f64, y: f64, c6: bool) -> bool {
    if c6 {
        return x * x + y * y <= 15.0 * 15.0;
    }
    // the box's center is (0, 2)
    let ly = y - 2.0;
    if (-4.0..=4.0).contains(&x) && (-9.0..=9.0).contains(&ly) {
        return true;
    }
    if x * x + y * y > 14.0 * 14.0 {
        return false;
    }
    let dot = if x == 0.0 && y == 0.0 { 1.0 } else { y / (x * x + y * y).sqrt() };
    let angle = dot.clamp(-1.0, 1.0).acos() * 180.0 / std::f64::consts::PI;
    (-40.0..=40.0).contains(&angle)
}

/// The indices of the droplets in range, in the gadget list's order, and
/// how many.
fn droplets(s: &Sim, ci: usize, c6: bool) -> ([usize; DROPLET_LIMIT], usize) {
    let mut out = [0; DROPLET_LIMIT];
    let mut n = 0;
    for (k, gd) in s.gadgets.iter().enumerate() {
        if gd.owner as usize == ci && gd.id == DROPLET && in_area(gd.x, gd.y, c6) {
            out[n] = k;
            n += 1;
        }
    }
    (out, n)
}

/// consumeDroplet for the droplets at these indices: each is removed and
/// heals him 5 frames later.
fn consume(s: &mut Sim, ci: usize, idx: &mut [usize]) {
    idx.sort_unstable();
    for &k in idx.iter().rev() {
        s.gadgets.remove(k);
    }
    for _ in 0..idx.len() {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_DROPLET_HEAL, arg: 0 }, 5);
    }
}

/// legalEvalFindDroplets: up to three of the droplets in range, in the
/// order of Go's rand.Perm.
fn legal_eval_find(s: &mut Sim, ci: usize) -> usize {
    let (found, n) = droplets(s, ci, false);
    let mut perm = [0usize; DROPLET_LIMIT];
    for i in 0..n {
        let j = s.rng.intn(i as i32 + 1) as usize;
        perm[i] = perm[j];
        perm[j] = i;
    }
    let orbs = n.min(3);
    let mut picked = [0usize; 3];
    for i in 0..orbs {
        picked[i] = found[perm[i]];
    }
    consume(s, ci, &mut picked[..orbs]);
    orbs
}

/// The skill's three droplets, nearer the player the nearer the enemy.
fn skill_droplets(s: &mut Sim, ci: usize) {
    let d2 = s.target_dist * s.target_dist;
    let i = if d2 <= 5.0 * 5.0 {
        0
    } else if d2 <= 10.0 * 10.0 {
        1
    } else {
        2
    };
    for j in 0..3 {
        let off = g::SKILL_DROPLET_OFFSETS[i][j];
        let rr = g::SKILL_DROPLET_RANDOM_RANGES[i][j];
        let (x, y) = random_point(s, off[0], off[1], rr[0], rr[1]);
        add_droplet(s, ci, x, y);
    }
}

// ------------------------------------------------------------ charged attacks

fn short_charge(s: &mut Sim, ci: usize, windup: i32) {
    legal_eval_find(s, ci);
    // without the stamina he floats back down
    let r = (1.0 + s.stam_percent_mod(Action::Charge)).max(0.0);
    if s.player.stam > 50.0 * r {
        s.use_stam(50.0 * r);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.pattern = s.box_on_player(0.0, 0.0, 3.0, 8.0);
        if ai.pattern.hits_circle(0.0, s.target_dist, s.target_radius) != Some(true) {
            s.unsupported("neuvillette short charge missing the enemy");
            return;
        }
        let h = g::SHORT_CHARGE_HITMARK + windup;
        s.queue_attack(ai, 0, h, h);
    }
}

fn legal_eval(s: &mut Sim, ci: usize) {
    let orbs = legal_eval_find(s, ci);
    let left = g::INITIAL_LEGAL_EVAL_DUR - g::DROPLET_LEGAL_EVAL_REDUCTION[orbs];
    let lvl = s.talent(ci, 0);
    let flat = g::CHARGE_JUDGEMENT[lvl] * s.max_hp(ci);
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    st.charge_flat = flat;
    st.judge_start = f + left;
    st.judge_dur = 173;
    st.frames_base = st.windup + 3 + left;
    st.tick = 1;
    st.max_tick = st.ticks;
    let src = st.judge_start;
    sync_frames(s, ci);
    if s.chars[ci].cons >= 6 {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6_CHECK, arg: src }, left);
    }
    s.player.swap_cd = i16::MAX as i32;
    // hitlag does not move the ticks
    s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg: src }, left + tick_delay(1));
    // he drains on frames 40, 70, 100, 130 and 160
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_HP, arg: src }, left + 40);
}

/// One wave of the judgment, its damage from his max HP now.
fn judgment_wave(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 0);
    let mut flat = s.st::<State>(ci).charge_flat;
    if s.chars[ci].ascension >= 1 {
        flat = g::CHARGE_JUDGEMENT[lvl] * s.max_hp(ci) * g::A1_MULTIPLIERS[count_a1(s, ci)];
        s.st_mut::<State>(ci).charge_flat = flat;
    }
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
    ai.abil = g::CHARGE_JUDGEMENT_NAME;
    ai.flat_dmg = flat;
    ai.pattern = s.box_on_player(0.0, 0.0, 3.5, 15.0);
    let cbs = if s.chars[ci].cons >= 6 { CB_C6 } else { 0 };
    if ai.pattern.hits_circle(0.0, s.target_dist, s.target_radius) == Some(true) {
        s.queue_attack(ai, cbs, 0, 0);
    } else {
        s.queue_gadget_attack(ai, 0);
    }
}

/// chargeJudgementTick: `last` is the tick queued for the end of the
/// judgment, which C6 may have extended since.
fn judgment_tick(s: &mut Sim, ci: usize, src: i32, last: bool) {
    let st = s.st::<State>(ci);
    if st.judge_start != src || s.f > st.judge_start + st.judge_dur {
        return;
    }
    if last {
        if s.f == st.judge_start + st.judge_dur {
            judgment_wave(s, ci);
            s.player.swap_cd = g::END_LAG[Action::Swap as usize];
        } else {
            let st = s.st_mut::<State>(ci);
            st.tick_anim = st.tick_anim_c6;
            sync_frames(s, ci);
        }
        return;
    }
    let (tick, max_tick) = (st.tick, st.max_tick);
    if tick == max_tick {
        judgment_wave(s, ci);
        s.st_mut::<State>(ci).early_cancelled = true;
        return;
    }
    judgment_wave(s, ci);
    if max_tick == -1 || tick < max_tick {
        let delay = tick_delay(tick + 1);
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        let next_len = f - st.judge_start + delay;
        st.tick = tick + 1;
        s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg: src }, delay);
        let st = s.st_mut::<State>(ci);
        if next_len > st.judge_dur {
            let d = st.judge_dur - st.tick_anim;
            st.tick_anim = st.judge_dur;
            st.tick_anim_c6 = next_len;
            s.add_task(Task::Char { c: ci as u8, id: TASK_TICK_LAST, arg: src }, d);
        } else {
            st.tick_anim = next_len;
        }
        sync_frames(s, ci);
    }
}

/// 8% of his max HP every 0.5 s while above half HP.
fn consume_hp(s: &mut Sim, ci: usize, src: i32) {
    let st = s.st::<State>(ci);
    if st.judge_start != src || s.f > st.judge_start + st.judge_dur {
        return;
    }
    if s.chars[ci].hp_ratio > 0.5 {
        let amt = 0.08 * s.max_hp(ci);
        s.drain(ci, amt);
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_HP, arg: src }, 30);
}

/// C6: in the judgment's last second, a droplet within 15 lengthens it by
/// 1 s, one every 0.3 s.
fn c6_droplet_check(s: &mut Sim, ci: usize, src: i32) {
    let st = s.st::<State>(ci);
    if st.judge_start != src || s.f > st.judge_start + st.tick_anim {
        return;
    }
    if st.judge_start + st.judge_dur - s.f <= 60 {
        let (found, n) = droplets(s, ci, true);
        if n > 0 {
            consume(s, ci, &mut [found[0]]);
            s.st_mut::<State>(ci).judge_dur += 60;
        }
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6_CHECK, arg: src }, 18);
}

/// C6: a judgment hit fires two currents, once every 2 s.
fn c6_cb(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, C6_ICD) {
        return;
    }
    s.add_status(ci, C6_ICD, 2 * 60, true);
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NEUVILLETTE_C6, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
    ai.abil = C6_NAME;
    ai.flat_dmg = 0.1 * s.max_hp(ci) * g::A1_MULTIPLIERS[count_a1(s, ci)];
    ai.pattern = s.circle_on_target(0.5);
    s.queue_attack(ai, 0, 29, 29);
    s.queue_attack(ai, 0, 29, 29);
}

// ------------------------------------------------------------ skill and burst

fn thorn(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, ALIGNED_ICD) {
        return;
    }
    s.add_status(ci, ALIGNED_ICD, g::SKILL_ALIGNED_ICD, true);
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 0.0, g::THORN[lvl]);
    ai.abil = "Spiritbreath Thorn (neuvillette)";
    ai.hitlag_factor = 0.01;
    ai.def_halt = true;
    ai.pattern = s.circle_on_target(4.5);
    let d = g::SKILL_HITMARKS[1] - g::SKILL_HITMARKS[0];
    s.queue_attack(ai, 0, d, d);
}

/// A burst hit: its droplets (three, two, one), then the initial wave on
/// the player or a waterfall near the enemy, or ahead when it is beyond 10.
fn burst_tick(s: &mut Sim, ci: usize, i: usize) {
    let count = 3 - i;
    for j in 0..count {
        let off = g::DROPLET_POS_OFFSETS[i][j];
        let rr = g::DROPLET_RANDOM_RANGES[i];
        let (x, y) = random_point(s, off[0], off[1], rr[0], rr[1]);
        add_droplet(s, ci, x, y);
    }
    let flat = s.st::<State>(ci).burst_flat;
    let (flat, abil) = if i == 0 {
        (flat[0], "O Tides, I Have Returned: Skill DMG")
    } else {
        (flat[1], "O Tides, I Have Returned: Waterfall DMG")
    };
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
    ai.abil = abil;
    ai.flat_dmg = flat;
    ai.pattern = if i == 0 {
        s.circle_on_player(0.0, 1.0, 8.0)
    } else {
        let off = g::DEFAULT_BURST_ATK_POS_OFFSETS[i - 1];
        let (mut x, mut y) = (off[0], off[1]);
        let d = s.target_dist;
        if d * d <= 10.0 * 10.0 {
            // the enemy faces (0, 1), gcsim's default
            let (px, py) = random_point(s, g::BURST_TICK_TARGET_X_OFFSETS[i - 1], d, 0.0, 1.5);
            x = px;
            y = py;
        }
        Pattern::Circle { x, y, r: 5.0 }
    };
    if ai.pattern.hits_circle(0.0, s.target_dist, s.target_radius) == Some(true) {
        s.queue_attack(ai, 0, 0, 0);
    } else {
        s.queue_gadget_attack(ai, 0);
    }
}
