//! Varesa, from gcsim's internal/characters/varesa, all of it: normal
//! attacks (Fiery Passion's in the Nightsoul blessing; a skill's follow-up
//! turns an attack into the charged attack), charged attacks (the
//! follow-up strike's shortcut to the high plunge), the high plunge after a
//! charged attack and the airborne (Xianyun's) plunges with their collision,
//! Nightsoul points and the 15 s blessing at 40, Apex Drive and Volcano
//! Kablam, the skill's two charges (free in the blessing), the burst, A1's
//! plunge flat damage from ATK read every 0.1 s, A4's stacks on Nightsoul
//! Bursts, and C1 to C6.

use crate::content::{Character, Event};
use crate::chars::plunge;
use crate::gcsimdata::chars::varesa as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    particle_generated: bool,
    free_skill: bool,
    exit_ns: bool,
    a1_src: i32,
    a1_atk: f64,
    /// A4's two stacks: the token of the Add that holds each (0 when free)
    /// and its frame.
    a4: [(u32, i32); 2],
    a4_token: u32,
    used_short_burst: bool,
    /// Volcano Kablam's flat damage, from the cast.
    kablam_flat: f64,
}

const SKILL_STATUS: ModKey = ModKey::new(g::SKILL_STATUS);
const APEX: ModKey = ModKey::new(g::APEX_STATE);
const A1: ModKey = ModKey::new(g::A1_STATUS);
const A4: ModKey = ModKey::new("varesa-a4");
const C4: ModKey = ModKey::new("varesa-c4");
const C4_STATUS: ModKey = ModKey::new(g::C4_STATUS);
const C6: ModKey = ModKey::new("varesa-c6");

const CB_PARTICLE: u16 = 1 << 0;
const CB_A1_CANCEL: u16 = 1 << 1;
const CB_C2: u16 = 1 << 2;
const CB_C4: u16 = 1 << 3;

const TASK_A1: u8 = 0;
const TASK_SKILL_POINTS: u8 = 1;
const TASK_KICK_POINTS: u8 = 2;
const TASK_KABLAM: u8 = 3;
const TASK_PLUNGE_POINTS: u8 = 4;
const TASK_CONSUME_ALL: u8 = 5;
/// The blessing's end (clearNightsoul), as its exit callback.
const TASK_CLEAR_NS: u8 = 6;
// arg: the stack's index * 2^24 + its token
const TASK_A4_END: u8 = 7;

// frames: arg = the normal counter * 16 + the windup
const FRAMES_NORMAL: u8 = 0;
const FRAMES_FIERY: u8 = 1;
// arg: 1 for the follow-up strike
const FRAMES_CHARGE: u8 = 2;
const FRAMES_FIERY_CHARGE: u8 = 3;
const FRAMES_XY_LOW: u8 = 4;
const FRAMES_XY_HIGH: u8 = 5;

const REMOVED_CLEAR_NS: u8 = 0;
const REMOVED_KABLAM: u8 = 1;

impl Character for Char {
    const KEY: &'static str = "varesa";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { particle_generated: false, free_skill: false, exit_ns: false, a1_src: 0, a1_atk: 0.0, a4: [(0, 0); 2], a4_token: 0, used_short_burst: false, kablam_flat: 0.0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // two skill charges, 40 Nightsoul points at most
        let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
        cd.available = 2;
        cd.max_extra = 1;
        s.chars[ci].ns.max_points = 40.0;
        // updateA1Bonus
        let f = s.f;
        s.st_mut::<State>(ci).a1_src = f;
        if s.chars[ci].ascension >= 1 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1, arg: f }, 6);
        }
        if s.chars[ci].ascension >= 4 {
            s.add_mod(ci, Mod::owned(A4, ModKind::Stat, o, st::ATKP, false, false, -1), -1);
            s.subscribe(EventKind::NightsoulBurst, o);
        }
        if s.chars[ci].cons >= 4 {
            s.add_mod(ci, Mod::owned(C4, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        }
        if s.chars[ci].cons >= 6 {
            s.add_mod(ci, Mod::owned(C6, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        match a {
            Action::HighPlunge if s.current_state() != AnimState::Charge && s.player.airborne != Airborne::Xianyun => Some("high_plunge can only be used after charge or while airborne".into()),
            Action::HighPlunge => None,
            Action::LowPlunge if s.player.airborne != Airborne::Xianyun => Some("low_plunge can only be used while airborne".into()),
            Action::LowPlunge => None,
            _ => crate::content::template_invalid(s, a),
        }
    }

    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        if a == Action::Skill && s.st::<State>(ci).free_skill {
            return Some(None);
        }
        if a == Action::Burst && s.mod_active(ci, APEX) {
            if !s.ignore_burst_energy && s.chars[ci].energy < g::KABLAM_COST as f64 {
                return Some(Some(Failure::InsufficientEnergy));
            }
            if s.chars[ci].cd[crate::sim::Char::cd_index(Action::Burst)].available <= 0 {
                return Some(Some(Failure::BurstCd));
            }
            return Some(None);
        }
        None
    }

    fn action_stam(s: &Sim, ci: usize, a: Action) -> Option<f64> {
        (a == Action::Charge && s.mod_active(ci, SKILL_STATUS)).then_some(0.0)
    }

    fn start_delay(s: &Sim, ci: usize, key: usize) -> Option<i32> {
        if !s.ns_has_blessing(ci) {
            return None;
        }
        // 1: AnimationYelanN0StartDelay, 2: AnimationXingqiuN0StartDelay
        match key {
            1 => Some(5),
            2 => Some(23),
            _ => None,
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        (fields[0] == "nightsoul").then(|| s.ns_condition(ci, fields))
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        clear_ns_cb(s, ci, AnimState::Normal);
        if s.mod_active(ci, SKILL_STATUS) {
            return charge(s, ci);
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let state = s.current_state();
        let short = s.st::<State>(ci).used_short_burst;
        let fiery = s.ns_has_blessing(ci);
        let (mut ai, hitbox, oy, hitmark, windup, id) = if fiery {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::FIERY_ATTACK[n][lvl]);
            ai.abil = FIERY_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = 0.03 * 60.0;
            let windup = match state {
                AnimState::Normal => 0,
                AnimState::Burst => if short { 0 } else { 4 },
                _ => 6,
            };
            (ai, g::FIERY_ATTACK_HITBOXES[n], g::FIERY_ATTACK_OFFSETS[n], g::FIERY_ATTACK_HITMARKS[n], windup, FRAMES_FIERY)
        } else {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::ATTACK[n][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = g::ATTACK_HITLAG_FACTOR[n];
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
            let windup = if n != 0 { 0 } else if state == AnimState::Burst && short { 4 } else { 6 };
            (ai, g::ATTACK_HITBOXES[n], g::ATTACK_OFFSETS[n], g::ATTACK_HITMARKS[n], windup, FRAMES_NORMAL)
        };
        ai.def_halt = true;
        ai.pattern = if hitbox.len() == 1 { s.circle_on_player(0.0, oy, hitbox[0]) } else { s.box_on_player(0.0, oy, hitbox[0], hitbox[1]) };
        s.queue_attack(ai, 0, hitmark - windup, hitmark - windup);
        let (t, cqa) = if fiery { (&g::FIERY_ATTACK_FRAMES[n], g::FIERY_ATTACK_CQA[n]) } else { (&g::ATTACK_FRAMES[n], g::ATTACK_CQA[n]) };
        let info = ActInfo::new(Frames::CharLive { id, arg: (n * 16 + windup as usize) as i16 }, t[0] - windup, cqa - windup, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        clear_ns_cb(s, ci, AnimState::Charge);
        charge(s, ci)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        clear_ns_cb(s, ci, AnimState::Skill);
        let lvl = s.talent(ci, 1);
        let fiery = s.ns_has_blessing(ci);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_VARESA_COMBAT_CYCLE, icd::GROUP_DEFAULT, Element::Electro, 25.0, if fiery { g::FIERY_RUSH[lvl] } else { g::RUSH[lvl] });
        ai.abil = if fiery { "Fiery Passion Rush" } else { "Rush" };
        ai.add_tags |= add_tag::NIGHTSOUL;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.pattern = s.box_on_target(0.0, 0.0, 0.1, 2.0);
        let hitmark = if fiery { g::FIERY_SKILL_HITMARK } else { g::SKILL_HITMARK };
        let mut cbs = CB_PARTICLE;
        if s.st::<State>(ci).free_skill {
            cbs = 0;
            s.st_mut::<State>(ci).free_skill = false;
        } else {
            s.st_mut::<State>(ci).particle_generated = false;
            s.set_cd(ci, Action::Skill, 9 * 60);
        }
        s.queue_attack(ai, cbs, hitmark, hitmark);
        a1(s, ci);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_POINTS, arg: 0 }, 5);
        s.add_status(ci, SKILL_STATUS, 5 * 60, true);
        if s.mod_active(ci, APEX) {
            s.delete_mod(ci, APEX);
        }
        let t = if fiery { &g::FIERY_SKILL_FRAMES } else { &g::SKILL_FRAMES };
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // c4Burst
        if s.chars[ci].cons >= 4 && !s.ns_has_blessing(ci) && !s.mod_active(ci, APEX) {
            s.add_status(ci, C4_STATUS, 15 * 60, true);
        }
        if s.mod_active(ci, APEX) {
            return kablam(s, ci);
        }
        let lvl = s.talent(ci, 2);
        let fiery = s.ns_has_blessing(ci);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, if fiery { g::FIERY_KICK[lvl] } else { g::KICK[lvl] });
        ai.abil = if fiery { "Fiery Passion Flying Kick" } else { "Flying Kick" };
        ai.add_tags |= add_tag::NIGHTSOUL;
        ai.poise = if fiery { 150.0 } else { 100.0 };
        ai.blunt = true;
        ai.hitlag_factor = 0.1;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_KICK_POINTS, arg: 0 }, 3);
        ai.pattern = s.circle_toward_target(0.0, 1.0, 7.5);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.consume_energy(ci, g::BURST_ENERGY_FRAME);
        s.set_cd(ci, Action::Burst, 18 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let after_charge = s.current_state() == AnimState::Charge;
        let fiery = s.ns_has_blessing(ci);
        let lvl = s.talent(ci, 0);
        let hitmark = match (after_charge, fiery) {
            (true, false) => g::HIGH_PLUNGE_HITMARK,
            (true, true) => g::FIERY_HIGH_PLUNGE_HITMARK,
            (false, false) => g::XIANYUN_HIGH_PLUNGE_HITMARK,
            (false, true) => g::XIANYUN_FIERY_HIGH_PLUNGE_HITMARK,
        };
        let mult = if fiery { g::FIERY_HIGH_PLUNGE[lvl] } else { g::HIGH_PLUNGE[lvl] };
        let abil = if fiery { "Fiery Passion High Plunge" } else { "High Plunge" };
        plunge_hit(s, ci, p, abil, mult, hitmark, g::HIGH_PLUNGE_RADIUS, fiery);
        let (frames, t): (Frames, &FrameTable) = match (after_charge, fiery) {
            (true, false) => (Frames::Table(&g::HIGH_PLUNGE_FRAMES), &g::HIGH_PLUNGE_FRAMES),
            (true, true) => (Frames::Table(&g::FIERY_HIGH_PLUNGE_FRAMES), &g::FIERY_HIGH_PLUNGE_FRAMES),
            (false, false) => (Frames::CharLive { id: FRAMES_XY_HIGH, arg: 0 }, &g::XIANYUN_HIGH_PLUNGE_FRAMES),
            (false, true) => (Frames::Table(&g::XIANYUN_FIERY_HIGH_PLUNGE_FRAMES), &g::XIANYUN_FIERY_HIGH_PLUNGE_FRAMES),
        };
        plunge_info(s, ci, frames, t)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let fiery = s.ns_has_blessing(ci);
        let lvl = s.talent(ci, 0);
        let hitmark = if fiery { g::XIANYUN_FIERY_LOW_PLUNGE_HITMARK } else { g::XIANYUN_LOW_PLUNGE_HITMARK };
        let mult = if fiery { g::FIERY_LOW_PLUNGE[lvl] } else { g::LOW_PLUNGE[lvl] };
        let abil = if fiery { "Fiery Passion Low Plunge" } else { "Low Plunge" };
        plunge_hit(s, ci, p, abil, mult, hitmark, g::LOW_PLUNGE_RADIUS, fiery);
        let (frames, t): (Frames, &FrameTable) = if fiery {
            (Frames::Table(&g::XIANYUN_FIERY_LOW_PLUNGE_FRAMES), &g::XIANYUN_FIERY_LOW_PLUNGE_FRAMES)
        } else {
            (Frames::CharLive { id: FRAMES_XY_LOW, arg: 0 }, &g::XIANYUN_LOW_PLUNGE_FRAMES)
        };
        plunge_info(s, ci, frames, t)
    }

    fn frames_live(s: &Sim, ci: usize, id: u8, arg: i16, next: Action) -> i32 {
        let nx = next as usize;
        match id {
            FRAMES_NORMAL => g::ATTACK_FRAMES[(arg / 16) as usize][nx] - (arg % 16) as i32,
            FRAMES_FIERY => g::FIERY_ATTACK_FRAMES[(arg / 16) as usize][nx] - (arg % 16) as i32,
            FRAMES_CHARGE | FRAMES_FIERY_CHARGE => {
                let (t, skip) = if id == FRAMES_CHARGE { (&g::CHARGE_FRAMES, g::FAST_CHARGED_FRAME) } else { (&g::FIERY_CHARGE_FRAMES, g::FAST_FIERY_CHARGED_FRAME) };
                if arg == 1 && next == Action::HighPlunge { t[nx] - skip } else { t[nx] }
            }
            FRAMES_XY_LOW | FRAMES_XY_HIGH => {
                let (t, walk) = if id == FRAMES_XY_LOW { (&g::XIANYUN_LOW_PLUNGE_FRAMES, g::XIANYUN_LOW_PLUNGE_NON_NS_WALK) } else { (&g::XIANYUN_HIGH_PLUNGE_FRAMES, g::XIANYUN_HIGH_PLUNGE_NON_NS_WALK) };
                if !s.ns_has_blessing(ci) && next == Action::Walk { walk } else { t[nx] }
            }
            _ => unreachable!(),
        }
    }

    fn on_removed(s: &mut Sim, ci: usize, id: u8, next: AnimState) {
        clear_ns_cb(s, ci, next);
        if id == REMOVED_KABLAM {
            s.st_mut::<State>(ci).used_short_burst = false;
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_A1 => {
                if s.st::<State>(ci).a1_src != arg {
                    return;
                }
                let atk = s.total_atk(ci);
                s.st_mut::<State>(ci).a1_atk = atk;
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1, arg }, 6);
            }
            TASK_SKILL_POINTS => s.ns_generate(ci, 20.0),
            TASK_KICK_POINTS => {
                let max = s.chars[ci].ns.max_points;
                s.ns_generate(ci, max);
                generate_plunge_ns(s, ci);
            }
            TASK_KABLAM => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_VARESA_COMBAT_CYCLE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::KABLAM[lvl]);
                ai.abil = g::KABLAM_ABIL;
                ai.add_tags |= add_tag::NIGHTSOUL;
                ai.poise = 75.0;
                ai.blunt = true;
                ai.hitlag_factor = 0.1;
                ai.flat_dmg = s.st::<State>(ci).kablam_flat;
                ai.pattern = s.circle_toward_target(0.0, 1.0, 7.5);
                s.queue_attack(ai, CB_A1_CANCEL, 0, 0);
                s.delete_mod(ci, APEX);
            }
            TASK_PLUNGE_POINTS => generate_plunge_ns(s, ci),
            TASK_CONSUME_ALL => {
                let p = s.ns_points(ci);
                s.ns_consume(ci, p);
            }
            TASK_CLEAR_NS => clear_ns(s, ci),
            TASK_A4_END => {
                let (i, token) = ((arg >> 24) as usize, (arg & 0xff_ffff) as u32);
                let st = s.st_mut::<State>(ci);
                if st.a4[i].0 == token {
                    st.a4[i] = (0, 0);
                }
            }
            _ => unreachable!(),
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        // A4's ATK
        out[st::ATKP] += 0.35 * s.st::<State>(ci).a4.iter().filter(|x| x.0 != 0).count() as f64;
        true
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C4 {
            if (ev.info.tag == AttackTag::ElementalBurst || ev.info.abil == g::KABLAM_ABIL) && (s.ns_has_blessing(ci) || s.mod_active(ci, APEX)) {
                ev.stats[st::DMGP] += 1.0;
            }
        } else if ev.info.tag == AttackTag::ElementalBurst || ev.info.tag == AttackTag::Plunge && ev.info.durability > 0.0 {
            ev.stats[st::CR] += 0.1;
            ev.stats[st::CD] += 1.0;
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.st::<State>(ci).particle_generated {
            s.st_mut::<State>(ci).particle_generated = true;
            let n = if s.rng.float64() < 0.5 { 3.0 } else { 2.0 };
            s.queue_particle(n, Element::Electro, s.particle_delay[ci]);
        }
        if cbs & CB_A1_CANCEL != 0 && s.chars[ci].ascension >= 1 {
            s.delete_mod(ci, A1);
        }
        if cbs & CB_C2 != 0 {
            s.add_energy(ci, 11.5);
        }
        if cbs & CB_C4 != 0 {
            s.delete_mod(ci, C4_STATUS);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        // A4: a stack on every Nightsoul Burst
        if let Event::NightsoulBurst(_) = *e {
            let f = s.f;
            let st = s.st_mut::<State>(ci);
            let mut idx = 0;
            for i in 0..st.a4.len() {
                if st.a4[i].0 == 0 {
                    idx = i;
                    break;
                }
                if st.a4[i].1 < st.a4[idx].1 {
                    idx = i;
                }
            }
            st.a4_token += 1;
            let token = st.a4_token;
            st.a4[idx] = (token, f);
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A4_END, arg: ((idx as i32) << 24) | token as i32 }, 12 * 60);
        }
    }
}

const FIERY_NAMES: [&str; 3] = ["Fiery Passion 0", "Fiery Passion 1", "Fiery Passion 2"];

/// The charged attack, plain or Fiery Passion, and the follow-up strike.
fn charge(s: &mut Sim, ci: usize) -> ActInfo {
    let lvl = s.talent(ci, 0);
    let fiery = s.ns_has_blessing(ci);
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_VARESA_COMBAT_CYCLE, icd::GROUP_DEFAULT, Element::Electro, 25.0, if fiery { g::FIERY_CHARGED[lvl] } else { g::CHARGED[lvl] });
    ai.add_tags |= add_tag::NIGHTSOUL;
    ai.poise = if fiery { 120.0 } else { 80.0 };
    ai.blunt = true;
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = 0.03 * 60.0;
    ai.def_halt = true;
    let follow = s.mod_active(ci, SKILL_STATUS);
    ai.abil = match (fiery, follow) {
        (false, false) => "Charged Attack",
        (false, true) => "Charged Attack (Follow-Up Strike)",
        (true, false) => "Fiery Passion Charged Attack",
        (true, true) => "Fiery Passion Charged Attack (Follow-Up Strike)",
    };
    let mut hitmark = if fiery { g::FIERY_CHARGE_HITMARK } else { g::CHARGE_HITMARK };
    if follow {
        hitmark = if fiery { g::FAST_FIERY_CHARGE_HITMARK } else { g::FAST_CHARGE_HITMARK };
        s.delete_mod(ci, SKILL_STATUS);
    }
    ai.pattern = s.circle_on_player(0.0, 0.7, 3.5);
    s.queue_attack(ai, 0, hitmark, hitmark);
    let t = if fiery { &g::FIERY_CHARGE_FRAMES } else { &g::CHARGE_FRAMES };
    ActInfo::new(Frames::CharLive { id: if fiery { FRAMES_FIERY_CHARGE } else { FRAMES_CHARGE }, arg: follow as i16 }, t[0], hitmark, AnimState::Charge)
}

/// A plunge's collision, hit, Nightsoul and Apex Drive.
#[allow(clippy::too_many_arguments)]
fn plunge_hit(s: &mut Sim, ci: usize, p: &Params, abil: &'static str, mult: f64, hitmark: i32, radius: f64, fiery: bool) {
    let lvl = s.talent(ci, 0);
    if plunge::wants_collision(p) {
        let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, g::COLLISION[lvl]);
        ai.abil = "Plunge Collision";
        ai.hitlag_factor = 0.02;
        ai.pattern = s.circle_on_player(0.0, 0.0, 1.0);
        s.queue_attack(ai, 0, g::COLLISION_HITMARK, g::COLLISION_HITMARK);
    }
    let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, mult);
    ai.abil = abil;
    ai.add_tags |= add_tag::NIGHTSOUL;
    ai.flat_dmg = a1_bonus(s, ci) + c4_flat(s, ci);
    ai.hitlag_factor = 0.1;
    s.st_mut::<State>(ci).exit_ns = fiery;
    if fiery {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CONSUME_ALL, arg: 0 }, 2);
    } else {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_PLUNGE_POINTS, arg: 0 }, hitmark);
    }
    // getApexDrive
    if s.chars[ci].cons >= 2 || s.ns_has_blessing(ci) {
        s.add_status(ci, APEX, g::APEX_DURATION, true);
        if s.chars[ci].cons >= 6 {
            s.add_energy(ci, 30.0);
        }
    }
    ai.pattern = s.circle_on_player(0.0, 1.0, radius);
    let mut cbs = CB_A1_CANCEL;
    if s.chars[ci].cons >= 2 {
        cbs |= CB_C2;
    }
    if s.chars[ci].cons >= 4 {
        cbs |= CB_C4;
    }
    s.queue_attack(ai, cbs, hitmark, hitmark);
}

fn plunge_info(s: &Sim, ci: usize, frames: Frames, t: &FrameTable) -> ActInfo {
    // the live frames' cancels read at the cast
    let (len, cqa) = match frames {
        Frames::CharLive { id, arg } => (Char::frames_live(s, ci, id, arg, Action::Invalid), Char::frames_live(s, ci, id, arg, Action::Dash)),
        _ => (t[0], t[Action::Dash as usize]),
    };
    let mut info = ActInfo::new(frames, len, cqa, AnimState::Plunge);
    info.on_removed = OnRemoved::Char(REMOVED_CLEAR_NS);
    info
}

/// Volcano Kablam, Apex Drive's burst.
fn kablam(s: &mut Sim, ci: usize) -> ActInfo {
    if s.chars[ci].cons >= 1 {
        a1(s, ci);
    }
    s.st_mut::<State>(ci).kablam_flat = a1_bonus(s, ci);
    s.add_task(Task::Char { c: ci as u8, id: TASK_KABLAM, arg: 0 }, g::KABLAM_HITMARK);
    s.consume_energy_partial(ci, g::KABLAM_COST as f64);
    s.set_cd(ci, Action::Burst, 60);
    s.st_mut::<State>(ci).used_short_burst = true;
    let t = &g::VOLCANIC_FRAMES;
    let mut info = ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Plunge);
    info.on_removed = OnRemoved::Char(REMOVED_KABLAM);
    info
}

fn a1(s: &mut Sim, ci: usize) {
    if s.chars[ci].ascension >= 1 {
        s.add_status(ci, A1, 5 * 60, true);
    }
}

/// a1PlungeBonus: 50% (180% at C1 or in the blessing) of the ATK last read.
fn a1_bonus(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 1 || !s.mod_active(ci, A1) {
        return 0.0;
    }
    let mult = if s.chars[ci].cons >= 1 || s.ns_has_blessing(ci) { 1.8 } else { 0.5 };
    mult * s.st::<State>(ci).a1_atk
}

fn c4_flat(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].cons < 4 || !s.mod_active(ci, C4_STATUS) {
        return 0.0;
    }
    (s.total_atk(ci) * 5.0).min(20000.0)
}

/// generatePlungeNightsoul: 25 points, and the 15 s blessing (a free
/// skill) when they reach 40 outside it.
fn generate_plunge_ns(s: &mut Sim, ci: usize) {
    s.ns_generate(ci, 25.0);
    if !s.ns_has_blessing(ci) && s.ns_points(ci) == s.chars[ci].ns.max_points {
        let p = s.ns_points(ci);
        s.ns_enter_timed(ci, p, 15 * 60, TASK_CLEAR_NS);
        s.st_mut::<State>(ci).free_skill = true;
    }
}

fn clear_ns(s: &mut Sim, ci: usize) {
    s.st_mut::<State>(ci).free_skill = false;
    s.ns_exit(ci);
}

fn clear_ns_cb(s: &mut Sim, ci: usize, next: AnimState) {
    if next == AnimState::Burst {
        return;
    }
    if s.st::<State>(ci).exit_ns {
        clear_ns(s, ci);
        s.st_mut::<State>(ci).exit_ns = false;
    }
}
