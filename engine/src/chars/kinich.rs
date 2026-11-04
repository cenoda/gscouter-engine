//! Kinich, from gcsim's internal/characters/kinich: normal and charged
//! attacks, Canopy Hunter: Riding High (the Nightsoul's Blessing it enters
//! after its hold, its points from time, Loop Shots and the blind spot,
//! the blind spot's side drawn as it appears), the Loop Shots that replace
//! his normal attacks in the blessing (with their own counter and his
//! angular position), Scalespiker Cannon at 20 points (its snapshot at
//! release, its travel), Hail to the Almighty Dragonlord (its hit, Ajaw's
//! breaths every 145 or 150 frames drawn, the blessing's extension), A1's
//! Desolation and points from burning and burgeon, A4's Hunter's
//! Experience, the end of the blessing on a swap and every constellation.
//!
//! As gcsim writes him: the skill's `c6-travel` param is accepted and not
//! read (gcsim reads `c6_travel`, which its validation rejects), so C6's
//! cannon travels 50 frames; A4's stacks are kept as a tag whatever its
//! status; C4 makes his burst's DMG +70% for good once it first triggers.

use crate::content::{Character, Event};
use crate::gcsl::Num;
use crate::gcsimdata::chars::kinich as g;
use crate::nightsoul::NIGHTSOUL_BLESSING;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    nightsoul_src: i32,
    ajaw_src: i32,
    /// characterAngularPosition and blindSpotAngularPosition, in [0, 360),
    /// the blind spot -1 when there is none.
    angle: f64,
    blind_spot: f64,
    particles_generated: bool,
    c2_aoe_increased: bool,
    /// A4's Hunter's Experience, gcsim's tag.
    a4_stacks: i32,
    /// The cannon on its way: its snapshot (taken at release), radius,
    /// blind spot delay and C6's travel.
    cannon: AttackEvent,
    cannon_radius: f64,
    cannon_travel: i32,
    cannon_blind_delay: i32,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const DESOLATION: ModKey = ModKey::new(g::DESOLATION_KEY);
const A1_ICD: ModKey = ModKey::new(g::A1_ICD);
const A4_STACKS: ModKey = ModKey::new(g::A4_STACK_KEY);
const C1: ModKey = ModKey::new("kinich-c1");
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const C4_DMGP: ModKey = ModKey::new("kinich-c4-dmgp");
const C2_SHRED: ModKey = ModKey::new("kinich-c2");

const CB_PARTICLE: u16 = 1 << 0;
const CB_A1: u16 = 1 << 1;
const CB_C2: u16 = 1 << 2;

/// The skill's start: the blessing, after the hold.
const TASK_SKILL_START: u8 = 0;
/// cancelNightsoul, the blessing's timer callback.
const TASK_CANCEL: u8 = 1;
/// timePassGenerateNSPoints, with the blessing's src.
const TASK_TIME_POINTS: u8 = 2;
/// createBlindSpot.
const TASK_BLIND_SPOT: u8 = 3;
/// The cannon's release: the points consumed and the snapshot.
const TASK_CANNON_RELEASE: u8 = 4;
/// The cannon's hit, after its travel.
const TASK_CANNON_HIT: u8 = 5;
/// c4, on the core queue at a Loop Shot's first hitmark.
const TASK_C4: u8 = 6;
/// The blind spot entered during a Loop Shot.
const TASK_BLIND_ENTER: u8 = 7;
/// The angular position after a Loop Shot, with its direction (0 for -1).
const TASK_ANGLE: u8 = 8;
/// Ajaw's breath, with src * 8 + step.
const TASK_LASER: u8 = 9;

/// The skill's frames, after its hold (arg).
const FR_SKILL: u8 = 0;
/// The cannon's frames, after its release (arg).
const FR_CANNON: u8 = 1;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];
const LOOP_NAMES: [&str; 2] = ["Loop Shot 0", "Loop Shot 1"];
const CHARGE_NAMES: [&str; 3] = ["Charge 0", "Charge 1", "Charge 2"];

impl Character for Char {
    const KEY: &'static str = "kinich";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        let ai = AttackInfo::new(0, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, 0.0);
        State {
            nightsoul_src: 0,
            ajaw_src: 0,
            angle: 0.0,
            blind_spot: -1.0,
            particles_generated: false,
            c2_aoe_increased: false,
            a4_stacks: 0,
            cannon: AttackEvent::new(ai, 0),
            cannon_radius: 3.0,
            cannon_travel: 0,
            cannon_blind_delay: 0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        s.chars[ci].ns.max_points = 20.0;
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::EnemyDamage, o);
        }
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::NightsoulBurst, o);
        }
        if s.chars[ci].cons >= 1 {
            s.chars[ci].mods.push(Mod::owned(C1, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        // onExitField
        s.subscribe(EventKind::Swap, o);
    }

    /// gcsim validates direction on the normal attack and c6-travel, hold
    /// and travel on the skill, and reads no params of the others; he has
    /// no plunges.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let only = |known: &[&str]| p.0.iter().find(|(k, _)| !known.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"));
        match a {
            Action::Attack => only(&["direction"]),
            Action::Skill => only(&["c6-travel", "hold", "travel"]),
            Action::Charge | Action::Burst => None,
            _ => Some(name.into()),
        }
    }

    /// No charged attack or plunge in the blessing (gcsim's
    /// ErrInvalidChargeAction).
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if s.ns_has_blessing(ci) && matches!(a, Action::Charge | Action::LowPlunge | Action::HighPlunge) {
            return Some("need to use attack right before charge".into());
        }
        crate::content::template_invalid(s, a)
    }

    /// The skill in the blessing, whatever its cooldown.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && s.ns_has_blessing(ci)).then_some(None)
    }

    /// In the blessing every N0 delay is the Loop Shot's dash cancel.
    fn start_delay(s: &Sim, ci: usize, _: usize) -> Option<i32> {
        s.ns_has_blessing(ci).then_some(g::SKILL_ATTACK_FRAMES[0][Action::Dash as usize])
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "nightsoul" => Some(s.ns_condition(ci, fields)),
            "blind_spot" => {
                let st = s.st::<State>(ci);
                if st.blind_spot == -1.0 {
                    return Some(Ok(Num::Int(0)));
                }
                let diff = normalize180(st.blind_spot - st.angle);
                Some(Ok(Num::Float(diff / diff.abs())))
            }
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.ns_has_blessing(ci) {
            return loop_shot(s, ci, p);
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.blunt = true;
        ai.poise = g::ATTACK_POISE_DMG[n];
        ai.hitlag_factor = g::ATTACK_HITLAG_FACTOR[n];
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = g::ATTACK_DEF_HALT[n];
        let hb = g::ATTACK_HITBOXES[n];
        ai.pattern = s.box_on_player(0.0, g::ATTACK_OFFSETS[n], hb[0], hb[1]);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        advance_normal(s, ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        // NewBoxHitOnTarget(Player().Pos(), {Y: 0.45}, 3, 5): a point, so
        // gcsim's default direction
        let (x, y) = offset_point(s.player_pos, (0.0, 0.45), DEFAULT_DIR);
        let (dx, dy) = DEFAULT_DIR;
        for (i, &h) in g::CHARGE_HITMARKS.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[lvl]);
            ai.abil = CHARGE_NAMES[i];
            ai.blunt = true;
            ai.poise = 40.0;
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = 0.01 * 60.0;
            ai.pattern = Pattern::Box { x, y, w: 3.0, h: 5.0, dx, dy };
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.ns_has_blessing(ci) {
            if s.ns_points(ci) == s.chars[ci].ns.max_points {
                return cannon(s, ci, p);
            }
            let msg = format!("kinich: Cannot use Scalespiker Cannon with {} Nightsoul points, should be {}", s.ns_points(ci), s.chars[ci].ns.max_points);
            fail(s, &msg);
            let t = &g::SKILL_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[0], AnimState::Skill);
        }
        let mut hold = p.get("hold").unwrap_or(0).clamp(0, 301) as i32;
        if hold > 0 {
            hold -= 1;
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_START, arg: 0 }, g::SKILL_START + hold);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Char { id: FR_SKILL, arg: hold as i16 }, t[0] + hold, t[Action::Burst as usize] + hold, AnimState::Skill)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_SKILL => g::SKILL_FRAMES[next as usize] + arg as i32,
            FR_CANNON => arg as i32 + g::SCALESPIKER_FRAMES[next as usize],
            _ => unreachable!(),
        }
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let f = s.f;
        s.st_mut::<State>(ci).ajaw_src = f;
        s.add_status(ci, BURST, g::AJAW_DURATION, false);
        if s.ns_has_blessing(ci) {
            let d = s.ns_duration(ci);
            if d > 0 {
                s.ns_set_exit_timer(ci, d + 102, TASK_CANCEL);
            }
        }
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::BURST[s.talent(ci, 2)]);
        ai.abil = "Hail to the Almighty Dragonlord (Skill DMG)";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.pattern = s.circle_on_target(4.0);
        s.queue_attack(ai, 0, g::BURST_HIT_MARK, g::BURST_HIT_MARK);
        s.add_task(Task::Char { c: ci as u8, id: TASK_LASER, arg: f * 8 + 1 }, g::AJAW_FIRST_HITMARK);
        s.consume_energy(ci, g::CONSUME_ENERGY_DELAY);
        s.set_cd_with_delay(ci, Action::Burst, 18 * 60, g::CD_START);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL_START => {
                let f = s.f;
                s.st_mut::<State>(ci).nightsoul_src = f;
                s.ns_enter_timed(ci, 0.0, 10 * 60 + 10, TASK_CANCEL);
                let st = s.st_mut::<State>(ci);
                st.c2_aoe_increased = false;
                st.particles_generated = false;
                s.set_cd(ci, Action::Skill, g::SKILL_CD);
                s.add_task(Task::Char { c: ci as u8, id: TASK_TIME_POINTS, arg: f }, g::GENERATE_NS_POINT_DELAY);
                s.add_task(Task::Char { c: ci as u8, id: TASK_BLIND_SPOT, arg: 0 }, g::BLIND_SPOT_APPEARANCE_DELAYS[1] - g::SKILL_START);
            }
            TASK_CANCEL => cancel_nightsoul(s, ci),
            TASK_TIME_POINTS => {
                if s.st::<State>(ci).nightsoul_src != arg {
                    return;
                }
                s.ns_generate(ci, 1.0);
                s.add_task(Task::Char { c: ci as u8, id: TASK_TIME_POINTS, arg }, g::GENERATE_NS_POINT_DELAY);
            }
            TASK_BLIND_SPOT => {
                let side = (s.rng.intn(2) * 2 - 1) as f64;
                let st = s.st_mut::<State>(ci);
                st.blind_spot = normalize360(st.angle + side * 90.0);
            }
            TASK_CANNON_RELEASE => {
                let pts = s.ns_points(ci);
                s.ns_consume(ci, pts);
                let ai = s.st::<State>(ci).cannon.info;
                let snap = s.snapshot_event(ai, 0);
                s.st_mut::<State>(ci).cannon = snap;
                let d = s.st::<State>(ci).cannon_blind_delay;
                s.add_task(Task::Char { c: ci as u8, id: TASK_BLIND_SPOT, arg: 0 }, d);
                s.add_task(Task::Char { c: ci as u8, id: TASK_CANNON_HIT, arg: 0 }, arg);
            }
            TASK_CANNON_HIT => {
                if s.chars[ci].cons >= 2 && !s.st::<State>(ci).c2_aoe_increased {
                    let st = s.st_mut::<State>(ci);
                    st.c2_aoe_increased = true;
                    st.cannon_radius = 5.0;
                    st.cannon.stats[st::DMGP] += 1.0;
                }
                let r = s.st::<State>(ci).cannon_radius;
                let mut ev = s.st::<State>(ci).cannon;
                ev.info.pattern = s.circle_on_target(r);
                s.queue_attack_with_snap(ev, CB_PARTICLE | CB_A1 | CB_C2, 0);
                c4(s, ci);
                // c6: a second cannon on another enemy within its radius, or
                // the same one
                if s.chars[ci].cons >= 6 {
                    let mut ev = s.st::<State>(ci).cannon;
                    ev.info.abil = g::C6_ABIL;
                    ev.info.mult = 7.0;
                    ev.info.pattern = s.circle_on_target(r);
                    let travel = s.st::<State>(ci).cannon_travel;
                    s.queue_attack_with_snap(ev, CB_PARTICLE | CB_A1 | CB_C2, travel);
                }
            }
            TASK_C4 => c4(s, ci),
            TASK_BLIND_ENTER => {
                s.ns_generate(ci, 4.0);
                s.st_mut::<State>(ci).blind_spot = -1.0;
            }
            TASK_ANGLE => {
                let dir = if arg == 0 { -1.0 } else { 1.0 };
                let n = s.chars[ci].normal_s_counter as usize;
                let st = s.st_mut::<State>(ci);
                st.angle = normalize360(st.angle + dir * g::SKILL_ATTACK_ANGULAR_TRAVEL[n]);
            }
            TASK_LASER => {
                let (src, step) = (arg >> 3, arg & 7);
                if s.st::<State>(ci).ajaw_src != src || !s.mod_active(ci, BURST) {
                    return;
                }
                if step == 7 {
                    s.delete_mod(ci, BURST);
                    return;
                }
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::DRAGON_BREATH[s.talent(ci, 2)]);
                ai.abil = "Hail to the Almighty Dragonlord (Dragon Breath DMG)";
                ai.add_tags = add_tag::NIGHTSOUL;
                ai.hitlag_factor = 0.05;
                ai.deployable = true;
                ai.pattern = s.box_on_enemy(0.0, 0.0, 1.0, 10.0);
                s.queue_attack(ai, 0, 0, 0);
                let next = g::AJAW_HITMARKS[s.rng.intn(2) as usize];
                s.add_task(Task::Char { c: ci as u8, id: TASK_LASER, arg: src << 3 | (step + 1) }, next);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.st::<State>(ci).particles_generated {
            s.st_mut::<State>(ci).particles_generated = true;
            s.queue_particle(5.0, Element::Dendro, s.particle_delay[ci]);
        }
        // a1CB: Desolation on the enemy while in the blessing
        if cbs & CB_A1 != 0 && s.chars[ci].ascension >= 1 && s.mod_active(ci, NIGHTSOUL_BLESSING) {
            s.enemy_add_status(DESOLATION, 12 * 60, true);
        }
        // c2ResShredCB
        if cbs & CB_C2 != 0 && s.chars[ci].cons >= 2 && ev.damage != 0.0 {
            s.enemy_add_resist(C2_SHRED, Element::Dendro, -0.3, 6 * 60);
        }
    }

    /// C1: the cannon's CRIT DMG +100%; C4: the burst's DMG +70%.
    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C1 {
            if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) && ev.info.has_tag(add_tag::KINICH_CANNON) {
                ev.stats[st::CD] += 1.0;
            }
        } else if m.key == C4_DMGP && ev.info.tag == AttackTag::ElementalBurst {
            ev.stats[st::DMGP] += 0.7;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // a1: burning or burgeon damage on a desolated enemy, 7 points
            // once every 0.8 s
            Event::EnemyDamage(ev) => {
                if !matches!(ev.info.tag, AttackTag::BurningDamage | AttackTag::Burgeon) || !s.enemy_status_active(DESOLATION) || s.mod_active(ci, A1_ICD) {
                    return;
                }
                s.ns_generate(ci, 7.0);
                s.add_status(ci, A1_ICD, 48, false);
            }
            // a4: Hunter's Experience, 2 stacks
            Event::NightsoulBurst(_) => {
                let stacks = (s.st::<State>(ci).a4_stacks + 1).min(2);
                s.add_status(ci, A4_STACKS, 15 * 60, true);
                s.st_mut::<State>(ci).a4_stacks = stacks;
            }
            // onExitField: any swap ends the blessing
            Event::Swap { .. } => {
                if s.ns_has_blessing(ci) {
                    cancel_nightsoul(s, ci);
                }
            }
            _ => {}
        }
    }
}

fn fail(s: &mut Sim, what: &str) {
    if s.error.is_none() {
        s.error = Some(what.to_string());
    }
}

/// His AdvanceNormalIndex: the Loop Shots' counter in the blessing.
fn advance_normal(s: &mut Sim, ci: usize) {
    if s.ns_has_blessing(ci) {
        let c = &mut s.chars[ci];
        c.normal_s_counter += 1;
        if c.normal_s_counter == g::SKILL_HIT_NUM {
            c.normal_s_counter = 0;
        }
        return;
    }
    s.advance_normal(ci);
}

/// skillAttack: a Loop Shot, its points, and the blind spot it may cross.
fn loop_shot(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
    let n = s.chars[ci].normal_s_counter as usize;
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_KINICH_LOOP_SHOT, icd::GROUP_KINICH_LOOP_SHOT, Element::Dendro, 25.0, g::LOOP_SHOT[s.talent(ci, 1)]);
    ai.abil = LOOP_NAMES[n];
    ai.add_tags = add_tag::NIGHTSOUL;
    ai.ignore_infusion = true;
    // loopShotGenerateNSPoints
    if s.ns_has_blessing(ci) {
        s.ns_generate(ci, 3.0);
    }
    // NewCircleHit(Player, PrimaryTarget, nil, 0.5)
    ai.pattern = s.circle_toward_target(0.0, 0.0, 0.5);
    let marks = g::SKILL_ATTACK_HITMARKS[n];
    s.queue_attack(ai, CB_A1 | CB_C2, marks[0], marks[0]);
    s.queue_attack(ai, CB_A1 | CB_C2, marks[1], marks[1]);
    s.add_task(Task::Char { c: ci as u8, id: TASK_C4, arg: 0 }, marks[0]);
    let t = &g::SKILL_ATTACK_FRAMES[n];
    let info = ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Normal);
    let direction = p.get("direction").unwrap_or(-1);
    if direction != 1 && direction != -1 {
        let f = s.f;
        fail(s, &format!("{f}, kinich: Wrong value of direction: {direction}, should be 1 or -1"));
        advance_normal(s, ci);
        return info;
    }
    if let Some(boundary) = next_move_in_blind_spot(s, ci, n, direction as f64) {
        let time = normalize180(boundary - s.st::<State>(ci).angle).abs() / g::ANGULAR_VELOCITY;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BLIND_ENTER, arg: 0 }, time as i32);
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ANGLE, arg: (direction == 1) as i32 }, t[Action::Dash as usize]);
    advance_normal(s, ci);
    info
}

/// NextMoveIsInBlindSpot: the blind spot's boundary the next Loop Shot
/// crosses, if any.
fn next_move_in_blind_spot(s: &Sim, ci: usize, n: usize, direction: f64) -> Option<f64> {
    let st = s.st::<State>(ci);
    if st.blind_spot == -1.0 {
        return None;
    }
    let mut lower = normalize360(st.angle);
    let mut upper = normalize360(st.angle + direction * g::SKILL_ATTACK_ANGULAR_TRAVEL[n]);
    let mut lower_blind = normalize360(st.blind_spot - g::BLIND_SPOT_BOUNDARY);
    let mut upper_blind = normalize360(st.blind_spot + g::BLIND_SPOT_BOUNDARY);
    if direction == -1.0 {
        std::mem::swap(&mut lower, &mut upper);
        std::mem::swap(&mut lower_blind, &mut upper_blind);
    }
    let in_sector = |a: f64| if lower < upper { a >= lower && a <= upper } else { a >= lower || a <= upper };
    if in_sector(lower_blind) {
        return Some(lower_blind);
    }
    if in_sector(upper_blind) {
        return Some(upper_blind);
    }
    None
}

/// ScalespikerCannon: released after its hold, its points consumed and its
/// snapshot taken a frame later, hitting after its travel.
fn cannon(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
    let mut hold = p.get("hold").unwrap_or(0).clamp(0, 181) as i32;
    let travel = p.get("travel").unwrap_or(g::SCALESPIKER_DEFAULT_TRAVEL as i64) as i32;
    let (mut release, mut blind_delay) = (g::SCALESPIKER_RELEASES[0], g::BLIND_SPOT_APPEARANCE_DELAYS[0]);
    if hold > 0 {
        hold -= 1;
        release = g::SCALESPIKER_RELEASES[1];
        blind_delay = g::BLIND_SPOT_APPEARANCE_DELAYS[1];
    }
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_KINICH_SCALESPIKER_CANNON, icd::GROUP_KINICH_SCALESPIKER_CANNON, Element::Dendro, 25.0, g::SCALESPIKER_CANNON[s.talent(ci, 1)]);
    ai.abil = "Scalespiker Cannon";
    ai.add_tags = add_tag::NIGHTSOUL | add_tag::KINICH_CANNON;
    ai.flat_dmg = a4_amount(s, ci);
    ai.pattern = s.circle_on_target(3.0);
    let st = s.st_mut::<State>(ci);
    st.cannon = AttackEvent::new(ai, 0);
    st.cannon_radius = 3.0;
    st.cannon_travel = 50;
    st.cannon_blind_delay = blind_delay;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CANNON_RELEASE, arg: travel }, release + hold + g::POINTS_CONSUMPTIONS_DELAY);
    let t = &g::SCALESPIKER_FRAMES;
    let r = release + hold;
    ActInfo::new(Frames::Char { id: FR_CANNON, arg: r as i16 }, r + t[0], r + t[Action::Attack as usize], AnimState::Skill)
}

/// a4Amount: 320% of ATK a stack of Hunter's Experience, the stacks then
/// cleared.
fn a4_amount(s: &mut Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 4 {
        return 0.0;
    }
    let stacks = s.st::<State>(ci).a4_stacks;
    s.st_mut::<State>(ci).a4_stacks = 0;
    s.delete_mod(ci, A4_STACKS);
    3.2 * stacks as f64 * s.total_atk(ci)
}

/// cancelNightsoul: the blessing and its points, the blind spot, and the
/// enemy's Desolation.
fn cancel_nightsoul(s: &mut Sim, ci: usize) {
    s.ns_clear(ci);
    s.ns_exit(ci);
    let st = s.st_mut::<State>(ci);
    st.nightsoul_src = -1;
    st.blind_spot = -1.0;
    s.enemy_delete_status(DESOLATION);
}

/// c4: 5 energy once every 2.8 s, and the burst's DMG bonus from then on.
fn c4(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 4 || s.mod_active(ci, C4_ICD) {
        return;
    }
    s.add_status(ci, C4_ICD, 168, true);
    s.add_energy(ci, 5.0);
    s.add_mod(ci, Mod::owned(C4_DMGP, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1), -1);
}

fn normalize180(mut a: f64) -> f64 {
    while a < -180.0 {
        a += 360.0;
    }
    while a >= 180.0 {
        a -= 360.0;
    }
    a
}

fn normalize360(mut a: f64) -> f64 {
    while a < 0.0 {
        a += 360.0;
    }
    while a >= 360.0 {
        a -= 360.0;
    }
    a
}
