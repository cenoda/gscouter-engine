//! Ororon, from gcsim's internal/characters/ororon: normal attacks, aimed
//! shots (with weak spots), Night's Sling (the skill's Spirit Orb), Dark
//! Voices Echo (the burst and its Soundwave Collisions), A1 (Hypersense,
//! his Nightsoul points and his timed Nightsoul's Blessing), A4 and every
//! constellation.
//!
//! His Nightsoul is the general one (`crate::nightsoul`), 80 points at
//! most: A1 gains points from the Nightsoul Burst and, for 15 s after the
//! skill, from the other members' hydro and electro hits; electro-charged,
//! Lunar-Charged and the other members' Nightsoul damage then spend 10 of
//! them on Hypersense, entering his blessing for 6 s when he is not in it.
//!
//! The fight has one enemy: the skill's orb hits it once (gcsim bounces it
//! to up to 3, 5 from C1, enemies), and Hypersense hits it when it stands
//! within 15 of the attack that set it off (gcsim's loop over the enemies
//! in that circle runs while fewer than 4 are found, so it hits each of up
//! to 3 and none of 4 or more).

use crate::content::{Character, Event};
use crate::gcsimdata::chars::ororon as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The skill made its particles (gcsim's particlesGenerated).
    particles_generated: bool,
    /// The frame of the last burst (gcsim's burstSrc).
    burst_src: i32,
    /// The burst's area, a circle in front of the player as it was cast
    /// (gcsim's burstArea), where its ticks land.
    burst_area: Pattern,
    /// gcsim's tags "ororon-a1-electro-hydro", "ororon-a4" and "ororon-c2"
    /// (gcsim also sets "ororon-a1" to 0 and never reads it).
    tag_a1_eh: i32,
    tag_a1: i32,
    tag_a4: i32,
    tag_c2: i32,
    /// C6's stacks (gcsim's MultipleRefreshNoRemove of 3): each slot's
    /// frame and a token that tells its expiry task whether the slot was
    /// taken since (gcsim compares pointers), 0 for an empty slot.
    c6_stacks: [(i32, i32); 3],
    c6_next_token: i32,
}

const BURST_STATUS: ModKey = ModKey::new(g::BURST_STATUS);
const A1_ON_SKILL: ModKey = ModKey::new(g::A1_ON_SKILL_KEY);
const A1_GAIN_ICD: ModKey = ModKey::new(g::A1_GAIN_ICD_KEY);
const A1_DAMAGE_ICD: ModKey = ModKey::new(g::A1_DAMAGE_ICD_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const C1: ModKey = ModKey::new(g::C1_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_A4: u16 = 1 << 1;
const CB_C1: u16 = 1 << 2;
const CB_C2: u16 = 1 << 3;

/// The burst's tick, with its src (the fight's queue).
const TASK_BURST_TICK: u8 = 0;
/// C4's energy at the burst's hitmark.
const TASK_C4_ENERGY: u8 = 1;
/// A1's end of the blessing (gcsim's ExitBlessing, queued unconditionally).
const TASK_EXIT: u8 = 2;
/// A C6 stack's expiry: its slot and token, as token * 4 + slot.
const TASK_C6_EXPIRE: u8 = 3;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];

impl Character for Char {
    const KEY: &'static str = "ororon";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            particles_generated: false,
            burst_src: 0,
            burst_area: Pattern::Unknown,
            tag_a1_eh: 0,
            tag_a1: 0,
            tag_a4: 0,
            tag_c2: 0,
            c6_stacks: [(0, 0); 3],
            c6_next_token: 1,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].ns.max_points = 80.0;
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // a1Init: OnNightsoulBurst, OnEnemyHit, OnElectroCharged and
        // OnLunarCharged (one reaction subscription), OnEnemyDamage; then
        // a4Init's OnEnemyHit (A4 needs A1's ascension), handled in the
        // same subscription after A1's
        if asc >= 1 {
            s.subscribe(EventKind::NightsoulBurst, o);
            s.subscribe(EventKind::EnemyHit, o);
            s.subscribe(EventKind::Reaction, o);
            s.subscribe(EventKind::EnemyDamage, o);
        }
        // C1: Hypersense DMG +50% on an enemy the orb marked
        if cons >= 1 {
            s.chars[ci].mods.push(Mod::owned(C1, ModKind::Attack, o, st::NONE, false, false, -1));
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            Action::Aim if !matches!(p.get("hold"), None | Some(0) | Some(1)) => Some("aim[hold]".into()),
            Action::Aim => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let st = s.st::<State>(ci);
        match (fields[0].as_str(), fields.get(1).map(String::as_str)) {
            ("nightsoul", _) => Some(s.ns_condition(ci, fields)),
            ("tags", Some(g::A1_ELECTRO_HYDRO_KEY)) => Some(Ok(Num::Int(st.tag_a1_eh as i64))),
            ("tags", Some(g::A1_ON_SKILL_KEY)) => Some(Ok(Num::Int(st.tag_a1 as i64))),
            ("tags", Some(g::A4_KEY)) => Some(Ok(Num::Int(st.tag_a4 as i64))),
            ("tags", Some(g::C2_KEY)) => Some(Ok(Num::Int(st.tag_c2 as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let hitmark = g::ATTACK_HITMARKS[n];
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        s.queue_attack(ai, 0, hitmark, hitmark + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hitmark, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::FULLAIM[lvl]);
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
        // gcsim's aimedFrames has a third, empty slice
        let table: &'static FrameTable = g::AIMED_FRAMES[hold].try_into().unwrap();
        ActInfo::new(Frames::Table(table), table[0], hitmark, AnimState::Aim)
    }

    /// Night's Sling: the Spirit Orb on the enemy, A1's window for points.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        s.st_mut::<State>(ci).particles_generated = false;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SPIRIT_ORB[lvl]);
        ai.abil = "Spirit Orb DMG";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.hitlag_factor = 0.05;
        ai.pattern = s.circle_on_target(0.6);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        let mut cbs = CB_PARTICLE;
        if asc >= 4 {
            cbs |= CB_A4;
        }
        if cons >= 1 && asc >= 1 {
            cbs |= CB_C1;
        }
        // the primary target, the first of up to 3 (5 from C1): the only
        // enemy
        s.queue_attack(ai, cbs, g::SKILL_HITMARK, g::SKILL_HITMARK + travel);
        s.set_cd_with_delay(ci, Action::Skill, 15 * 60, 14);
        // a1OnSkill
        if asc >= 1 {
            s.add_status(ci, A1_ON_SKILL, 15 * 60, true);
            s.st_mut::<State>(ci).tag_a1 = 0;
        }
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    /// Dark Voices Echo: the Ritual hit, then a Soundwave Collision every
    /// second (0.75 s from C4) while Supersonic Oculus lasts.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let cons = s.chars[ci].cons;
        let c2 = if cons >= 2 { CB_C2 } else { 0 };
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::BURST[lvl]);
        ai.abil = "Ritual DMG";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.hitlag_factor = 0.05;
        let area = s.circle_on_player(0.0, 2.3, 6.5);
        ai.pattern = area;
        s.queue_attack(ai, c2, g::BURST_HITMARK, g::BURST_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C4_ENERGY, arg: 0 }, g::BURST_HITMARK);
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        st.burst_area = area;
        st.burst_src = f;
        s.add_status(ci, BURST_STATUS, 9 * 60, false);
        queue_burst_tick(s, ci, f);
        // c2OnBurst: ELECTRO DMG +8% per hit of the burst, up to 32%
        if cons >= 2 {
            s.st_mut::<State>(ci).tag_c2 = 1;
            s.add_mod(ci, Mod::owned(C2, ModKind::Stat, Owner::Char(ci as u8), st::ELECTROP, false, true, -1), 9 * 60);
        }
        // c6OnBurst
        if cons >= 6 && s.chars[ci].ascension >= 1 {
            let pos = s.enemy_pos;
            hypersense(s, ci, 3.2, "Hypersense (C6)", pos);
        }
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 22);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_BURST_TICK => {
                if s.st::<State>(ci).burst_src != arg || !s.mod_active(ci, BURST_STATUS) {
                    return;
                }
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ORORON_ELMENTAL_BURST, icd::GROUP_ORORON_ELEMENTAL_BURST, Element::Electro, 25.0, g::SOUNDWAVE[lvl]);
                ai.abil = "Soundwave Collision DMG";
                ai.add_tags = add_tag::NIGHTSOUL;
                ai.hitlag_factor = 0.02;
                ai.pattern = s.st::<State>(ci).burst_area;
                let c2 = if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
                s.queue_attack(ai, c2, 0, 0);
                queue_burst_tick(s, ci, arg);
            }
            // c4EnergyRestore
            TASK_C4_ENERGY => {
                if s.chars[ci].cons >= 4 {
                    s.add_energy(ci, 8.0);
                }
            }
            TASK_EXIT => s.ns_exit(ci),
            TASK_C6_EXPIRE => {
                let (slot, token) = ((arg & 3) as usize, arg >> 2);
                let st = s.st_mut::<State>(ci);
                if st.c6_stacks[slot].1 == token {
                    st.c6_stacks[slot] = (0, 0);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.st::<State>(ci).particles_generated {
            s.st_mut::<State>(ci).particles_generated = true;
            s.queue_particle(3.0, Element::Electro, s.particle_delay[ci]);
        }
        if cbs & CB_A4 != 0 {
            s.add_status(ci, A4, 15 * 60, true);
            s.st_mut::<State>(ci).tag_a4 = 0;
        }
        if cbs & CB_C1 != 0 {
            s.enemy_add_status(C1, 12 * 60, true);
        }
        if cbs & CB_C2 != 0 {
            s.st_mut::<State>(ci).tag_c2 += 1;
        }
    }

    /// C2's ELECTRO DMG and C6's ATK on the active character.
    fn mod_amount(s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool {
        let st = s.st::<State>(ci);
        if m.key == C2 {
            out[st::ELECTROP] += (0.08 * st.tag_c2 as f64).min(0.32);
            return true;
        }
        // C6
        if s.player.active as usize != on {
            return false;
        }
        let count = st.c6_stacks.iter().filter(|x| x.1 != 0).count();
        out[st::ATKP] += count as f64 * 0.1;
        true
    }

    /// C1: Hypersense (A1's, not C6's) DMG +50% on an enemy the orb marked.
    fn attack_mod(s: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.abil == g::A1_ABIL && s.enemy_status_active(C1) {
            ev.stats[st::DMGP] += 0.5;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // a1NSBurstKey
            Event::NightsoulBurst(_) => s.ns_generate(ci, 40.0),
            Event::EnemyHit(ev) => {
                if s.chars[ci].ascension >= 1 {
                    a1_hit(s, ci, ev);
                }
                if s.chars[ci].ascension >= 4 {
                    a4_hit(s, ci, ev);
                }
            }
            Event::Reaction(Reaction::ElectroCharged | Reaction::LunarCharged, ev) => a1_nightsoul_attack(s, ci, ev),
            Event::EnemyDamage(ev) => {
                if ev.info.actor as usize == ci || !ev.info.has_tag(add_tag::NIGHTSOUL) {
                    return;
                }
                a1_nightsoul_attack(s, ci, ev);
            }
            _ => {}
        }
    }
}

fn queue_burst_tick(s: &mut Sim, ci: usize, src: i32) {
    let interval = if s.chars[ci].cons >= 4 { 0.75 } else { 1.0 };
    let tick = (g::BURST_TICK_HITMARK as f64 * interval) as i32;
    s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_TICK, arg: src }, tick);
}

/// A1: for 15 s after the skill, another member's hydro or electro hit
/// gains 5 points, once every 0.3 s, ten times.
fn a1_hit(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    if ev.info.actor as usize == ci || !matches!(ev.info.element, Element::Hydro | Element::Electro) {
        return;
    }
    if !s.mod_active(ci, A1_ON_SKILL) || s.mod_active(ci, A1_GAIN_ICD) {
        return;
    }
    s.add_status(ci, A1_GAIN_ICD, 18, true);
    s.ns_generate(ci, 5.0);
    let st = s.st_mut::<State>(ci);
    st.tag_a1_eh += 1;
    if st.tag_a1_eh >= 10 {
        s.delete_mod(ci, A1_ON_SKILL);
    }
}

/// A4: for 15 s after the orb hits, the active character's normal,
/// charged or plunging hit gives it and Ororon 3 energy, once a second,
/// three times.
fn a4_hit(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    let active = s.player.active as usize;
    if ev.info.actor as usize != active || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
        return;
    }
    if !s.mod_active(ci, A4) || s.mod_active(ci, A4_ICD) {
        return;
    }
    s.add_status(ci, A4_ICD, 60, true);
    s.add_energy(active, 3.0);
    if active != ci {
        s.add_energy(ci, 3.0);
    }
    let st = s.st_mut::<State>(ci);
    st.tag_a4 += 1;
    if st.tag_a4 >= 3 {
        s.delete_mod(ci, A4);
    }
}

/// a1NightSoulAttack: with 10 points or more, once every 1.8 s, the
/// blessing for 6 s if not in it, 10 points, and Hypersense around the
/// attack that set it off.
fn a1_nightsoul_attack(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    if s.ns_points(ci) < 10.0 || s.mod_active(ci, A1_DAMAGE_ICD) {
        return;
    }
    // gcsim reads the attack's position (Pattern.Shape.Pos())
    let pos = match ev.info.pattern {
        Pattern::Single => (0.0, 0.0),
        Pattern::Circle { x, y, .. } | Pattern::Fan { x, y, .. } | Pattern::FanDir { x, y, .. } | Pattern::Box { x, y, .. } => (x, y),
        Pattern::Unknown => {
            let who = crate::gcsl::char_name(s.chars[ev.info.actor as usize].key);
            s.unsupported(&format!("hit pattern {who} {} for Ororon's A1", ev.info.abil));
            return;
        }
    };
    s.add_status(ci, A1_DAMAGE_ICD, 108, true);
    if !s.ns_has_blessing(ci) {
        // a1EnterBlessing
        let pts = s.ns_points(ci);
        s.ns_enter(ci, pts);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_EXIT, arg: 0 }, 6 * 60);
    }
    s.ns_consume(ci, 10.0);
    hypersense(s, ci, 1.6, g::A1_ABIL, pos);
}

/// Hypersense: electro Nightsoul damage on each enemy within 15 of `at`
/// (one here), then C6's stack.
fn hypersense(s: &mut Sim, ci: usize, mult: f64, abil: &'static str, at: (f64, f64)) {
    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, mult);
    ai.abil = abil;
    ai.add_tags = add_tag::NIGHTSOUL;
    ai.hitlag_factor = 0.01;
    ai.def_halt = true;
    if s.enemy_alive && s.enemy_within(Pattern::Circle { x: at.0, y: at.1, r: 15.0 }) {
        ai.pattern = s.box_on_enemy(0.0, 0.0, 0.2, 0.2);
        s.queue_attack(ai, 0, 12, 12);
    }
    c6_on_hypersense(s, ci);
}

/// C6: a stack for 9 s (3 at most, the oldest replaced), and the party's
/// ATK +10% per stack on the active character.
fn c6_on_hypersense(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 6 || s.chars[ci].ascension < 1 {
        return;
    }
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    let mut idx = 0;
    for i in 0..3 {
        if st.c6_stacks[i].1 == 0 {
            idx = i;
            break;
        }
        if st.c6_stacks[i].0 < st.c6_stacks[idx].0 {
            idx = i;
        }
    }
    let token = st.c6_next_token;
    st.c6_next_token += 1;
    st.c6_stacks[idx] = (f, token);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6_EXPIRE, arg: token << 2 | idx as i32 }, 9 * 60);
    for i in 0..s.chars.len() {
        s.add_mod(i, Mod::owned(C6, ModKind::Stat, Owner::Char(ci as u8), st::NONE, false, true, -1), 9 * 60);
    }
}
