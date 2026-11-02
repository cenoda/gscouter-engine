//! Layla, from gcsim's internal/characters/layla: normal and charged
//! attacks, the skill's Curtain of Slumber (its shield, the Night Stars it
//! gathers over time, from any character's skill and from the burst's
//! slugs, and the Shooting Stars it fires at four), the burst (param
//! travel), A1, A4, every constellation, and her tags (nightstars,
//! shooting-stars) as conditions.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::layla as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    a1_stack: i32,
    particle_switch: bool,
    star_tick_src: i32,
    shoot_star_src: i32,
    /// gcsim's tags nightstars and shooting-stars.
    night_stars: i32,
    shooting: i32,
    /// The burst's slug and the area it looks for an enemy in, from the
    /// cast.
    burst_ai: AttackInfo,
    burst_area: Pattern,
    burst_travel: i32,
}

const SHIELD: ModKey = ModKey::new("layla-shield");
const STAR_SKILL_ICD: ModKey = ModKey::new(g::STAR_SKILL_ICD);
const STAR_BURST_ICD: ModKey = ModKey::new(g::STAR_BURST_ICD);
const PARTICLE_ICD1: ModKey = ModKey::new(g::PARTICLE_ICD1_KEY);
const PARTICLE_ICD2: ModKey = ModKey::new(g::PARTICLE_ICD2_KEY);
const LAYLA_BURST: ModKey = ModKey::new("laylaburst");
const A1: ModKey = ModKey::new("layla-a1");
const C4: ModKey = ModKey::new(g::C4_KEY);
const C6: ModKey = ModKey::new("layla-c6");

const CB_C2: u16 = 1 << 0;
const CB_PARTICLE1: u16 = 1 << 1;
const CB_PARTICLE2: u16 = 1 << 2;
const CB_SLUG: u16 = 1 << 3;

const TASK_SHIELD: u8 = 0;
/// arg: src * 2 + star.
const TASK_TICK: u8 = 1;
/// arg: src * 4 + has_last * 2 + particle key.
const TASK_SHOOT: u8 = 2;
const TASK_SLUG: u8 = 3;
/// arg: the character whose Dawn Star ends.
const TASK_C4_END: u8 = 4;

const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];

#[derive(Clone, Copy, PartialEq)]
enum StarIcd {
    Skill,
    Burst,
}

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn has_shield(s: &Sim) -> bool {
    s.shields.iter().any(|x| x.typ == ShieldType::LaylaSkill)
}

/// gcsim's removeShield.
fn remove_shield(s: &mut Sim, ci: usize) {
    let x = st_mut(s, ci);
    if x.shooting == 0 {
        x.night_stars = 0;
    }
    x.a1_stack = 0;
}

/// A4: the Shooting Stars' flat damage.
fn a4(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 4 {
        return 0.0;
    }
    0.015 * s.max_hp(ci)
}

/// gcsim's addNightStars.
fn add_night_stars(s: &mut Sim, ci: usize, count: i32, icd: StarIcd) {
    if st(s, ci).shooting > 0 {
        return;
    }
    let (key, dur) = match icd {
        StarIcd::Skill => (STAR_SKILL_ICD, 18),
        StarIcd::Burst => (STAR_BURST_ICD, 30),
    };
    if s.mod_active(ci, key) {
        return;
    }
    s.add_status(ci, key, dur, false);
    let x = st_mut(s, ci);
    if x.a1_stack < 4 {
        x.a1_stack += 1;
    }
    let stars = (count + x.night_stars).min(4);
    x.night_stars = stars;
    if stars == 4 && x.shooting == 0 {
        x.shooting = 1;
        let f = s.f;
        st_mut(s, ci).shoot_star_src = f;
        // makeParticleCB: the two particle ICDs take turns, volley by volley
        let x = st_mut(s, ci);
        let key = x.particle_switch as i32;
        x.particle_switch = !x.particle_switch;
        s.add_task(Task::Char { c: ci as u8, id: TASK_SHOOT, arg: f * 4 + key }, 6);
    }
}

/// gcsim's shootStars.
fn shoot_stars(s: &mut Sim, ci: usize, arg: i32) {
    let (src, has_last, key) = (arg >> 2, arg & 2 != 0, arg & 1);
    if st(s, ci).shoot_star_src != src || st(s, ci).shooting == 0 {
        return;
    }
    let area = s.circle_on_player(0.0, 0.0, 10.0);
    let found = s.enemy_alive && s.enemy_within(area);
    if !has_last {
        if !found {
            s.add_task(Task::Char { c: ci as u8, id: TASK_SHOOT, arg }, 6);
            return;
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_SHOOT, arg: src * 4 + 2 + key }, 30);
        return;
    }
    // the enemy, or the last one shot at
    let stars = st(s, ci).night_stars;
    if stars == 4 && s.chars[ci].cons >= 4 {
        for i in 0..s.chars.len() {
            s.add_status(i, C4, 180, true);
        }
    }
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_LAYLA, Element::Cryo, 25.0, g::STAR_DMG[s.talent(ci, 1)]);
    ai.abil = g::SHOOTING_STARS_ABIL;
    ai.flat_dmg = a4(s, ci);
    ai.pattern = s.circle_on_target(0.8);
    let mut cbs = if key == 0 { CB_PARTICLE1 } else { CB_PARTICLE2 };
    if s.chars[ci].cons >= 2 {
        cbs |= CB_C2;
    }
    let travel = g::STARS_TRAVEL[g::STARS_TRAVEL.len() - stars as usize];
    s.queue_attack(ai, cbs, 0, travel);
    let stars = stars - 1;
    st_mut(s, ci).night_stars = stars;
    if stars != 0 {
        s.add_task(Task::Char { c: ci as u8, id: TASK_SHOOT, arg }, 27);
        return;
    }
    st_mut(s, ci).shooting = 0;
    let f = s.f;
    st_mut(s, ci).star_tick_src = f;
    tick_night_star(s, ci, f, false);
}

/// gcsim's tickNightStar.
fn tick_night_star(s: &mut Sim, ci: usize, src: i32, star: bool) {
    if st(s, ci).star_tick_src != src || !has_shield(s) {
        return;
    }
    if star {
        add_night_stars(s, ci, 1, StarIcd::Skill);
    }
    let interval = if s.chars[ci].cons >= 6 { (1.5 * 0.8 * 60.0) as i32 } else { 90 };
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_TICK, arg: src * 2 + 1 }, interval);
}

impl Character for Char {
    const KEY: &'static str = "layla";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            a1_stack: 0,
            particle_switch: false,
            star_tick_src: -1,
            shoot_star_src: -1,
            night_stars: 0,
            shooting: 0,
            burst_ai: AttackInfo::new(0, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 0.0, 0.0),
            burst_area: Pattern::Circle { x: 0.0, y: 0.0, r: 0.0 },
            burst_travel: 0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        s.subscribe(EventKind::Skill, o);
        if s.chars[ci].ascension >= 1 {
            s.shield_bonus_mods.push(ShieldBonusMod { key: A1, owner: o, expiry: -1 });
        }
        let cons = s.chars[ci].cons;
        if cons >= 4 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        if cons >= 6 {
            s.add_mod(ci, Mod::owned(C6, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Burst => p.0.iter().find(|(k, _)| k != "travel").map(|(k, _)| format!("{name} param {k}")),
            Action::Attack | Action::Charge | Action::Skill => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match (fields[0].as_str(), fields.get(1).map(|f| f.as_str())) {
            ("tags", Some(g::NIGHT_STARS)) => Some(Ok(Num::Int(st(s, ci).night_stars as i64))),
            ("tags", Some(g::SHOOTING_STARS)) => Some(Ok(Num::Int(st(s, ci).shooting as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::AUTO[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        let (off, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
        ai.pattern = if n == 1 || n == 2 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::CHARGE.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = CHARGE_NAMES[i];
            if i == 1 {
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = 0.06 * 60.0;
                ai.def_halt = true;
            }
            ai.pattern = s.circle_on_player(0.0, 0.3, 2.8);
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL[s.talent(ci, 1)]);
        ai.abil = "Nights of Formal Focus";
        ai.pattern = s.circle_on_player(0.0, 0.0, 2.5);
        s.queue_attack(ai, 0, 0, g::SKILL_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SHIELD, arg: 0 }, 19);
        s.set_cd_with_delay(ci, Action::Skill, 12 * 60, 19);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(g::TICK_TRAVEL, |v| v as i32);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 0.0);
        ai.abil = "Starlight Slug";
        ai.flat_dmg = g::BURST[s.talent(ci, 2)] * s.max_hp(ci);
        s.add_core_status(LAYLA_BURST, 12 * 60 + g::BURST_START);
        let area = s.circle_on_player(0.0, 1.0, 12.0);
        let x = st_mut(s, ci);
        x.burst_ai = ai;
        x.burst_area = area;
        x.burst_travel = travel;
        let mut delay = g::BURST_START;
        while delay < 12 * 60 + g::BURST_START {
            s.add_task(Task::Char { c: ci as u8, id: TASK_SLUG, arg: 0 }, delay);
            delay += 90;
        }
        s.set_cd(ci, Action::Burst, 12 * 60);
        s.consume_energy(ci, 6);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SHIELD => {
                let f = s.f;
                if has_shield(s) {
                    s.set_shield_expiry(ShieldType::LaylaSkill, f + 12 * 60);
                } else {
                    let lvl = s.talent(ci, 1);
                    let mut hp = g::SHIELD_BASE[lvl] + g::SHIELD_PER[lvl] * s.max_hp(ci);
                    if s.chars[ci].cons >= 1 {
                        hp *= 1.2;
                    }
                    s.add_shield(Shield { owner: ci as u8, key: SHIELD, typ: ShieldType::LaylaSkill, target: -1, ele: Element::Cryo, hp, expiry: f + 12 * 60, em_bonus: 0.0 });
                }
                s.apply_self_infusion(AURA_CRYO, 25.0, 6);
                st_mut(s, ci).star_tick_src = f;
                tick_night_star(s, ci, f, false);
            }
            TASK_TICK => tick_night_star(s, ci, arg >> 1, arg & 1 != 0),
            TASK_SHOOT => shoot_stars(s, ci, arg),
            TASK_SLUG => {
                if !s.enemy_alive || !s.enemy_within(st(s, ci).burst_area) {
                    return;
                }
                let mut ai = st(s, ci).burst_ai;
                ai.pattern = s.circle_on_target(1.5);
                let travel = st(s, ci).burst_travel;
                s.queue_attack(ai, CB_SLUG, g::TICK_RELEASE, g::TICK_RELEASE + travel);
            }
            TASK_C4_END => s.delete_mod(arg as usize, C4),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_C2 != 0 {
            s.add_energy(ci, 1.0);
        }
        for (bit, key) in [(CB_PARTICLE1, PARTICLE_ICD1), (CB_PARTICLE2, PARTICLE_ICD2)] {
            if cbs & bit != 0 && !s.mod_active(ci, key) {
                s.add_status(ci, key, 210, false);
                let count = if s.rng.float64() < 0.33 { 2.0 } else { 1.0 };
                s.queue_particle(count, Element::Cryo, s.particle_delay[ci]);
            }
        }
        if cbs & CB_SLUG != 0 && has_shield(s) {
            add_night_stars(s, ci, 1, StarIcd::Burst);
        }
    }

    /// C6: the Shooting Stars and the burst's slugs.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::ElementalBurst || ev.info.abil == g::SHOOTING_STARS_ABIL {
            ev.stats[st::DMGP] += 0.4;
        }
    }

    fn shield_bonus(s: &Sim, ci: usize, _: ModKey) -> (f64, bool) {
        if !has_shield(s) {
            return (0.0, false);
        }
        (st(s, ci).a1_stack as f64 * 0.06, false)
    }

    fn shield_damaged(s: &mut Sim, ci: usize, _: ModKey, _: f64, broken: bool) {
        if broken {
            remove_shield(s, ci);
        }
    }

    fn shield_expired(s: &mut Sim, ci: usize, _: ModKey) {
        remove_shield(s, ci);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // any skill with the Curtain up: two Night Stars
            Event::Skill => {
                if has_shield(s) {
                    add_night_stars(s, ci, 2, StarIcd::Skill);
                }
            }
            // C4: Dawn Star's flat damage on a normal or charged attack,
            // removed 3 frames later
            Event::EnemyHit(ev) => {
                if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
                    return;
                }
                let a = ev.info.actor as usize;
                if !s.mod_active(a, C4) {
                    return;
                }
                s.hit_flat += 0.05 * s.max_hp(ci);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C4_END, arg: a as i32 }, 3);
            }
            _ => {}
        }
    }
}
