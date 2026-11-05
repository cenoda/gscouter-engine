//! Xianyun, from gcsim's internal/characters/xianyun: normal and charged
//! attacks, White Clouds at Dawn (up to three leaps of Skyladder) and
//! Driftcloud Wave, her own airborne plunges, Stars Gather at Dusk with its
//! heals, the airborne buff it gives the party and the Starwicker that
//! answers their plunges (Adeptal Assistance), A1, A4 and every
//! constellation.
//!
//! As in gcsim, C4's heal reads the leap count after Driftcloud Wave has
//! reset it, so it heals with the ratio of zero leaps.

use crate::content::{Character, Event};
use crate::gamedata::chars::xianyun as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    skill_counter: i32,
    skill_src: i32,
    skill_was_c6: bool,
    /// The one enemy took Skyladder's damage in this Cloud
    /// Transmogrification state.
    skill_enemy_hit: bool,
    /// A1's Boost stacks, by character.
    a1_buffer: [i32; 4],
    a4_atk: f64,
    a4_src: i32,
    a4_max: f64,
    a4_ratio: f64,
    adeptal_stacks: i32,
    /// gcsim's tag of xianyun-c6: the leaps C6 has left.
    c6_tag: i32,
    /// Driftcloud Wave's area, taken around the player at the action.
    driftcloud_pat: Pattern,
}

const SKILL_STATE: ModKey = ModKey::new(g::SKILL_STATE_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const LOSS: ModKey = ModKey::new(g::LOSS_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const A1_BUFF: ModKey = ModKey::new("xianyun-a1-buff");
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const A4_WINDOW: ModKey = ModKey::new(g::A4_WINDOW_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C4_ICD: ModKey = ModKey::new(g::C4_ICD);
const C6: ModKey = ModKey::new(g::C6_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_A1: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;

// arg: the skill's frame
const TASK_CD_REDUCE: u8 = 0;
// arg: the leap index
const TASK_DRIFTCLOUD: u8 = 1;
const TASK_BURST: u8 = 2;
const TASK_BURST_HEAL: u8 = 3;
// arg: the burst's frame
const TASK_A4_UPDATE: u8 = 4;
// arg: the character
const TASK_A1_EXPIRE: u8 = 5;

const DRIFTCLOUD_NAMES: [&str; 4] = ["Driftcloud Wave (0 Leaps)", "Driftcloud Wave (1 Leaps)", "Driftcloud Wave (2 Leaps)", "Driftcloud Wave (3 Leaps)"];

impl Character for Char {
    const KEY: &'static str = "xianyun";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            skill_counter: 0,
            skill_src: g::NO_SRC_VAL,
            skill_was_c6: false,
            skill_enemy_hit: false,
            a1_buffer: [0; 4],
            a4_atk: 0.0,
            a4_src: 0,
            a4_max: 0.0,
            a4_ratio: 0.0,
            adeptal_stacks: 0,
            c6_tag: 0,
            driftcloud_pat: Pattern::Unknown,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // a1: the CRIT Rate mod on every character, whatever her ascension
        for i in 0..s.chars.len() {
            s.chars[i].mods.push(Mod::owned(A1_BUFF, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        // a4
        if s.chars[ci].ascension >= 4 {
            let st = s.st_mut::<State>(ci);
            st.a4_max = 9000.0;
            st.a4_ratio = 2.0;
            s.subscribe(EventKind::EnemyHit, o);
        }
        // c1: two charges of the skill
        if s.chars[ci].cons >= 1 {
            let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
            cd.available = 2;
            cd.max_extra = 1;
        }
        // c2
        if s.chars[ci].cons >= 2 {
            let st = s.st_mut::<State>(ci);
            st.a4_max = 18000.0;
            st.a4_ratio = 4.0;
        }
        // burstPlungeDoTTrigger
        s.subscribe(EventKind::ApplyAttack, o);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let unknown = |known: &[&str]| p.0.iter().find(|(k, _)| !known.iter().any(|n| k == *n)).map(|(k, _)| format!("{name} param {k}"));
        match a {
            Action::Attack | Action::Charge => unknown(&["travel"]),
            Action::LowPlunge | Action::HighPlunge => unknown(&["collision"]),
            Action::Skill | Action::Burst => unknown(&[]),
            _ => Some(name.into()),
        }
    }

    /// Her plunges: Driftcloud Wave in the skill's state, otherwise only
    /// while airborne.
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        match a {
            Action::LowPlunge | Action::HighPlunge if s.mod_active(ci, SKILL_STATE) => None,
            Action::LowPlunge | Action::HighPlunge if s.player.airborne == Airborne::Grounded => {
                Some(format!("xianyun {} can only be used while airborne", crate::gcsl::action_name(a)))
            }
            _ => crate::content::template_invalid(s, a),
        }
    }

    /// Up to three leaps per state, on cooldown or not (and in C6's state);
    /// no normal or charged attack in the skill's state.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        let state = s.mod_active(ci, SKILL_STATE);
        if a == Action::Skill && (state || s.mod_active(ci, C6)) && s.st::<State>(ci).skill_counter < 3 {
            return Some(None);
        }
        if matches!(a, Action::Attack | Action::Charge) && state {
            return Some(Some(Failure::NoFailure));
        }
        None
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "adeptal-assistance" => Some(Ok(Num::Int(s.st::<State>(ci).adeptal_stacks as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").unwrap_or(10) as i32;
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.circle_on_target(g::ATTACK_RADIUS[n]);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").unwrap_or(10) as i32;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::CHARGED[lvl]);
        ai.abil = "Charge Attack";
        ai.pattern = s.circle_on_target(3.0);
        let windup = if s.current_state() == AnimState::Normal { 14 } else { 0 };
        let h = g::CHARGE_HITMARK - windup;
        s.queue_attack(ai, 0, h, h + travel);
        ActInfo::new(Frames::Char { id: 0, arg: windup as i16 }, g::CHARGE_FRAMES[0] - windup, h, AnimState::Charge)
    }

    fn frames(_: u8, windup: i16, next: Action) -> i32 {
        g::CHARGE_FRAMES[next as usize] - windup as i32
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // a first leap, or one after three leaps without a plunge
        if !s.mod_active(ci, SKILL_STATE) || s.st::<State>(ci).skill_counter == 3 {
            s.st_mut::<State>(ci).skill_counter = 0;
            if s.mod_active(ci, C6) {
                let st = s.st_mut::<State>(ci);
                st.skill_was_c6 = true;
                st.c6_tag -= 1;
                if st.c6_tag <= 0 {
                    s.delete_mod(ci, C6);
                }
            } else {
                s.set_cd(ci, Action::Skill, 12 * 60);
                s.st_mut::<State>(ci).skill_was_c6 = false;
            }
            s.st_mut::<State>(ci).skill_enemy_hit = false;
        }
        // C2: ATK +20% for 15 s
        if s.chars[ci].cons >= 2 {
            s.add_stat_mod(ci, C2, st::ATKP, 0.2, g::C2_DUR, true);
        }
        // Skyladder hits each enemy once per state, those whose center is
        // within its circle (gcsim's EnemiesWithinArea: PointInShape)
        let area = s.circle_on_player(0.0, 0.0, g::SKILL_RADIUS);
        if s.enemy_alive && !s.st::<State>(ci).skill_enemy_hit && s.enemy_within(area) {
            let lvl = s.talent(ci, 1);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 0.0, g::SKILL_PRESS[lvl]);
            ai.abil = "Skyladder";
            ai.pattern = Pattern::Single;
            s.queue_attack(ai, 0, g::SKILL_PRESS_HITMARK, g::SKILL_PRESS_HITMARK);
            s.st_mut::<State>(ci).skill_enemy_hit = true;
        }
        let f = s.f;
        let n = s.st::<State>(ci).skill_counter as usize;
        s.st_mut::<State>(ci).skill_src = f;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CD_REDUCE, arg: f }, g::SKILL_STATE_DUR[n]);
        s.add_status(ci, SKILL_STATE, g::SKILL_STATE_DUR[n], true);
        s.st_mut::<State>(ci).skill_counter += 1;
        let t = &g::SKILL_LEAP_FRAMES[n];
        ActInfo::new(Frames::Table(t), t[0], t[Action::HighPlunge as usize], AnimState::Skill)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.mod_active(ci, SKILL_STATE) {
            return driftcloud_wave(s, ci);
        }
        plunge_xy(s, ci, true, p)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.mod_active(ci, SKILL_STATE) {
            return driftcloud_wave(s, ci);
        }
        plunge_xy(s, ci, false, p)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.set_cd(ci, Action::Burst, 18 * 60);
        s.consume_energy(ci, 18);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST, arg: 0 }, g::BURST_HITMARK);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_CD_REDUCE => {
                // no Driftcloud Wave in the state: the next cooldown is 3 s
                // shorter
                if s.st::<State>(ci).skill_src == arg {
                    s.reduce_cd(ci, Action::Skill, 3 * 60);
                }
            }
            TASK_DRIFTCLOUD => {
                let ind = arg as usize;
                let lvl = s.talent(ci, 1);
                let counter = s.st::<State>(ci).skill_counter;
                let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::LEAP[ind][lvl]);
                ai.abil = DRIFTCLOUD_NAMES[counter.clamp(0, 3) as usize];
                ai.pattern = s.st::<State>(ci).driftcloud_pat;
                let mut ev = s.snapshot_event(ai, 0);
                // C6: CRIT DMG by the leaps
                if s.chars[ci].cons >= 6 {
                    ev.stats[st::CD] += g::C6_BUFF[counter.clamp(0, 3) as usize];
                }
                let mut cbs = 0;
                if !s.st::<State>(ci).skill_was_c6 {
                    cbs |= CB_PARTICLE;
                }
                if s.chars[ci].ascension >= 1 {
                    cbs |= CB_A1;
                }
                if s.chars[ci].cons >= 4 {
                    cbs |= CB_C4;
                }
                s.queue_attack_with_snap(ev, cbs, 0);
                // the window ends with the leap
                s.delete_mod(ci, SKILL_STATE);
                let st = s.st_mut::<State>(ci);
                st.skill_counter = 0;
                st.skill_enemy_hit = false;
                st.skill_src = g::NO_SRC_VAL;
            }
            TASK_BURST => burst_cast(s, ci),
            TASK_BURST_HEAL => {
                let lvl = s.talent(ci, 2);
                let src = g::HEAL_DOT_P[lvl] * total_atk(&s.stats(ci)) + g::HEAL_DOT_FLAT[lvl];
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, None, false, src, bonus);
            }
            TASK_A4_UPDATE => a4_update(s, ci, arg),
            TASK_A1_EXPIRE => {
                s.st_mut::<State>(ci).a1_buffer[arg as usize] -= 1;
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD as i32, true);
            s.queue_particle(g::PARTICLE_COUNT as f64, Element::Anemo, s.particle_delay[ci]);
        }
        if cbs & CB_A1 != 0 {
            for i in 0..s.chars.len() {
                s.st_mut::<State>(ci).a1_buffer[i] += 1;
                s.add_status(i, A1, g::A1_DUR, true);
                s.queue_char_task(i, Task::Char { c: ci as u8, id: TASK_A1_EXPIRE, arg: i as i32 }, g::A1_DUR);
            }
        }
        if cbs & CB_C4 != 0 && !s.mod_active(ci, C4_ICD) {
            let counter = s.st::<State>(ci).skill_counter.clamp(0, 3) as usize;
            let src = g::C4_RATIO[counter] * total_atk(&s.stats(ci));
            let bonus = s.stat(ci, st::HEAL);
            s.heal_info(ci, None, false, src, bonus);
            s.add_status(ci, C4_ICD, g::C4_ICD_DUR, true);
        }
    }

    /// A1: Boost's CRIT Rate on plunges.
    fn attack_mod(s: &Sim, ci: usize, on: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag != AttackTag::Plunge {
            return;
        }
        let stacks = s.st::<State>(ci).a1_buffer[on].min(4);
        if stacks == 0 {
            return;
        }
        ev.stats[st::CR] += g::A1_CRIT[stacks as usize];
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::ApplyAttack(atk) => starwicker(s, ci, atk),
            Event::EnemyHit(ae) => {
                // A4: the plunge's shockwave gains flat damage from her ATK
                if ae.info.tag != AttackTag::Plunge || ae.info.durability == 0.0 {
                    return;
                }
                if !s.mod_active(ci, A4_WINDOW) || s.mod_active(ci, A4_ICD) {
                    return;
                }
                let st = s.st::<State>(ci);
                let amt = st.a4_max.min(st.a4_ratio * st.a4_atk);
                s.hit_flat += amt;
                s.add_status(ci, A4_ICD, 24, true);
            }
            _ => {}
        }
    }
}

/// gcsim's tags on characters that Xianyun sets: xianyun-a1 on each, and
/// xianyun-c6 on her.
pub fn tag(s: &Sim, ci: usize, key: &str) -> i64 {
    let Some(xi) = s.chars.iter().position(|c| c.kind == CharKind::Xianyun) else { return 0 };
    let st = s.st::<State>(xi);
    match key {
        g::A1_KEY => st.a1_buffer[ci].min(4) as i64,
        g::C6_KEY if ci == xi => st.c6_tag as i64,
        _ => 0,
    }
}

/// Driftcloud Wave, the plunge after a leap.
fn driftcloud_wave(s: &mut Sim, ci: usize) -> ActInfo {
    let ind = (s.st::<State>(ci).skill_counter - 1) as usize;
    // gcsim takes the area as she plunges, and hits from it at the hitmark
    let area = s.circle_on_player(0.0, 0.0, g::PLUNGE_RADIUS[ind]);
    s.st_mut::<State>(ci).driftcloud_pat = area;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_DRIFTCLOUD, arg: ind as i32 }, g::PLUNGE_HITMARKS[ind]);
    let t = &g::DRIFTCLOUD_FRAMES[ind];
    ActInfo::new(Frames::Table(t), t[0], t[Action::Charge as usize], AnimState::Plunge)
}

/// Her low or high plunge while airborne.
fn plunge_xy(s: &mut Sim, ci: usize, high: bool, p: &Params) -> ActInfo {
    let lvl = s.talent(ci, 0);
    if p.get("collision").unwrap_or(0) > 0 {
        let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 0.0, g::COLLISION[lvl]);
        ai.abil = "Plunge Collision";
        ai.pattern = s.circle_on_player(0.0, 0.0, 1.5);
        s.queue_attack(ai, 0, g::COLLISION_HITMARK, g::COLLISION_HITMARK);
    }
    let (mult, h, r, t) = if high {
        (g::HIGH_PLUNGE[lvl], g::HIGH_PLUNGE_HITMARK, g::HIGH_PLUNGE_RADIUS, &g::HIGH_PLUNGE_FRAMES_XY)
    } else {
        (g::LOW_PLUNGE[lvl], g::LOW_PLUNGE_HITMARK, g::LOW_PLUNGE_RADIUS, &g::LOW_PLUNGE_FRAMES_XY)
    };
    let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, mult);
    ai.abil = if high { "High Plunge" } else { "Low Plunge" };
    ai.pattern = s.circle_on_player(0.0, 0.0, r);
    s.queue_attack(ai, 0, h, h);
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Plunge)
}

/// The burst at its hitmark: the hit, the first heal, the airborne buff on
/// every character, eight Adeptal Assistance stacks, the heals over time
/// and A4's ATK.
fn burst_cast(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST[lvl]);
    ai.abil = "Stars Gather at Dusk (Initial)";
    ai.pattern = s.circle_on_player(0.0, 0.0, 7.0);
    s.queue_attack(ai, 0, 0, 0);
    let src = g::HEAL_INSTANT_P[lvl] * total_atk(&s.stats(ci)) + g::HEAL_INSTANT_FLAT[lvl];
    let bonus = s.stat(ci, st::HEAL);
    s.heal_info(ci, None, false, src, bonus);
    s.add_status(ci, BURST, g::BURST_DURATION, false);
    // c6
    if s.chars[ci].cons >= 6 {
        s.add_status(ci, C6, g::C6_DUR, true);
        s.st_mut::<State>(ci).c6_tag = 8;
    }
    for i in 0..s.chars.len() {
        s.add_status(i, XIANYUN_AIRBORNE, g::BURST_DURATION, false);
    }
    s.st_mut::<State>(ci).adeptal_stacks = 8;
    // unaffected by hitlag
    let mut i = g::BURST_HEAL;
    while i <= g::BURST_HEAL + g::BURST_DURATION {
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_HEAL, arg: 0 }, i);
        i += 150;
    }
    // a4StartUpdate
    if s.chars[ci].ascension >= 4 {
        let f = s.f;
        s.st_mut::<State>(ci).a4_src = f;
        a4_update(s, ci, f);
    }
}

/// A4's ATK, taken every half second from the latest burst.
fn a4_update(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).a4_src != src {
        return;
    }
    let atk = total_atk(&s.stats(ci));
    s.st_mut::<State>(ci).a4_atk = atk;
    s.add_task(Task::Char { c: ci as u8, id: TASK_A4_UPDATE, arg: src }, 30);
}

/// burstPlungeDoTTrigger: a plunge's shockwave by the active character,
/// under the airborne buff, consumes a stack for a Starwicker hit.
fn starwicker(s: &mut Sim, ci: usize, atk: &AttackEvent) {
    // Raiden's plunges in Musou Isshin are burst damage, found by name
    if atk.info.tag != AttackTag::Plunge && !atk.info.abil.contains("Low Plunge") && !atk.info.abil.contains("High Plunge") {
        return;
    }
    // plunge collisions have no gauge
    if atk.info.durability == 0.0 {
        return;
    }
    let active = s.player.active as usize;
    if active != atk.info.actor as usize || !s.mod_active(active, XIANYUN_AIRBORNE) {
        return;
    }
    if s.st::<State>(ci).adeptal_stacks <= 0 || s.mod_active(ci, LOSS) {
        return;
    }
    s.add_status(ci, LOSS, g::LOSS_ICD, false);
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST_DOT[lvl]);
    ai.abil = "Starwicker";
    ai.pattern = s.circle_on_player(0.0, 0.0, g::BURST_DO_T_RADIUS);
    s.queue_attack(ai, 0, g::BURST_DO_T_DELAY, g::BURST_DO_T_DELAY);
    s.st_mut::<State>(ci).adeptal_stacks -= 1;
    if s.st::<State>(ci).adeptal_stacks == 0 {
        for i in 0..s.chars.len() {
            s.delete_mod(i, XIANYUN_AIRBORNE);
        }
    }
    // keep a window open for A4
    s.add_status(ci, A4_WINDOW, 1, false);
}
