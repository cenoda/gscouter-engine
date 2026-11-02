//! Columbina, from gcsim's internal/characters/columbina: normal attacks,
//! the charged attack and, with dew, Moondew Cleanse (direct Lunar-Bloom,
//! a dew spent), the skill and its ticks with Gravity (gathered from lunar
//! reactions and lunar damage while the skill lasts; at 60 a Gravity
//! Interference of the reaction gathered most), the burst's Lunar Domain
//! (its lunar reaction bonus to characters inside), her jump, A1, A4, C1
//! to C6 and the conditions .columbina.gravity and .columbina.a1-stacks.
//! She is Moonsign, enables Lunar-Charged, Lunar-Bloom and
//! Lunar-Crystallize for the fight, and raises the base DMG of every
//! lunar hit with her max HP. gcsim gives her no plunges: an error.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::columbina as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

const LC: usize = 0;
const LB: usize = 1;
const LCR: usize = 2;

pub struct State {
    skill_src: i32,
    gravity: [f64; 3],
    gravity_task: bool,
    /// gcsim's gravityLastReaction: LC, LB, LCR, or none (-1).
    gravity_last: i32,
    burst_src: i32,
    burst_area: Pattern,
    a1_stacks: i32,
    a4_moondew_count: i32,
    /// The normal attack in progress: its table, ATK SPD and windup.
    attack_n: usize,
    attack_atkspd: f64,
}

const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const GRAVITY: ModKey = ModKey::new(g::GRAVITY_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const BURST_BUFF: ModKey = ModKey::new(g::BURST_BUFF_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const MOONDEW_ICD: ModKey = ModKey::new(g::MOONRIDGE_DEW_ICD_KEY);
const MOONDEW_TIMER: ModKey = ModKey::new(g::MOONRIDGE_DEW_TIMER_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C1_SHIELD: ModKey = ModKey::new("columbina-c1-shield");
const C2: ModKey = ModKey::new(g::C2_KEY);
const C2_LC: ModKey = ModKey::new(g::C2_LC_KEY);
const C2_LCR: ModKey = ModKey::new(g::C2_L_CR_KEY);
const C2_LB: ModKey = ModKey::new(g::C2_LB_KEY);
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const C6_LC: ModKey = ModKey::new(g::C6_LC_KEY);
const C6_LB: ModKey = ModKey::new(g::C6_LB_KEY);
const C6_LCR: ModKey = ModKey::new(g::C6_L_CR_KEY);

const CB_PARTICLE: u16 = 1 << 0;

const TASK_SKILL_HIT: u8 = 0;
const TASK_SKILL_TICK: u8 = 1;
const TASK_GRAVITY_ACCUM: u8 = 2;
/// arg: the reaction.
const TASK_GRAVITY_HIT: u8 = 3;
const TASK_MOONDEW_RESET: u8 = 4;
const TASK_BURST_FIELD: u8 = 5;
/// arg: the burst's src.
const TASK_BURST_TICK: u8 = 6;
const TASK_CONSUME_DEW: u8 = 7;

const FRAMES_ATTACK: u8 = 0;
const FRAMES_CHARGE_LB: u8 = 1;

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn moonsign_level(s: &Sim) -> i32 {
    s.chars.iter().map(|c| c.moonsign).sum()
}

/// Her base DMG bonus on lunar hits: 0.2% per 1,000 max HP, at most 7%.
fn lunar_bonus(s: &Sim, ci: usize) -> f64 {
    (s.max_hp(ci) / 1000.0 * 0.002).min(0.07)
}

fn total_gravity(s: &Sim, ci: usize) -> f64 {
    st(s, ci).gravity.iter().sum()
}

/// gcsim's gravityAccum: 10 Gravity a second (C2: 34% more) of the last
/// reaction, every 3 frames while Gravity and the skill last.
fn gravity_accum(s: &mut Sim, ci: usize) {
    if !s.mod_active(ci, GRAVITY) || !s.mod_active(ci, SKILL) {
        st_mut(s, ci).gravity_task = false;
        return;
    }
    let c2 = if s.chars[ci].cons >= 2 { 0.34 } else { 0.0 };
    let x = st_mut(s, ci);
    x.gravity_task = true;
    let amt = 10.0 * 0.05 * (1.0 + c2);
    if x.gravity_last >= 0 {
        x.gravity[x.gravity_last as usize] += amt;
    }
    if total_gravity(s, ci) >= g::GRAVITY_MAX as f64 {
        gravity_tick(s, ci, true);
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_GRAVITY_ACCUM, arg: 0 }, 3);
}

/// Gravity from a lunar reaction or lunar damage while the skill lasts.
fn gravity_hook(s: &mut Sim, ci: usize, reaction: Option<usize>) {
    if let Some(r) = reaction {
        st_mut(s, ci).gravity_last = r as i32;
    }
    s.add_status(ci, GRAVITY, 120, false);
    if !st(s, ci).gravity_task {
        gravity_accum(s, ci);
    }
}

/// gcsim's gravityTick: Gravity Interference of the reaction with the most
/// Gravity (ties to the earlier), or by default of the element most of the
/// team has (electro, dendro, geo; ties to the earlier).
fn gravity_tick(s: &mut Sim, ci: usize, clear: bool) {
    let count = |e: Element| s.chars.iter().filter(|c| c.element == e).count();
    let counts = [count(Element::Electro), count(Element::Dendro), count(Element::Geo)];
    let mut reaction = 0;
    for i in 1..3 {
        if counts[i] > counts[reaction] {
            reaction = i;
        }
    }
    let mut max = 0.0;
    for (i, &gv) in st(s, ci).gravity.iter().enumerate() {
        if gv > max {
            max = gv;
            reaction = i;
        }
    }
    if clear {
        st_mut(s, ci).gravity = [0.0; 3];
    }
    // A1: CRIT Rate stacks
    if s.chars[ci].ascension >= 1 {
        let stacks = if !s.mod_active(ci, A1) { 1 } else { (st(s, ci).a1_stacks + 1).min(3) };
        st_mut(s, ci).a1_stacks = stacks;
        s.add_mod(ci, Mod::owned(A1, ModKind::Stat, Owner::Char(ci as u8), st::CR, false, true, -1), 10 * 60);
    }
    c1_on_gravity_tick(s, ci, reaction);
    c2_on_gravity_tick(s, ci, reaction);
    let hitmarks: &[i32] = match reaction {
        LC => &[g::SKILL_HITMARKS_LC],
        LB => g::SKILL_HITMARKS_LB,
        _ => &[g::SKILL_HITMARKS_L_CR],
    };
    for &d in hitmarks {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_GRAVITY_HIT, arg: reaction as i32 }, d);
    }
}

fn gravity_hit(s: &mut Sim, ci: usize, reaction: usize) {
    let lvl = s.talent(ci, 1);
    let (mult, tag, ele, abil, radius, travel) = match reaction {
        LC => (g::SKILL_LC[lvl], AttackTag::DirectLunarCharged, Element::Electro, "Gravity Interference (Lunar-Charged)", 6.0, 0),
        LB => (g::SKILL_LB[lvl], AttackTag::DirectLunarBloom, Element::Dendro, "Gravity Interference (Lunar-Bloom)", 0.5, g::SKILL_LB_TRAVEL),
        _ => (g::SKILL_L_CR[lvl], AttackTag::DirectLunarCrystallize, Element::Geo, "Gravity Interference (Lunar-Crystallize)", 6.0, 0),
    };
    let mut ai = AttackInfo::new(ci, tag, icd::TAG_NONE, icd::GROUP_DEFAULT, ele, 0.0, mult);
    ai.abil = abil;
    ai.use_hp = true;
    ai.ignore_def = 1.0;
    ai.pattern = s.circle_on_target(radius);
    ai.flat_dmg = c4_flat(s, ci, reaction);
    s.queue_attack(ai, 0, 0, travel);
}

/// C1: on a Gravity Interference, 6 energy to the active character
/// (Lunar-Charged) or a shield on her (Lunar-Crystallize).
fn c1_on_gravity_tick(s: &mut Sim, ci: usize, reaction: usize) {
    if s.chars[ci].cons < 1 {
        return;
    }
    match reaction {
        LC => {
            let a = s.player.active as usize;
            s.add_energy(a, 6.0);
        }
        LCR => {
            let hp = 0.12 * s.max_hp(ci);
            let f = s.f;
            s.add_shield(Shield { owner: ci as u8, key: C1_SHIELD, typ: ShieldType::ColumbinaC1, target: ci as i8, ele: Element::Hydro, hp, expiry: f + 8 * 60, em_bonus: 0.0 });
        }
        _ => {}
    }
}

/// C2: her max HP, and with Moonsign 2 or more a buff on the active
/// character from her max HP by the reaction.
fn c2_on_gravity_tick(s: &mut Sim, ci: usize, reaction: usize) {
    if s.chars[ci].cons < 2 {
        return;
    }
    s.add_stat_mod(ci, C2, st::HPP, 0.4, 8 * 60, false);
    if moonsign_level(s) < 2 {
        return;
    }
    let (key, stat, others) = match reaction {
        LC => (C2_LC, st::ATK, [C2_LCR, C2_LB]),
        LCR => (C2_LCR, st::DEF, [C2_LC, C2_LB]),
        _ => (C2_LB, st::EM, [C2_LC, C2_LCR]),
    };
    for i in 0..s.chars.len() {
        for k in others {
            s.delete_mod(i, k);
        }
        s.add_mod(i, Mod::owned(key, ModKind::Stat, Owner::Char(ci as u8), stat, false, false, -1), 8 * 60);
    }
}

/// C4: 4 energy on every Gravity Interference hit, and once every 15 s
/// flat damage from her max HP.
fn c4_flat(s: &mut Sim, ci: usize, reaction: usize) -> f64 {
    if s.chars[ci].cons < 4 {
        return 0.0;
    }
    s.add_energy(ci, 4.0);
    if s.mod_active(ci, C4_ICD) {
        return 0.0;
    }
    s.add_status(ci, C4_ICD, 15 * 60, true);
    match reaction {
        LB => 0.025 * s.max_hp(ci),
        _ => 0.125 * s.max_hp(ci),
    }
}

fn charge_windup(s: &Sim, dew: bool) -> i32 {
    match (s.current_state(), dew) {
        (AnimState::Charge, _) => 3,
        (AnimState::Normal, false) => -2,
        (AnimState::Normal, true) => -5,
        (_, false) => 15,
        (_, true) => 12,
    }
}

impl Character for Char {
    const KEY: &'static str = "columbina";
    const MOONSIGN: i32 = 1;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            skill_src: 0,
            gravity: [0.0; 3],
            gravity_task: false,
            gravity_last: -1,
            burst_src: 0,
            burst_area: Pattern::Circle { x: 0.0, y: 0.0, r: 0.0 },
            a1_stacks: 0,
            a4_moondew_count: 0,
            attack_n: 0,
            attack_atkspd: 0.0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // skillInit: Gravity from lunar reactions and lunar damage
        s.subscribe(EventKind::Reaction, o);
        s.subscribe(EventKind::EnemyDamage, o);
        // moonsignInit
        s.enable_lunar_charge();
        s.enable_lunar_bloom_crystallize();
        s.subscribe(EventKind::EnemyHit, o);
        s.subscribe(EventKind::SpecialReactionAttack, o);
        let cons = s.chars[ci].cons;
        // consElevationInit (the direct part in Sim::apply_attack)
        if cons >= 1 {
            s.lunar_direct_elevation += (g::ELEVATION[cons as usize] * 1000.0).round() as u16;
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::Jump && s.mod_active(ci, XIANYUN_AIRBORNE) {
            return Some("xianyun jump not implemented".into());
        }
        crate::content::template_invalid(s, a)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "gravity" => Some(Ok(Num::Float(total_gravity(s, ci)))),
            "a1-stacks" => Some(Ok(Num::Int(if s.mod_active(ci, A1) { st(s, ci).a1_stacks as i64 } else { 0 }))),
            _ => None,
        }
    }

    fn action_stam(s: &Sim, _: usize, a: Action) -> Option<f64> {
        (a == Action::Charge && s.dew() > 0).then_some(0.0)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let windup = if n == 0 && matches!(s.current_state(), AnimState::Normal | AnimState::Charge) { 2 } else { 0 };
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::ATTACK[n][s.talent(ci, 0)]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.circle_on_target(g::ATTACK_RADIUS[n]);
        let h = g::ATTACK_HITMARKS[n] + windup;
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let x = st_mut(s, ci);
        x.attack_n = n;
        x.attack_atkspd = atkspd;
        let t = &g::ATTACK_FRAMES[n];
        s.advance_normal(ci);
        ActInfo::new(Frames::CharLive { id: FRAMES_ATTACK, arg: windup as i16 }, t[0] + windup, h, AnimState::Normal)
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        if s.dew() > 0 {
            // Moondew Cleanse: direct Lunar-Bloom, a dew spent
            let windup = charge_windup(s, true);
            let mut ai = AttackInfo::new(ci, AttackTag::DirectLunarBloom, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 0.0, g::CHARGE_LB[lvl]);
            ai.abil = "Moondew Cleanse";
            ai.use_hp = true;
            ai.ignore_def = 1.0;
            ai.pattern = s.circle_on_target(g::CHARGE_RADIUS_BLOOM);
            for &h in g::CHARGE_HITMARK_BLOOM {
                s.queue_attack(ai, 0, h + windup, h + windup);
            }
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CONSUME_DEW, arg: 0 }, g::VERDANT_DEW_FRAME + windup);
            let t = &g::CHARGE_LB_FRAMES;
            return ActInfo::new(Frames::CharLive { id: FRAMES_CHARGE_LB, arg: 0 }, t[0], t[Action::Dash as usize], AnimState::Charge);
        }
        let windup = charge_windup(s, false);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.pattern = s.circle_on_target(g::CHARGE_RADIUS);
        let d = g::CHARGE_HITMARK + windup;
        s.queue_attack(ai, 0, d, d);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Charge)
    }

    fn frames_live(s: &Sim, ci: usize, id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FRAMES_ATTACK => {
                let x = st(s, ci);
                atk_spd_adjust(g::ATTACK_FRAMES[x.attack_n][next as usize], x.attack_atkspd) + arg as i32
            }
            _ => {
                if next == Action::Charge && s.dew() <= 0 {
                    89
                } else {
                    g::CHARGE_LB_FRAMES[next as usize]
                }
            }
        }
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_HIT, arg: 0 }, g::SKILL_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 17 * 60, 20);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 50.0, g::BURST[s.talent(ci, 2)]);
        ai.abil = "Burst";
        ai.use_hp = true;
        ai.pattern = s.circle_on_player(0.0, 0.5, 6.5);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        let area = s.circle_on_player(0.0, 0.5, 20.0);
        st_mut(s, ci).burst_area = area;
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_FIELD, arg: 0 }, g::BURST_FIELD);
        s.consume_energy(ci, 6);
        s.set_cd(ci, Action::Burst, 15 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    /// Her jump, not the template's (gcsim's Jump override).
    fn own_jump(s: &mut Sim, _: usize, _: &Params) -> Option<ActInfo> {
        let f = if s.current_state() == AnimState::Dash { 52 } else { 48 };
        Some(ActInfo::new(Frames::Const(f), f, f, AnimState::Jump))
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL_HIT => {
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::SKILL[s.talent(ci, 1)]);
                ai.abil = "Skill";
                ai.use_hp = true;
                ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
                s.queue_attack(ai, 0, 0, 0);
                if !s.mod_active(ci, SKILL) {
                    st_mut(s, ci).gravity = [0.0; 3];
                    let f = s.f;
                    st_mut(s, ci).skill_src = f;
                    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_TICK, arg: f }, 117);
                }
                s.add_status(ci, SKILL, 25 * 60, true);
                // C1: a Gravity Interference on the skill, once every 15 s
                if s.chars[ci].cons >= 1 && !s.mod_active(ci, C1_ICD) {
                    gravity_tick(s, ci, false);
                    s.add_status(ci, C1_ICD, 15 * 60, true);
                }
            }
            TASK_SKILL_TICK => {
                if st(s, ci).skill_src != arg || !s.mod_active(ci, SKILL) {
                    return;
                }
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::SKILL_DO_T[s.talent(ci, 1)]);
                ai.abil = "Skill (DoT)";
                ai.use_hp = true;
                let r = if moonsign_level(s) >= 2 { 6.0 } else { 4.0 };
                ai.pattern = s.circle_on_player(0.0, 0.0, r);
                s.queue_attack(ai, CB_PARTICLE, 0, 0);
                s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_TICK, arg }, 117);
            }
            TASK_GRAVITY_ACCUM => gravity_accum(s, ci),
            TASK_GRAVITY_HIT => gravity_hit(s, ci, arg as usize),
            TASK_MOONDEW_RESET => st_mut(s, ci).a4_moondew_count = 0,
            TASK_BURST_FIELD => {
                let f = s.f;
                st_mut(s, ci).burst_src = f;
                s.add_core_status(BURST, g::BURST_DUR);
                let mut i = 0;
                while i <= g::BURST_DUR {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_TICK, arg: f }, i);
                    i += 48;
                }
            }
            TASK_BURST_TICK => {
                if st(s, ci).burst_src != arg || !s.player_within(st(s, ci).burst_area) {
                    return;
                }
                for i in 0..s.chars.len() {
                    s.add_mod(i, Mod::owned(BURST_BUFF, ModKind::ReactBonus, Owner::Char(ci as u8), st::NONE, false, true, -1), g::WITHIN_TIMER as i32);
                }
            }
            TASK_CONSUME_DEW => {
                s.consume_dew(1);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 3 * 60, false);
            let n = if s.rng.float64() < 0.67 { 1.0 } else { 2.0 };
            s.queue_particle(n, Element::Hydro, s.particle_delay[ci]);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool {
        match m.key {
            k if k == A1 => out[st::CR] += 0.05 * st(s, ci).a1_stacks as f64,
            // C2's buffs only for the active character
            k if k == C2_LC || k == C2_LCR || k == C2_LB => {
                if s.player.active as usize != on {
                    return false;
                }
                let hp = s.max_hp(ci);
                if k == C2_LC {
                    out[st::ATK] += 0.01 * hp;
                } else if k == C2_LCR {
                    out[st::DEF] += 0.01 * hp;
                } else {
                    out[st::EM] += 0.0035 * hp;
                }
            }
            _ => return false,
        }
        true
    }

    /// C6: CRIT DMG on the elements of the lunar reaction seen in her
    /// burst.
    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        let e = ev.info.element;
        let on = if m.key == C6_LC {
            matches!(e, Element::Electro | Element::Hydro)
        } else if m.key == C6_LB {
            matches!(e, Element::Dendro | Element::Hydro)
        } else {
            matches!(e, Element::Geo | Element::Hydro)
        };
        if on {
            ev.stats[st::CD] += 0.8;
        }
    }

    /// The burst's Lunar Domain: lunar reaction DMG.
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if !info.tag.is_lunar() {
            return 0.0;
        }
        g::BURST_BUFF[s.talent(ci, 2)]
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Reaction(r @ (Reaction::LunarCharged | Reaction::LunarBloom | Reaction::LunarCrystallize), ev) if ev.on_enemy() => {
                let idx = match r {
                    Reaction::LunarCharged => LC,
                    Reaction::LunarBloom => LB,
                    _ => LCR,
                };
                // skillInit's hooks, then A4, then C6, as she subscribes
                if s.mod_active(ci, SKILL) {
                    gravity_hook(s, ci, Some(idx));
                }
                if s.chars[ci].ascension >= 4 {
                    a4(s, ci, idx);
                }
                if s.chars[ci].cons >= 6 && s.mod_active(ci, BURST_BUFF) {
                    let key = [C6_LC, C6_LB, C6_LCR][idx];
                    for i in 0..s.chars.len() {
                        s.add_mod(i, Mod::owned(key, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 8 * 60);
                    }
                }
            }
            Event::EnemyDamage(ev) => {
                if !ev.info.tag.is_lunar() || !s.mod_active(ci, SKILL) {
                    return;
                }
                // Gravity is gathered before the reaction is noted
                s.add_status(ci, GRAVITY, 120, false);
                if !st(s, ci).gravity_task {
                    gravity_accum(s, ci);
                }
                let last = match ev.info.tag {
                    AttackTag::DirectLunarCharged | AttackTag::ReactionLunarCharge => LC as i32,
                    AttackTag::DirectLunarBloom => LB as i32,
                    AttackTag::DirectLunarCrystallize | AttackTag::ReactionLunarCrystallize => LCR as i32,
                    _ => st(s, ci).gravity_last,
                };
                st_mut(s, ci).gravity_last = last;
            }
            // her base DMG bonus on direct lunar hits
            Event::EnemyHit(ev) => {
                if ev.info.tag.is_lunar_direct() {
                    s.hit_base_bonus += lunar_bonus(s, ci);
                }
            }
            // and on lunar reaction contributions, with C1+'s elevation and
            // C6's CRIT DMG
            Event::SpecialReactionAttack(ev) => {
                if ev.info.tag.is_lunar_react() {
                    s.special_base_bonus += lunar_bonus(s, ci);
                }
                let cons = s.chars[ci].cons;
                if cons >= 1 && ev.info.tag.is_lunar_react() {
                    s.special_atk.elevation += (g::ELEVATION[cons as usize] * 1000.0).round() as u16;
                }
                if cons >= 6 {
                    let a = ev.info.actor as usize;
                    let add = match ev.info.element {
                        Element::Electro => s.mod_active(a, C6_LC),
                        Element::Hydro => s.mod_active(a, C6_LC) || s.mod_active(a, C6_LCR) || s.mod_active(a, C6_LB),
                        Element::Geo => s.mod_active(a, C6_LCR),
                        Element::Dendro => s.mod_active(a, C6_LB),
                        _ => false,
                    };
                    if add {
                        s.special_atk.stats[st::CD] += 0.8;
                    }
                }
            }
            _ => {}
        }
    }
}

/// A4: in her burst, a second Lunar-Charged or Lunar-Crystallize hit a
/// third of the time, and Moonridge Dew from Lunar-Bloom (3 per 18 s).
fn a4(s: &mut Sim, ci: usize, idx: usize) {
    let active = s.mod_active(ci, BURST_BUFF);
    match idx {
        LC | LCR => s.set_lunar_extra_hit(active.then_some(0.33)),
        _ => {
            if !active || s.mod_active(ci, MOONDEW_ICD) {
                return;
            }
            if st(s, ci).a4_moondew_count < 3 {
                s.add_status(ci, MOONDEW_ICD, 3, true);
                st_mut(s, ci).a4_moondew_count += 1;
                s.add_moonridge_dew();
                if !s.mod_active(ci, MOONDEW_TIMER) {
                    s.add_status(ci, MOONDEW_TIMER, 18 * 60, true);
                    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_MOONDEW_RESET, arg: 0 }, 18 * 60);
                }
            }
        }
    }
}
