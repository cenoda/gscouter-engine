//! Furina, from gcsim's internal/characters/furina: normal attacks with
//! their Arkhe strikes, the charged attack that switches Ousia and Pneuma,
//! the skill (the Salon Members, which drain the party, or the Singer,
//! which heals), the burst's fanfare from every HP change, A1, A4 and every
//! constellation. Plunges are not implemented.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::furina as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

/// Fanfare gains wait in a ring: a task carries an index into it.
const PENDING: usize = 64;

pub struct State {
    pneuma: bool,
    cur_fanfare: f64,
    max_q_fanfare: f64,
    max_c2_fanfare: f64,
    fanfare_debounce_task_queued: bool,
    pending: [f64; PENDING],
    next_pending: usize,
    a1_src: i32,
    a4_dmg: f64,
    a4_interval_reduction: f64,
    last_summon_src: i32,
    /// The flat damage of each Salon Member's attack in flight.
    salon_flat: [f64; 3],
    c6_count: i32,
    c6_heal_src: i32,
}

const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const FANFARE_DEBOUNCE: ModKey = ModKey::new(g::FANFARE_DEBOUNCE_KEY);
const BURST_DMG: ModKey = ModKey::new("furina-burst-damage-buff");
const BURST_HEAL: ModKey = ModKey::new("furina-burst-heal-buff");
const ARKHE_ICD: [ModKey; 2] = [ModKey::new("spiritbreath-thorn-icd"), ModKey::new("surging-blade-icd")];
const A1_HEAL: ModKey = ModKey::new(g::A1_HEAL_KEY);
const A4: ModKey = ModKey::new(g::A4_BUFF_KEY);
const C2: ModKey = ModKey::new(g::C2_BUFF_KEY);
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);
const C6_OUSIA_HEAL: ModKey = ModKey::new(g::C6_OUSIA_HEAL_KEY);

const CB_ARKHE: u16 = 1 << 0;
const CB_C6: u16 = 1 << 1;
const CB_PARTICLE: u16 = 1 << 2;
const CB_C4: u16 = 1 << 3;

const TASK_NORMAL: u8 = 0;
const TASK_ARKHE: u8 = 1;
const TASK_CHARGE: u8 = 2;
// arg: 1 to switch to Pneuma, 0 to Ousia
const TASK_ARKHE_SWITCH: u8 = 3;
const TASK_SUMMON_SALON: u8 = 4;
const TASK_SUMMON_SINGER: u8 = 5;
// a Salon Member's tick (arg: src * 64 + tick) and its hit (arg: src),
// by member: Chevalmarin, Usher, Crabaletta
const TASK_SALON_TICK: u8 = 6;
const TASK_SALON_HIT: u8 = 9;
const TASK_SINGER: u8 = 12;
const TASK_BURST_STATUS: u8 = 13;
const TASK_DEBOUNCE: u8 = 14;
// arg: the index of the gain in the ring
const TASK_FANFARE: u8 = 15;
const TASK_A1_HEAL: u8 = 16;
const TASK_A4: u8 = 17;
// arg: src * 4 + the character healed
const TASK_C6_HEAL: u8 = 18;

const FRAMES_CHARGE: u8 = 0;

/// Chevalmarin, Usher and Crabaletta: initial tick, mean interval,
/// travel, HP drained, ICD tag and group, and name.
struct Member {
    initial: f64,
    interval: f64,
    travel: i32,
    drain: f64,
    icd_tag: IcdTag,
    icd_group: IcdGroup,
    abil: &'static str,
}

const MEMBERS: [Member; 3] = [
    Member {
        initial: g::CHEVALMARIN_INITIAL_TICK,
        interval: g::CHEVALMARIN_INTERVAL_MEAN,
        travel: g::CHEVALMARIN_TRAVEL,
        drain: 0.016,
        icd_tag: icd::TAG_FURINA_CHEVALMARIN,
        icd_group: icd::GROUP_FURINA_SALON_SOLITAIRE,
        abil: "Salon Member: Surintendante Chevalmarin",
    },
    Member {
        initial: g::USHER_INITIAL_TICK,
        interval: g::USHER_INTERVAL_MEAN,
        travel: g::USHER_TRAVEL,
        drain: 0.024,
        icd_tag: icd::TAG_FURINA_USHER,
        icd_group: icd::GROUP_FURINA_SALON_SOLITAIRE,
        abil: "Salon Member: Gentilhomme Usher",
    },
    Member {
        initial: g::CRABALETTA_INITIAL_TICK,
        interval: g::CRABALETTA_INTERVAL_MEAN,
        travel: g::CRABALETTA_TRAVEL,
        drain: 0.036,
        icd_tag: icd::TAG_NONE,
        icd_group: icd::GROUP_DEFAULT,
        abil: "Salon Member: Mademoiselle Crabaletta",
    },
];

impl Character for Char {
    const KEY: &'static str = "furina";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            pneuma: false,
            cur_fanfare: 0.0,
            max_q_fanfare: 0.0,
            max_c2_fanfare: 0.0,
            fanfare_debounce_task_queued: false,
            pending: [0.0; PENDING],
            next_pending: 0,
            a1_src: 0,
            a4_dmg: 0.0,
            a4_interval_reduction: 0.0,
            last_summon_src: 0,
            salon_flat: [0.0; 3],
            c6_count: 0,
            c6_heal_src: 0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // A1, then the burst's fanfare: both on heals
        s.subscribe(EventKind::Heal, o);
        if asc >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Attack, o, st::NONE, false, false, -1));
            a4_tick(s, ci);
        }
        // burstInit
        let st = s.st_mut::<State>(ci);
        st.max_c2_fanfare = 300.0;
        st.max_q_fanfare = 300.0;
        if cons >= 1 {
            st.max_q_fanfare = 400.0;
            st.max_c2_fanfare = 400.0;
        }
        if cons >= 2 {
            st.max_c2_fanfare = 800.0;
        }
        s.subscribe(EventKind::Drain, o);
        for i in 0..s.chars.len() {
            s.chars[i].mods.push(Mod::owned(BURST_DMG, ModKind::Attack, o, st::NONE, false, false, -1));
            s.chars[i].mods.push(Mod::owned(BURST_HEAL, ModKind::HealBonus, o, st::NONE, false, false, -1));
        }
        if cons >= 2 {
            s.chars[ci].mods.push(Mod::owned(C2, ModKind::Stat, o, st::HPP, false, false, -1));
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// Furina charges without a normal attack before it.
    fn invalid(_: &Sim, _: usize, _: Action, _: &Params) -> Option<String> {
        None
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let st = s.st::<State>(ci);
        match fields[0].as_str() {
            "ousia" => Some(Ok(Num::from_bool(!st.pneuma))),
            "fanfare" => Some(Ok(Num::Float(if s.mod_active(ci, BURST) { st.cur_fanfare } else { 0.0 }))),
            "c6-count" => Some(Ok(Num::Int(st.c6_count as i64))),
            _ => None,
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _on: usize, m: &Mod, out: &mut Stats) -> bool {
        let st = s.st::<State>(ci);
        if !s.mod_active(ci, BURST) {
            return false;
        }
        if m.key == BURST_HEAL {
            out[st::HEAL] += st.cur_fanfare.min(st.max_q_fanfare) * g::BURST_FANFARE_HB_RATIO[s.talent(ci, 2)];
        } else {
            // C2
            out[st::HPP] += (st.cur_fanfare - st.max_q_fanfare).max(0.0) * 0.0035;
        }
        true
    }

    fn attack_mod(s: &Sim, ci: usize, _on: usize, m: &Mod, ev: &mut AttackEvent) {
        let st = s.st::<State>(ci);
        if m.key == BURST_DMG {
            if s.mod_active(ci, BURST) {
                ev.stats[st::DMGP] += st.cur_fanfare.min(st.max_q_fanfare) * g::BURST_FANFARE_DMG_RATIO[s.talent(ci, 2)];
            }
        } else if ev.info.tag == AttackTag::ElementalArt && ev.info.abil.contains(g::SALON_MEMBER_KEY) {
            // A4
            ev.stats[st::DMGP] += st.a4_dmg;
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter;
        let hitmark = g::ATTACK_HITMARKS[n as usize];
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: n }, hitmark);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n as usize];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hitmark, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let windup = if s.current_state() == AnimState::Normal { 11 } else { 0 };
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CHARGE, arg: s.st::<State>(ci).pneuma as i32 }, g::CHARGE_HITMARK - windup);
        // a frame after the hit, so that C6 reads the old Arkhe
        let to_pneuma = !s.st::<State>(ci).pneuma;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ARKHE_SWITCH, arg: to_pneuma as i32 }, g::CHARGE_HITMARK - windup + 1);
        ActInfo::new(Frames::Char { id: FRAMES_CHARGE, arg: windup as i8 }, g::CHARGE_FRAMES[0] - windup, g::CHARGE_HITMARK - windup, AnimState::Charge)
    }

    fn frames(_: u8, windup: i8, next: Action) -> i32 {
        g::CHARGE_FRAMES[next as usize] - windup as i32
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.chars[ci].cons >= 6 {
            s.st_mut::<State>(ci).c6_count = 0;
            s.add_status(ci, C6, 10 * 60, true);
        }
        if s.st::<State>(ci).pneuma {
            s.add_status(ci, SKILL, g::SKILL_MAX_DURATION + g::PNEUMA_CD_DELAY, false);
            s.add_task(Task::Char { c: ci as u8, id: TASK_SUMMON_SINGER, arg: 0 }, g::PNEUMA_CD_DELAY);
            s.set_cd_with_delay(ci, Action::Skill, 1200, g::PNEUMA_CD_DELAY);
            let t = &g::SKILL_FRAMES[g::PNEUMA as usize];
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill);
        }
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
        ai.abil = "Salon Solitaire: Ousia Bubble";
        ai.flat_dmg = g::SKILL_OUSIA_BUBBLE[lvl] * s.max_hp(ci);
        s.queue_attack(ai, 0, g::OUSIA_BUBBLE_HITMARK, g::OUSIA_BUBBLE_HITMARK);
        s.add_status(ci, SKILL, g::SKILL_MAX_DURATION + g::OUSIA_BUBBLE_HITMARK, false);
        s.add_task(Task::Char { c: ci as u8, id: TASK_SUMMON_SALON, arg: 0 }, g::OUSIA_BUBBLE_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 1200, g::OUSIA_CD_DELAY);
        let t = &g::SKILL_FRAMES[g::OUSIA as usize];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
        ai.abil = "Let the People Rejoice";
        ai.flat_dmg = s.max_hp(ci) * g::BURST_DMG[lvl];
        s.st_mut::<State>(ci).cur_fanfare = 0.0;
        s.delete_mod(ci, BURST);
        // before the hitmark, so that the hit has C1's fanfare
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_STATUS, arg: 0 }, 95);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 7);
        ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_FRAMES[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => normal_hit(s, ci, arg as usize),
            TASK_ARKHE => arkhe_strike(s, ci),
            TASK_CHARGE => charge_hit(s, ci),
            TASK_ARKHE_SWITCH => {
                let pneuma = arg == 1;
                s.st_mut::<State>(ci).pneuma = pneuma;
                if s.mod_active(ci, SKILL) {
                    let id = if pneuma { TASK_SUMMON_SINGER } else { TASK_SUMMON_SALON };
                    s.add_task(Task::Char { c: ci as u8, id, arg: 0 }, 0);
                }
            }
            TASK_SUMMON_SALON => {
                let src = s.f;
                s.st_mut::<State>(ci).last_summon_src = src;
                for m in 0..3 {
                    let d = salon_tick_frame(s, 0, &MEMBERS[m]);
                    s.add_task(Task::Char { c: ci as u8, id: TASK_SALON_TICK + m as u8, arg: src * 64 }, d);
                }
            }
            TASK_SUMMON_SINGER => {
                let src = s.f;
                s.st_mut::<State>(ci).last_summon_src = src;
                s.add_task(Task::Char { c: ci as u8, id: TASK_SINGER, arg: src }, g::SINGER_INITIAL_TICK);
            }
            TASK_SALON_TICK..=11 if id < TASK_SALON_HIT => salon_tick(s, ci, (id - TASK_SALON_TICK) as usize, arg / 64, arg % 64),
            TASK_SALON_HIT..=11 => salon_hit(s, ci, (id - TASK_SALON_HIT) as usize, arg),
            TASK_SINGER => singer(s, ci, arg),
            TASK_BURST_STATUS => {
                if s.chars[ci].cons >= 1 {
                    s.st_mut::<State>(ci).cur_fanfare = 150.0;
                }
                s.add_status(ci, BURST, g::BURST_DUR as i32, true);
            }
            TASK_DEBOUNCE => {
                s.add_status(ci, FANFARE_DEBOUNCE, g::FANFARE_DEBOUNCE_DUR, false);
                s.st_mut::<State>(ci).fanfare_debounce_task_queued = false;
            }
            TASK_FANFARE => {
                let c2 = s.chars[ci].cons >= 2;
                let st = s.st_mut::<State>(ci);
                let mut amt = st.pending[arg as usize];
                if c2 {
                    amt *= 3.5;
                }
                st.cur_fanfare = st.max_c2_fanfare.min(st.cur_fanfare + amt);
            }
            TASK_A1_HEAL => {
                if !s.mod_active(ci, A1_HEAL) || s.st::<State>(ci).a1_src != arg {
                    return;
                }
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, None, true, 0.02, bonus);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1_HEAL, arg }, 2 * 60);
            }
            TASK_A4 => a4_tick(s, ci),
            TASK_C6_HEAL => {
                let (src, on) = (arg / 4, (arg % 4) as usize);
                if s.st::<State>(ci).c6_heal_src != src || !s.mod_active(ci, C6_OUSIA_HEAL) {
                    return;
                }
                let (hp, bonus) = (s.max_hp(ci), s.stat(ci, st::HEAL));
                s.heal(ci, on, 0.04 * hp, bonus);
                s.queue_char_task(on, Task::Char { c: ci as u8, id: TASK_C6_HEAL, arg }, 60);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16) {
        if cbs & CB_ARKHE != 0 {
            arkhe_cb(s, ci);
        }
        if cbs & CB_C6 != 0 {
            c6_cb(s, ci);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 150, true);
            s.queue_particle(1.0, Element::Hydro, s.particle_delay[ci]);
        }
        if cbs & CB_C4 != 0 {
            c4_energy(s, ci);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Heal { caller, target, on, amount, overheal } => {
                if s.chars[ci].ascension >= 1 {
                    a1(s, ci, caller, target, overheal);
                }
                if !s.mod_active(ci, BURST) || amount <= 0.0 || (amount - overheal).abs() <= 1e-9 {
                    return;
                }
                let amt = (amount - overheal) / s.max_hp(on) * 100.0;
                queue_fanfare_gain(s, ci, amt);
            }
            Event::Drain { on, amount } => {
                if !s.mod_active(ci, BURST) || amount <= 0.0 {
                    return;
                }
                let amt = amount / s.max_hp(on) * 100.0;
                queue_fanfare_gain(s, ci, amt);
            }
            _ => {}
        }
    }
}

/// C6's bonus to normal and charged attacks, more in Pneuma.
fn c6_flat(s: &Sim, ci: usize, extra: bool) -> f64 {
    let mut scale = 0.18;
    if extra && s.st::<State>(ci).pneuma {
        scale += 0.25;
    }
    scale * s.max_hp(ci)
}

fn c6_active(s: &Sim, ci: usize) -> bool {
    s.chars[ci].cons >= 6 && s.mod_active(ci, C6)
}

fn normal_hit(s: &mut Sim, ci: usize, n: usize) {
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
    ai.abil = NORMAL_NAMES[n];
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
    ai.def_halt = true;
    let mut cbs = CB_ARKHE;
    if c6_active(s, ci) {
        ai.element = Element::Hydro;
        ai.ignore_infusion = true;
        ai.flat_dmg = c6_flat(s, ci, true);
        cbs |= CB_C6;
    }
    s.queue_attack(ai, cbs, 0, 0);
}

fn arkhe_cb(s: &mut Sim, ci: usize) {
    let a = s.st::<State>(ci).pneuma as usize;
    if s.mod_active(ci, ARKHE_ICD[a]) {
        return;
    }
    s.add_status(ci, ARKHE_ICD[a], 6 * 60, true);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ARKHE, arg: 0 }, 30);
}

fn arkhe_strike(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 0.0, g::ARKHE[lvl]);
    ai.abil = if s.st::<State>(ci).pneuma { "Surging Blade (furina)" } else { "Spiritbreath Thorn (furina)" };
    ai.ignore_infusion = true;
    // C6's bonus applies, without Pneuma's extra
    if c6_active(s, ci) {
        ai.flat_dmg = c6_flat(s, ci, false);
    }
    s.queue_attack(ai, 0, 0, 0);
}

fn charge_hit(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[lvl]);
    // gcsim names it after the Arkhe when the charge started
    ai.abil = if s.st::<State>(ci).pneuma { "Charge Pneuma" } else { "Charge Ousia" };
    ai.hitlag_halt = 0.02 * 60.0;
    let mut cbs = 0;
    if c6_active(s, ci) {
        ai.flat_dmg = c6_flat(s, ci, true);
        cbs = CB_C6;
        ai.element = Element::Hydro;
        ai.ignore_infusion = true;
    }
    s.queue_attack(ai, cbs, 0, 0);
}

/// gcsim's calcSalonTick: the frame, from the summon, of a member's tick,
/// with a skewed random offset in [-7, 7].
fn salon_tick_frame(s: &mut Sim, tick: i32, m: &Member) -> i32 {
    let off = ((s.rng.norm_float64() * 1.0403 + 4.073023273).max(0.0).powf(1.0 / 0.728) - 7.0).min(7.0);
    (m.initial + tick as f64 * m.interval + off).round() as i32
}

fn salon_tick(s: &mut Sim, ci: usize, member: usize, src: i32, tick: i32) {
    let st = s.st::<State>(ci);
    if st.pneuma || src != st.last_summon_src || !s.mod_active(ci, SKILL) {
        return;
    }
    let m = &MEMBERS[member];
    let drained = consume_allies_health(s, m.drain);
    let mult = 1.0 + 0.1 * drained as f64;
    let flat = [g::SKILL_CHEVALMARIN, g::SKILL_USHER, g::SKILL_CRABALETTA][member][s.talent(ci, 1)] * s.max_hp(ci) * mult;
    s.st_mut::<State>(ci).salon_flat[member] = flat;
    s.add_task(Task::Char { c: ci as u8, id: TASK_SALON_HIT + member as u8, arg: src }, m.travel);
    let next = salon_tick_frame(s, tick + 1, m) - (s.f - src);
    s.add_task(Task::Char { c: ci as u8, id: TASK_SALON_TICK + member as u8, arg: src * 64 + tick + 1 }, next);
}

/// gcsim's queueSalonAttack: the attack is lost if the members were
/// dismissed or recalled meanwhile.
fn salon_hit(s: &mut Sim, ci: usize, member: usize, src: i32) {
    if src != s.st::<State>(ci).last_summon_src || !s.mod_active(ci, SKILL) {
        return;
    }
    let m = &MEMBERS[member];
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, m.icd_tag, m.icd_group, Element::Hydro, 25.0, 0.0);
    ai.abil = m.abil;
    ai.flat_dmg = s.st::<State>(ci).salon_flat[member];
    let mut cbs = CB_PARTICLE;
    if s.chars[ci].cons >= 4 {
        cbs |= CB_C4;
    }
    s.queue_attack(ai, cbs, 0, 0);
}

/// The members drain every character above half HP, except the active one
/// in a burst's animation; they hit harder for each above half.
fn consume_allies_health(s: &mut Sim, ratio: f64) -> i32 {
    let mut n = 0;
    for i in 0..s.chars.len() {
        if s.chars[i].hp_ratio <= 0.5 {
            continue;
        }
        n += 1;
        if s.player.active as usize == i && s.current_state() == AnimState::Burst {
            continue;
        }
        let amt = s.max_hp(i) * ratio;
        s.drain(i, amt);
    }
    n
}

fn singer(s: &mut Sim, ci: usize, src: i32) {
    let st = s.st::<State>(ci);
    if !st.pneuma || src != st.last_summon_src || !s.mod_active(ci, SKILL) {
        return;
    }
    let lvl = s.talent(ci, 1);
    let heal = g::SKILL_SINGER_HEAL_FLAT[lvl] + g::SKILL_SINGER_HEAL_SCALE[lvl] * s.max_hp(ci);
    let (active, bonus) = (s.player.active as usize, s.stat(ci, st::HEAL));
    s.heal(ci, active, heal, bonus);
    let interval = (g::SINGER_INTERVAL as f64 * (1.0 - s.st::<State>(ci).a4_interval_reduction) + 0.5) as i32;
    s.add_task(Task::Char { c: ci as u8, id: TASK_SINGER, arg: src }, interval);
    if s.chars[ci].cons >= 4 {
        c4_energy(s, ci);
    }
}

/// Fanfare from an HP change comes after a delay, and after the debounce
/// when one is running.
fn queue_fanfare_gain(s: &mut Sim, ci: usize, amt: f64) {
    let delay = if !s.mod_active(ci, FANFARE_DEBOUNCE) {
        // drains of other characters in the same frame still come before it
        if !s.st::<State>(ci).fanfare_debounce_task_queued {
            s.st_mut::<State>(ci).fanfare_debounce_task_queued = true;
            s.add_task(Task::Char { c: ci as u8, id: TASK_DEBOUNCE, arg: 0 }, 1);
        }
        g::FANFARE_DRAIN_TO_GAIN_DELAY
    } else {
        s.mod_duration(ci, g::FANFARE_DEBOUNCE_KEY)
    };
    let st = s.st_mut::<State>(ci);
    let i = st.next_pending;
    st.pending[i] = amt;
    st.next_pending = (i + 1) % PENDING;
    s.add_task(Task::Char { c: ci as u8, id: TASK_FANFARE, arg: i as i32 }, delay);
}

/// A1: overhealing of the active character (or the party) by others heals
/// the party 2% of their max HP every 2 s while it lasts.
fn a1(s: &mut Sim, ci: usize, caller: usize, target: Option<usize>, overheal: f64) {
    if caller == ci || overheal <= 0.0 || target.is_some_and(|t| t != s.player.active as usize) {
        return;
    }
    if !s.mod_active(ci, A1_HEAL) {
        let f = s.f;
        s.st_mut::<State>(ci).a1_src = f;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1_HEAL, arg: f }, 2 * 60);
    }
    s.add_status(ci, A1_HEAL, 4 * 60, true);
}

/// A4: from max HP, the Salon Members' DMG bonus and the Singer's faster
/// heals, updated every half second.
fn a4_tick(s: &mut Sim, ci: usize) {
    let hp = s.max_hp(ci);
    let st = s.st_mut::<State>(ci);
    st.a4_dmg = (hp / 1000.0 * 0.007).min(0.28);
    st.a4_interval_reduction = (hp / 1000.0 * 0.004).min(0.16);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, 30);
}

fn c4_energy(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, C4_ICD) {
        return;
    }
    s.add_status(ci, C4_ICD, 5 * 60, true);
    s.add_energy(ci, 4.0);
}

/// C6's hits: in Ousia they heal the party over time, in Pneuma they
/// drain it.
fn c6_cb(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, C6_ICD) {
        return;
    }
    s.add_status(ci, C6_ICD, 6, true);
    if !s.st::<State>(ci).pneuma {
        if !s.mod_active(ci, C6_OUSIA_HEAL) {
            let f = s.f;
            s.st_mut::<State>(ci).c6_heal_src = f;
            for i in 0..s.chars.len() {
                s.queue_char_task(i, Task::Char { c: ci as u8, id: TASK_C6_HEAL, arg: f * 4 + i as i32 }, 60);
                s.add_status(i, C6_OUSIA_HEAL, 174, true);
            }
        } else {
            for i in 0..s.chars.len() {
                s.extend_mod(i, C6_OUSIA_HEAL, 174);
            }
        }
    } else {
        for i in 0..s.chars.len() {
            let amt = s.current_hp(i) * 0.01;
            s.drain(i, amt);
        }
    }
    let st = s.st_mut::<State>(ci);
    st.c6_count += 1;
    if st.c6_count == 6 {
        s.delete_mod(ci, C6);
    }
}
