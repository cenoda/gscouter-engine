//! Nicole, from gcsim's internal/characters/nicole: normal and charged
//! attacks, Revelation: Uncreated Light (the hit, the Shield of Blazing
//! Light and Grace of Kenosis, the party's flat ATK from her ATK), the
//! burst's Silent Contemplation and its Arcane Projections, A1 (Guidance of
//! Theosis on the member swapped in, at once for a Hexerei member), A4 (the
//! same on Nicole from the active member's elemental hits), her Hexerei
//! bonus (the projections' flat damage) and every constellation.
//!
//! The projections are the active member's attacks: no tag, no durability,
//! that member's element, at a hitbox by member (gcsim's projectionHitbox:
//! Albedo, Razor, Sucrose and Klee have their own, everyone else the
//! default). gcsim's quirks, kept: A4 has no ascension check (before A1 the
//! upgrade adds nothing); the skill adds C2's buff to every member once per
//! member; the charged attack's windup shortens its frames but not its
//! hitmark; a normal attack after the third, still in the normal attack
//! state, takes a windup of 7.
//!
//! The fight has one enemy: C2's resistance shred lands on it when it
//! stands within 10 of the player.

use crate::content::{Character, Event, EventKind, Owner};
use crate::gamedata::chars::nicole as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    projections: i32,
    /// The frame of the last swap A1 answers (gcsim's a1Src).
    a1_src: i32,
    /// gcsim's "nicole-c2" tag (the frame of its ticker) and "nicole-c4"
    /// tag (stacks) on each member.
    c2_tags: [i32; 4],
    c4_tags: [i32; 4],
    /// The attack speed the current normal attack's frames read (gcsim's
    /// attackFuncWithWindup fixes it as the attack starts).
    fr_atkspd: f64,
}

const SKILL_BUFF: ModKey = ModKey::new(g::SKILL_BUFF_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const BURST_ICD: ModKey = ModKey::new(g::BURST_ICD_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C4: ModKey = ModKey::new(g::C4_KEY);
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const SHIELD: ModKey = ModKey::new("nicole-shield");

const CB_PARTICLE: u16 = 1 << 0;

/// The skill's shield and Grace of Kenosis, a frame before its hitmark.
const TASK_SKILL_BUFF: u8 = 0;
/// The burst's Silent Contemplation, a frame after its hitmark.
const TASK_BURST_STATE: u8 = 1;
/// An Arcane Projection for the member in arg (the fight's queue).
const TASK_PROJECTION: u8 = 2;
/// C1's Arcane Projection: Unity for the member in arg (the fight's queue).
const TASK_UNITY: u8 = 3;
/// A1's upgrade on the member swapped in: src * 4 + member.
const TASK_A1_SWAP: u8 = 4;
/// C2's ticker: src * 4 + member (the fight's queue).
const TASK_C2_TICK: u8 = 5;
/// The charged attack's hit (the fight's queue).
const TASK_CHARGE: u8 = 6;

const FR_CHARGE: u8 = 0;
/// A normal attack's frames: arg is counter * 16 + windup.
const FR_NORMAL: u8 = 1;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];

impl Character for Char {
    const KEY: &'static str = "nicole";
    const HEXEREI: bool = true;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { projections: 0, a1_src: 0, c2_tags: [0; 4], c4_tags: [0; 4], fr_atkspd: 0.0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // burstInit, a4Init, c1Init, c4Init and c6Init subscribe to
        // OnEnemyHit (one subscription, handled in that order); a1Init to
        // OnCharacterSwap
        s.subscribe(EventKind::EnemyHit, o);
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::Swap, o);
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "projections" => {
                let n = if s.mod_active(ci, BURST) { s.st::<State>(ci).projections } else { 0 };
                Some(Ok(Num::Int(n as i64)))
            }
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let windup = match s.current_state() {
            AnimState::Normal if n == 0 => 7,
            AnimState::Charge => 3,
            _ => 0,
        };
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        let hb = g::ATTACK_HITBOXES[n];
        ai.pattern = if n == 0 { s.box_on_enemy(0.0, -1.0, hb[0], hb[1]) } else { s.circle_on_enemy(0.0, 0.0, hb[0]) };
        let hm = g::ATTACK_HITMARKS[n] + windup;
        s.queue_attack(ai, 0, hm, hm);
        let atkspd = s.stat(ci, st::ATKSPD);
        s.st_mut::<State>(ci).fr_atkspd = atkspd;
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::CharLive { id: FR_NORMAL, arg: (n as i16) << 4 | windup as i16 }, table[0] + windup, hm, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let windup = if s.current_state() == AnimState::Idle { 2 } else { 0 };
        s.add_task(Task::Char { c: ci as u8, id: TASK_CHARGE, arg: 0 }, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: FR_CHARGE, arg: windup as i16 }, t[0] - windup, t[Action::Jump as usize] - windup, AnimState::Charge)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_CHARGE => g::CHARGE_FRAMES[next as usize] - arg as i32,
            _ => unreachable!(),
        }
    }

    /// gcsim's attackFuncWithWindup: the attack speed fixed at the start,
    /// the windup after the adjustment.
    fn frames_live(s: &Sim, ci: usize, id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_NORMAL => {
                let (n, windup) = ((arg >> 4) as usize, (arg & 15) as i32);
                atk_spd_adjust(g::ATTACK_FRAMES[n][next as usize], s.st::<State>(ci).fr_atkspd) + windup
            }
            _ => unreachable!(),
        }
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL[s.talent(ci, 1)]);
        ai.abil = "Revelation: Uncreated Light";
        ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
        s.queue_attack(ai, CB_PARTICLE, g::SKILL_HITMARK, g::SKILL_HITMARK);
        // the buffs go before Grace of Kenosis is snapshot
        for i in 0..s.chars.len() {
            s.delete_mod(i, SKILL_BUFF);
        }
        if s.chars[ci].ascension >= 1 {
            for i in 0..s.chars.len() {
                s.delete_mod(i, A1);
            }
        }
        if s.chars[ci].cons >= 2 {
            for i in 0..s.chars.len() {
                s.delete_mod(i, C2);
            }
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_BUFF, arg: 0 }, g::SKILL_HITMARK - 1);
        s.set_cd(ci, Action::Skill, 16 * 60);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.delete_mod(ci, BURST);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BURST_INITIAL[s.talent(ci, 2)]);
        ai.abil = "Revelation: Ladder of Divine Ascent";
        s.consume_energy(ci, 12);
        s.set_cd(ci, Action::Burst, 15 * 60);
        ai.pattern = s.circle_on_target(6.5);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_STATE, arg: 0 }, g::BURST_HITMARK + 1);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL_BUFF => {
                let lvl = s.talent(ci, 1);
                // addShield
                let hp = g::SKILL_SHIELD_ATK[lvl] * s.total_atk(ci) + g::SKILL_SHIELD_FLAT[lvl];
                let f = s.f;
                s.add_shield(Shield { owner: ci as u8, key: SHIELD, typ: ShieldType::NicoleSkill, target: -1, ele: Element::Pyro, hp, expiry: f + 20 * 60, em_bonus: 0.0 });
                // SelectStat(true, BaseATK, ATKP, ATK).TotalATK()
                let atk = total_atk(&s.select_stats(ci, [st::BASEATK, st::ATKP, st::ATK], true));
                let buff = (atk * g::SKILL_BUFF_RATIO[lvl]).min(g::SKILL_BUFF_MAX[lvl]);
                let cons = s.chars[ci].cons;
                for i in 0..s.chars.len() {
                    s.add_mod(i, Mod::stat(SKILL_BUFF, st::ATK, buff, true, false, -1), g::SKILL_DUR);
                    // c2OnSkillAddBuff, inside gcsim's loop
                    if cons >= 2 {
                        for j in 0..s.chars.len() {
                            s.add_mod(j, Mod::stat(C2, st::ATK, 300.0, false, false, -1), g::SKILL_DUR);
                        }
                    }
                }
            }
            TASK_BURST_STATE => {
                s.st_mut::<State>(ci).projections = 0;
                s.add_status(ci, BURST, 20 * 60, true);
            }
            TASK_PROJECTION => {
                let who = arg as usize;
                let mut ai = AttackInfo::new(who, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, s.chars[who].element, 0.0, g::BURST_PROJECTION[s.talent(ci, 2)]);
                ai.abil = "Arcane Projection";
                ai.flat_dmg = hexerei_on_projection(s, ci, who);
                ai.pattern = projection_pattern(s, who);
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_UNITY => {
                let who = arg as usize;
                let mut ai = AttackInfo::new(who, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, s.chars[who].element, 0.0, 6.0);
                ai.abil = "Arcane Projection: Unity";
                ai.flat_dmg = hexerei_on_projection(s, ci, who);
                ai.pattern = s.circle_on_target(4.5);
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_A1_SWAP => {
                let (src, who) = (arg >> 2, (arg & 3) as usize);
                if s.st::<State>(ci).a1_src != src {
                    return;
                }
                a1_upgrade(s, ci, who, -1);
            }
            TASK_C2_TICK => c2_ticker(s, ci, (arg & 3) as usize, arg >> 2),
            TASK_CHARGE => {
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::CHARGE[s.talent(ci, 0)]);
                ai.abil = "Charge";
                ai.pattern = s.circle_toward_target(0.0, 0.0, 3.0);
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
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyHit(ev) => {
                burst_hook(s, ci, ev);
                a4_hook(s, ci, ev);
                c1_hook(s, ci, ev);
                c4_hook(s, ci, ev);
                c6_hook(s, ci, ev);
            }
            // a1Init's OnCharacterSwap
            Event::Swap { prev, next } => {
                if prev != ci && s.chars[ci].cons < 6 {
                    s.delete_mod(prev, A1);
                }
                let src = s.f;
                s.st_mut::<State>(ci).a1_src = src;
                let delay = if s.chars[next].hexerei { 0 } else { 3 * 60 };
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1_SWAP, arg: src << 2 | next as i32 }, delay);
            }
            _ => {}
        }
    }
}

/// gcsim's projectionHitbox for the member, around the enemy.
fn projection_pattern(s: &Sim, who: usize) -> Pattern {
    let melee = (-1.5, 2.5);
    match crate::gcsl::char_name(s.chars[who].key) {
        "albedo" => s.box_on_enemy(melee.0, melee.1, 3.0, 8.0),
        "razor" | "sucrose" => s.circle_on_enemy(melee.0, melee.1, 4.5),
        "klee" => {
            // meleeOffset.Sub(norm.Scale(1.5)).Add(norm.Perp().Scale(0.5))
            let d = (melee.0 * melee.0 + melee.1 * melee.1).sqrt();
            let (nx, ny) = (melee.0 / d, melee.1 / d);
            let (px, py) = (-ny, nx);
            s.circle_on_enemy(melee.0 - nx * 1.5 + px * 0.5, melee.1 - ny * 1.5 + py * 0.5, 4.5)
        }
        _ => s.circle_on_enemy(-2.5, 2.5, 4.5),
    }
}

/// Her Hexerei bonus: with two Hexerei members, a Hexerei member's
/// projections add 300% of Nicole's ATK.
fn hexerei_on_projection(s: &Sim, ci: usize, who: usize) -> f64 {
    if !s.chars[ci].hexerei || s.hexerei_count() < 2 || !s.chars[who].hexerei {
        return 0.0;
    }
    s.total_atk(ci) * 3.0
}

/// gcsim's nicole-burst-hook: the active member's hit, once every 3 s,
/// four times while Silent Contemplation lasts, sends a projection.
fn burst_hook(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    let actor = ev.info.actor as usize;
    if actor != s.player.active as usize || s.st::<State>(ci).projections >= 4 || !s.mod_active(ci, BURST) || s.mod_active(ci, BURST_ICD) {
        return;
    }
    s.add_status(ci, BURST_ICD, 3 * 60, true);
    s.st_mut::<State>(ci).projections += 1;
    s.add_task(Task::Char { c: ci as u8, id: TASK_PROJECTION, arg: actor as i32 }, g::PROJECTION_HITMARK);
}

/// A4: the active member's elemental hit upgrades Nicole's own buff, for
/// 8 s (as long as the buff from C6).
fn a4_hook(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    if ev.info.actor as usize != s.player.active as usize {
        return;
    }
    if !matches!(ev.info.element, Element::Pyro | Element::Hydro | Element::Electro | Element::Cryo | Element::Anemo | Element::Geo | Element::Dendro) {
        return;
    }
    let dur = if s.chars[ci].cons < 6 { 8 * 60 } else { -1 };
    a1_upgrade(s, ci, ci, dur);
}

/// C1: the active member's hit, once every 6 s, sends Arcane Projection:
/// Unity.
fn c1_hook(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    let actor = ev.info.actor as usize;
    if s.chars[ci].cons < 1 || actor != s.player.active as usize || s.mod_active(ci, C1_ICD) {
        return;
    }
    s.add_status(ci, C1_ICD, 6 * 60, true);
    s.add_task(Task::Char { c: ci as u8, id: TASK_UNITY, arg: actor as i32 }, g::PROJECTION_HITMARK);
}

/// C4: a member's upgraded hits add 70% of Nicole's ATK, eight times.
fn c4_hook(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    if s.chars[ci].cons < 4 {
        return;
    }
    if !matches!(ev.info.tag, AttackTag::ElementalBurst | AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
        return;
    }
    let actor = ev.info.actor as usize;
    if !s.mod_active(actor, C4) || s.st::<State>(ci).c4_tags[actor] <= 0 {
        return;
    }
    let amt = 0.7 * s.total_atk(ci);
    s.st_mut::<State>(ci).c4_tags[actor] -= 1;
    s.hit_flat += amt;
}

/// C6: an upgraded member's hits ignore 40% of DEF.
fn c6_hook(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    if s.chars[ci].cons < 6 || !s.mod_active(ev.info.actor as usize, A1) {
        return;
    }
    s.hit_ignore_def += 0.4;
}

/// gcsim's a1UpgradeBuff: Guidance of Theosis (ATK +300) on a member for
/// what is left of its Grace of Kenosis (at most dur when dur >= 0), then
/// C2's, C4's and C6's parts.
fn a1_upgrade(s: &mut Sim, ci: usize, who: usize, dur: i32) {
    let left = s.mod_duration(who, g::SKILL_BUFF_KEY);
    let dur = if dur < 0 { left } else { dur.min(left) };
    if dur == 0 {
        return;
    }
    // gcsim's a1Buff is made only from A1 (A4 runs before it with no
    // amount)
    let amt = if s.chars[ci].ascension >= 1 { 300.0 } else { 0.0 };
    s.add_mod(who, Mod::stat(A1, st::ATK, amt, false, false, -1), dur);
    let cons = s.chars[ci].cons;
    // c2OnUpgrade
    if cons >= 2 && s.mod_active(who, A1) {
        let src = s.f;
        s.st_mut::<State>(ci).c2_tags[who] = src;
        c2_ticker(s, ci, who, src);
    }
    // c4OnUpgrade
    if cons >= 4 && !s.mod_active(who, C4_ICD) {
        s.add_status(who, C4_ICD, 16 * 60, true);
        s.add_status(who, C4, 20 * 60, true);
        s.st_mut::<State>(ci).c4_tags[who] = 8;
    }
    // c6OnUpgrade: Nicole's own upgrade upgrades the others
    if cons >= 6 && who == ci {
        for i in 0..s.chars.len() {
            if i != ci {
                a1_upgrade(s, ci, i, -1);
            }
        }
    }
}

/// gcsim's c2Ticker: while a member is upgraded, the enemy within 10 of
/// the player loses 25% of that member's element's resistance, refreshed
/// every 0.3 s.
fn c2_ticker(s: &mut Sim, ci: usize, who: usize, src: i32) {
    if !s.mod_active(who, A1) || s.st::<State>(ci).c2_tags[who] != src {
        return;
    }
    if s.enemy_within(s.circle_on_player(0.0, 0.0, 10.0)) {
        let el = s.chars[who].element;
        s.enemy_add_resist(c2_key(el), el, -0.25, 60);
    }
    s.add_task(Task::Char { c: ci as u8, id: TASK_C2_TICK, arg: src << 2 | who as i32 }, 18);
}

/// gcsim's "nicole-c2-<element>" resist mods.
fn c2_key(e: Element) -> ModKey {
    match e {
        Element::Pyro => ModKey::new("nicole-c2-pyro"),
        Element::Hydro => ModKey::new("nicole-c2-hydro"),
        Element::Electro => ModKey::new("nicole-c2-electro"),
        Element::Cryo => ModKey::new("nicole-c2-cryo"),
        Element::Anemo => ModKey::new("nicole-c2-anemo"),
        Element::Geo => ModKey::new("nicole-c2-geo"),
        Element::Dendro => ModKey::new("nicole-c2-dendro"),
        _ => ModKey::new("nicole-c2-physical"),
    }
}
