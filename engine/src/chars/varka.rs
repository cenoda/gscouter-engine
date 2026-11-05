//! Varka, from gcsim's internal/characters/varka: normal and charged
//! attacks, Windbound Execution (the skill) and its Sturm und Drang state
//! (converted normal and charged attacks, Four Winds' Ascension and Azure
//! Devour on gcsim's ActionSpecialSkill cooldown of two charges), the
//! burst, A1, A4, his Hexerei cooldown reduction and every constellation.
//!
//! The special skill's cooldown is gcsim's template cooldown queue for
//! ActionSpecialSkill, kept here (`Special`); a reduction or reset of his
//! skill's cooldown in Sturm und Drang reaches it too, and the skill's
//! `cd` and `charge` conditions read it then, as his overrides do.

use crate::content::{Character, Event};
use crate::gamedata::chars::varka as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

/// gcsim's cooldown queue of one action with charges.
#[derive(Clone, Copy)]
struct Special {
    available: i32,
    queue: [i32; 4],
    len: usize,
    started: i32,
    token: i32,
}

const SPECIAL_CHARGES: i32 = 2;

pub struct State {
    conv: Element,
    four_winds_cd_stacks: i32,
    a1_buff: f64,
    a1_multiplier: f64,
    a4_stacks: i32,
    c1_extra: i32,
    special: Special,
    /// The ATK SPD a normal attack read, for its frames.
    atkspd: f64,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const A1_ANEMO: ModKey = ModKey::new("varka-a1-anemo");
const A1_CONV: ModKey = ModKey::new("varka-a1-conv");
const A4: ModKey = ModKey::new(g::A4_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const A4_MOD: ModKey = ModKey::new("varka-a4");
const C4_ANEMO: ModKey = ModKey::new("varka-c4-anemo");
const C6_CD: ModKey = ModKey::new("varka-c6-cdmg");
const C6_FREE_SKILL: ModKey = ModKey::new(g::C6_FREE_SKILL_KEY);
const C6_FREE_CA: ModKey = ModKey::new(g::C6_FREE_CA_KEY);

fn c4_key(e: Element) -> ModKey {
    match e {
        Element::Pyro => ModKey::new("varka-c4-pyro"),
        Element::Hydro => ModKey::new("varka-c4-hydro"),
        Element::Electro => ModKey::new("varka-c4-electro"),
        _ => ModKey::new("varka-c4-cryo"),
    }
}

const CB_PARTICLE: u16 = 1 << 0;
const CB_CD_RED: u16 = 1 << 1;

/// The skill's state, a frame before its hitmark.
const TASK_SKILL_STATE: u8 = 0;
/// The special skill's cooldown worker, with its token.
const TASK_SP_WORKER: u8 = 1;
/// The special skill's cooldown and C6, 39 frames after Four Winds.
const TASK_FOUR_WINDS_CD: u8 = 2;
/// A1's update every second.
const TASK_A1: u8 = 3;

/// frames_live: a normal attack's, arg counter * 8 + windup.
const FR_ATTACK: u8 = 0;
/// frames: a table with a windup, arg table * 32 + windup.
const FR_CHARGE: u8 = 0;
const FR_AZURE: u8 = 1;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];
const SKILL_NORMAL_NAMES: [&str; 5] = ["Sturm und Drang Normal 0", "Sturm und Drang Normal 1", "Sturm und Drang Normal 2", "Sturm und Drang Normal 3", "Sturm und Drang Normal 4"];

impl Character for Char {
    const KEY: &'static str = "varka";
    const HEXEREI: bool = true;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        let special = Special { available: SPECIAL_CHARGES, queue: [0; 4], len: 0, started: 0, token: 0 };
        State { conv: Element::Physical, four_winds_cd_stacks: 0, a1_buff: 0.0, a1_multiplier: 1.0, a4_stacks: 0, c1_extra: 0, special, atkspd: 0.0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let conv = [Element::Pyro, Element::Hydro, Element::Electro, Element::Cryo].into_iter().find(|&e| s.chars.iter().any(|c| c.element == e)).unwrap_or(Element::Physical);
        s.st_mut::<State>(ci).conv = conv;
        let o = Owner::Char(ci as u8);
        // onExitField
        s.subscribe(EventKind::Swap, o);
        // a1Init
        if s.chars[ci].ascension >= 1 {
            s.add_mod(ci, Mod::owned(A1_ANEMO, ModKind::Stat, o, st::ANEMOP, false, false, -1), -1);
            s.add_mod(ci, Mod::owned(A1_CONV, ModKind::Stat, o, conv.dmg_stat(), false, false, -1), -1);
            a1_update(s, ci);
            let count = |e: Element| s.chars.iter().filter(|c| c.element == e).count();
            let two_anemo = count(Element::Anemo) >= 2;
            let two_phec = [Element::Pyro, Element::Hydro, Element::Electro, Element::Cryo].into_iter().any(|e| count(e) >= 2);
            let m = g::A1_MULTIPLIER[usize::from(two_anemo) + usize::from(two_phec)];
            s.st_mut::<State>(ci).a1_multiplier = m;
        }
        // a4Init (gcsim checks no ascension) and c4Init: swirls
        s.subscribe(EventKind::Reaction, o);
        s.add_mod(ci, Mod::owned(A4_MOD, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        // c6Init: CRIT DMG per A4 stack (on a mod affecting anemo DMG%)
        if s.chars[ci].cons >= 6 {
            s.add_mod(ci, Mod::owned(C6_CD, ModKind::Stat, o, st::ANEMOP, false, false, -1), -1);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        if a == Action::Skill && use_special(s, ci) {
            if s.st::<State>(ci).special.available > 0 || c6_free(s, ci, C6_FREE_SKILL) {
                return Some(None);
            }
            return Some(Some(Failure::SkillCd));
        }
        None
    }

    fn action_stam(s: &Sim, ci: usize, a: Action) -> Option<f64> {
        if a != Action::Charge {
            return None;
        }
        if use_special(s, ci) && (s.st::<State>(ci).special.available > 0 || c6_free(s, ci, C6_FREE_CA)) {
            return Some(0.0);
        }
        Some(50.0)
    }

    fn skill_cd_changed(s: &mut Sim, ci: usize, reduce: Option<i32>) {
        if !use_special(s, ci) {
            return;
        }
        match reduce {
            Some(v) => sp_reduce(s, ci, v),
            None => sp_reset(s, ci),
        }
    }

    fn cd_view(s: &Sim, ci: usize, a: Action) -> Option<(i32, i32)> {
        if a != Action::Skill || !use_special(s, ci) {
            return None;
        }
        let sp = &s.st::<State>(ci).special;
        let cd = if sp.len == 0 { 0 } else { sp.started + sp.queue[0] - s.f };
        Some((cd, sp.available))
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let state = s.current_state();
        let mode = s.mod_active(ci, SKILL);
        let windup = if matches!(state, AnimState::Idle | AnimState::Swap | AnimState::Jump | AnimState::Dash) || (mode && state == AnimState::Walk) { 5 } else { 0 };
        let conv = s.st::<State>(ci).conv;
        let hb = g::ATTACK_HITBOXES[n];
        for (i, &h) in g::ATTACK_HITMARKS[n].iter().enumerate() {
            let off = g::ATTACK_OFFSETS[n][i];
            let pattern = if hb.len() == 1 { s.circle_on_player(0.0, off, hb[0]) } else { s.box_on_player(0.0, off, hb[0], hb[1]) };
            if mode {
                let k = (i + usize::from(n == 1 || n == 2)) % 2;
                let ele = if k == 0 { conv } else { Element::Anemo };
                let icd_tag = match k {
                    0 if conv == Element::Physical => icd::TAG_NORMAL_ATTACK,
                    0 => icd::TAG_VARKA_NORMAL_ATTACK_ELEM,
                    _ => icd::TAG_VARKA_NORMAL_ATTACK_WIND,
                };
                let mult = g::SKILL_ATTACK[n][i][s.talent(ci, 1)] * a1_skill_multi(s, ci);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd_tag, icd::GROUP_DEFAULT, ele, 25.0, mult);
                ai.abil = SKILL_NORMAL_NAMES[n];
                ai.ignore_infusion = ele != Element::Physical;
                set_normal(&mut ai, n, i, pattern);
                s.queue_attack(ai, CB_CD_RED, h + windup, h + windup);
            } else {
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][s.talent(ci, 0)]);
                ai.abil = NORMAL_NAMES[n];
                set_normal(&mut ai, n, i, pattern);
                s.queue_attack_at(ci, ai, 0, windup + h);
            }
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        s.st_mut::<State>(ci).atkspd = atkspd;
        let table = &g::ATTACK_FRAMES[n];
        let marks = g::ATTACK_HITMARKS[n];
        let info = ActInfo::new(Frames::CharLive { id: FR_ATTACK, arg: (n * 8) as i16 + windup as i16 }, table[0] + windup, marks[marks.len() - 1] + windup, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let windup = if matches!(s.current_state(), AnimState::Normal | AnimState::Charge) { 0 } else { 14 };
        let mode = s.mod_active(ci, SKILL);
        if mode && use_special(s, ci) && (s.st::<State>(ci).special.available > 0 || c6_free(s, ci, C6_FREE_CA)) {
            return azure_devour(s, ci, windup);
        }
        let conv = s.st::<State>(ci).conv;
        for (i, &h) in g::CHARGE_HITMARKS.iter().enumerate() {
            let mut ai = if mode {
                let ele = if i == 0 { conv } else { Element::Anemo };
                let icd_tag = match i {
                    0 if conv == Element::Physical => icd::TAG_EXTRA_ATTACK,
                    0 => icd::TAG_VARKA_EXTRA_ATTACK_ELEM,
                    _ => icd::TAG_VARKA_EXTRA_ATTACK_WIND,
                };
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd_tag, icd::GROUP_DEFAULT, ele, 25.0, g::SKILL_CHARGE[i][s.talent(ci, 1)] * a1_skill_multi(s, ci));
                ai.abil = "Sturm und Drang Charge";
                ai.ignore_infusion = ele != Element::Physical;
                ai
            } else {
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[i][s.talent(ci, 0)]);
                ai.abil = "Charge";
                ai
            };
            ai.blunt = true;
            ai.poise = g::CHARGE_POISE_DMG[i];
            ai.hitlag_halt = g::CHARGE_HITLAG_HALT_FRAMES[i] * 60.0;
            ai.hitlag_factor = 0.01;
            ai.def_halt = g::CHARGE_CAN_BE_DEFENSE_HALTED[i];
            ai.pattern = s.circle_on_player(0.0, 1.0, 2.5);
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: FR_CHARGE, arg: windup as i16 }, t[0] + windup, g::CHARGE_HITMARKS[1] + windup, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if use_special(s, ci) {
            let free = c6_free(s, ci, C6_FREE_SKILL);
            return four_winds(s, ci, free);
        }
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL_INITIAL[lvl]);
        ai.abil = "Windbound Execution";
        ai.blunt = true;
        ai.poise = 100.0;
        ai.hitlag_halt = 0.09 * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
        let h = g::SKILL_HITMARK;
        s.queue_attack(ai, CB_PARTICLE, h, h);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_STATE, arg: 0 }, h - 1);
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD, 39);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let conv = s.st::<State>(ci).conv;
        let first = if conv == Element::Physical { Element::Anemo } else { conv };
        for (i, &h) in g::BURST_HITMARK.iter().enumerate() {
            let ele = if i == 0 { first } else { Element::Anemo };
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, ele, 25.0, g::BURST[i][lvl]);
            ai.abil = "Northwind Avatar";
            ai.blunt = true;
            ai.poise = g::BURST_POISE_DMG[i];
            ai.pattern = s.circle_on_player(0.0, 2.0, 7.0);
            s.queue_attack(ai, 0, h, h);
        }
        s.extend_mod(ci, SKILL, 138);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 4);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        let t = if id == FR_AZURE { &g::AZURE_DEVOUR_FRAMES } else { &g::CHARGE_FRAMES };
        t[next as usize] + arg as i32
    }

    fn frames_live(s: &Sim, ci: usize, _: u8, arg: i16, next: Action) -> i32 {
        let (n, windup) = ((arg / 8) as usize, (arg % 8) as i32);
        atk_spd_adjust(g::ATTACK_FRAMES[n][next as usize], s.st::<State>(ci).atkspd) + windup
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL_STATE => {
                s.add_status(ci, SKILL, 12 * 60, true);
                if s.st::<State>(ci).conv == Element::Physical {
                    return;
                }
                s.st_mut::<State>(ci).four_winds_cd_stacks = 0;
                sp_discard(s, ci, g::FOUR_WINDS_CD);
                for _ in 0..s.st::<State>(ci).special.available {
                    sp_set_cd(s, ci, g::FOUR_WINDS_CD);
                }
                // c1OnSkill
                if s.chars[ci].cons >= 1 {
                    s.st_mut::<State>(ci).c1_extra = 1;
                    sp_reset(s, ci);
                }
            }
            TASK_SP_WORKER => {
                let sp = &mut s.st_mut::<State>(ci).special;
                if sp.token != arg {
                    return;
                }
                sp.available += 1;
                sp.queue.copy_within(1..4, 0);
                sp.len -= 1;
                if sp.len > 0 {
                    sp_start_worker(s, ci);
                }
            }
            TASK_FOUR_WINDS_CD => {
                sp_set_cd(s, ci, g::FOUR_WINDS_CD);
                if s.chars[ci].cons >= 6 {
                    s.add_status(ci, C6_FREE_CA, g::C6_DUR, true);
                }
            }
            TASK_A1 => a1_update(s, ci),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, true);
            s.queue_particle(6.0, Element::Anemo, s.particle_delay[ci]);
        }
        // fourWindsCDRedCB: once per hit
        if cbs & CB_CD_RED != 0 && s.st::<State>(ci).four_winds_cd_stacks < 15 && s.mod_active(ci, SKILL) {
            s.st_mut::<State>(ci).four_winds_cd_stacks += 1;
            let amt = if s.chars[ci].hexerei && s.hexerei_count() >= 2 { 60 } else { 30 };
            sp_reduce(s, ci, amt);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let st = s.st::<State>(ci);
        if m.key == A1_ANEMO {
            out[st::ANEMOP] += st.a1_buff;
            true
        } else if m.key == A1_CONV {
            out[st.conv.dmg_stat()] += st.a1_buff;
            true
        } else if m.key == C6_CD {
            if st.a4_stacks == 0 {
                return false;
            }
            out[st::CD] += st.a4_stacks as f64 * 0.2;
            true
        } else if m.key == C4_ANEMO {
            out[st::ANEMOP] += 0.2;
            true
        } else {
            // C4's element DMG%
            out[m.affected] += 0.2;
            true
        }
    }

    /// A4: normal, charged and Four Winds' Ascension DMG +7.5% per stack.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key != A4_MOD {
            return;
        }
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) && !ev.info.has_tag(add_tag::VARKA_SPECIAL) {
            return;
        }
        if !s.mod_active(ci, A4) {
            return;
        }
        ev.stats[st::DMGP] += s.st::<State>(ci).a4_stacks as f64 * 0.075;
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { prev, .. } => {
                if prev == ci && s.mod_active(ci, SKILL) {
                    s.delete_mod(ci, SKILL);
                }
            }
            Event::Reaction(r, ev) => {
                let ele = match r {
                    Reaction::SwirlPyro => Element::Pyro,
                    Reaction::SwirlHydro => Element::Hydro,
                    Reaction::SwirlElectro => Element::Electro,
                    Reaction::SwirlCryo | Reaction::StellarSwirl => Element::Cryo,
                    _ => return,
                };
                if !ev.on_enemy() {
                    return;
                }
                // A4: a stack, once a second for each character
                let actor = ev.info.actor as usize;
                if !s.mod_active(actor, A4_ICD) {
                    s.add_status(actor, A4_ICD, 60, true);
                    let active = s.mod_active(ci, A4);
                    let st = s.st_mut::<State>(ci);
                    st.a4_stacks = if active { (st.a4_stacks + 1).min(4) } else { 1 };
                    s.add_status(ci, A4, 8 * 60, true);
                }
                // C4: his swirls give the party anemo and the element's
                // DMG +20% for 10 s
                if s.chars[ci].cons >= 4 && actor == ci {
                    let o = Owner::Char(ci as u8);
                    for i in 0..s.chars.len() {
                        s.add_mod(i, Mod::owned(C4_ANEMO, ModKind::Stat, o, st::ANEMOP, false, true, -1), 10 * 60);
                    }
                    for i in 0..s.chars.len() {
                        s.add_mod(i, Mod::owned(c4_key(ele), ModKind::Stat, o, ele.dmg_stat(), false, true, -1), 10 * 60);
                    }
                }
            }
            _ => {}
        }
    }
}

fn set_normal(ai: &mut AttackInfo, n: usize, i: usize, pattern: Pattern) {
    ai.blunt = true;
    ai.poise = g::ATTACK_POISE_DMG[n][i];
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
    ai.def_halt = g::ATTACK_CAN_BE_DEFENSE_HALTED[n][i];
    ai.pattern = pattern;
}

fn use_special(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, SKILL) && s.st::<State>(ci).conv != Element::Physical
}

fn c6_free(s: &Sim, ci: usize, key: ModKey) -> bool {
    s.chars[ci].cons >= 6 && s.mod_active(ci, key)
}

fn a1_skill_multi(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 1 { 1.0 } else { s.st::<State>(ci).a1_multiplier }
}

/// A1: anemo and the converted element's DMG +10% per 1,000 ATK (without
/// extra mods), up to 25%, every second.
fn a1_update(s: &mut Sim, ci: usize) {
    let atk = total_atk(&s.select_stats(ci, [st::BASEATK, st::ATKP, st::ATK], true));
    s.st_mut::<State>(ci).a1_buff = (atk * 0.1 / 1000.0).min(0.25);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, 60);
}

/// C1: the first special skill after the skill does double damage.
fn c1_mult(s: &mut Sim, ci: usize) -> f64 {
    if s.chars[ci].cons < 1 {
        return 1.0;
    }
    let st = s.st_mut::<State>(ci);
    if st.c1_extra > 0 {
        st.c1_extra = 0;
        return 2.0;
    }
    1.0
}

/// C2: an anemo hit of 800% ATK with each special skill.
fn c2(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 2 {
        return;
    }
    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 0.0, 8.0);
    ai.abil = "Varka C2";
    ai.blunt = true;
    ai.poise = 20.0;
    ai.pattern = s.circle_on_player(0.0, 0.0, 7.0);
    s.queue_attack(ai, 0, 10, 10);
}

fn four_winds(s: &mut Sim, ci: usize, c6_free_skill: bool) -> ActInfo {
    let conv = s.st::<State>(ci).conv;
    let c1 = c1_mult(s, ci);
    let lvl = s.talent(ci, 1);
    for i in 0..2 {
        let ele = if i == 0 { conv } else { Element::Anemo };
        let mult = g::SKILL_ASCENSION[i][lvl] * a1_skill_multi(s, ci) * c1;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, ele, 25.0, mult);
        ai.abil = "Four Winds' Ascension";
        ai.blunt = true;
        ai.poise = g::FOUR_WINDS_POISE_DMG[i];
        ai.add_tags = add_tag::VARKA_SPECIAL;
        ai.hitlag_halt = g::FOUR_WINDS_HIT_HALT_FRAME[i] * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = g::FOUR_WINDS_CAN_BE_DEF_HALTED[i];
        ai.pattern = s.circle_on_player(0.0, 0.0, 6.5);
        let h = g::FOUR_WINDS_HITMARK[i];
        s.queue_attack(ai, 0, h, h);
    }
    if !c6_free_skill {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_FOUR_WINDS_CD, arg: 0 }, 39);
    } else {
        s.delete_mod(ci, C6_FREE_SKILL);
    }
    c2(s, ci);
    let t = &g::SPECIAL_SKILL_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Burst as usize], AnimState::Skill)
}

fn azure_devour(s: &mut Sim, ci: usize, windup: i32) -> ActInfo {
    let c6_free_ca = c6_free(s, ci, C6_FREE_CA);
    let conv = s.st::<State>(ci).conv;
    let c1 = c1_mult(s, ci);
    let lvl = s.talent(ci, 1);
    for (i, &h) in g::AZURE_DEVOUR_HITMARKS.iter().enumerate() {
        let (ele, icd_tag) = if i % 2 == 0 { (conv, icd::TAG_VARKA_EXTRA_ATTACK_ELEM) } else { (Element::Anemo, icd::TAG_VARKA_EXTRA_ATTACK_WIND) };
        let mult = g::SKILL_AZURE_DEVOUR[i][lvl] * a1_skill_multi(s, ci) * c1;
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd_tag, icd::GROUP_DEFAULT, ele, 25.0, mult);
        ai.abil = "Azure Devour";
        ai.blunt = true;
        ai.poise = g::AZURE_DEVOUR_POISE_DMG[i];
        ai.ignore_infusion = ele != Element::Physical;
        ai.hitlag_halt = g::AZURE_DEVOUR_HITLAG_HALT_FRAMES[i] * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = g::AZURE_DEVOUR_CAN_BE_DEFENSE_HALTED[i];
        ai.pattern = s.circle_on_player(0.0, 1.0, 3.0);
        s.queue_attack(ai, 0, h, h);
    }
    if !c6_free_ca {
        sp_set_cd(s, ci, 11 * 60);
        if s.chars[ci].cons >= 6 {
            s.add_status(ci, C6_FREE_SKILL, g::C6_DUR, true);
        }
    } else {
        s.delete_mod(ci, C6_FREE_CA);
    }
    c2(s, ci);
    let t = &g::AZURE_DEVOUR_FRAMES;
    ActInfo::new(Frames::Char { id: FR_AZURE, arg: windup as i16 }, t[0] + windup, g::AZURE_DEVOUR_HITMARKS[3] + windup, AnimState::Charge)
}

// ------------------------------------------------ the special skill's cooldown

fn sp_start_worker(s: &mut Sim, ci: usize) {
    let f = s.f;
    let sp = &mut s.st_mut::<State>(ci).special;
    if sp.len == 0 {
        return;
    }
    sp.started = f;
    sp.token += 1;
    let (token, dur) = (sp.token, sp.queue[0]);
    s.add_task(Task::Char { c: ci as u8, id: TASK_SP_WORKER, arg: token }, dur);
}

fn sp_set_cd(s: &mut Sim, ci: usize, dur: i32) {
    let dur = s.cd_reduction(ci, true, dur);
    let sp = &mut s.st_mut::<State>(ci).special;
    sp.queue[sp.len] = dur;
    sp.len += 1;
    let first = sp.len == 1;
    sp.available -= 1;
    if first {
        sp_start_worker(s, ci);
    }
}

fn sp_reset(s: &mut Sim, ci: usize) {
    let f = s.f;
    let sp = &mut s.st_mut::<State>(ci).special;
    if sp.available == SPECIAL_CHARGES {
        return;
    }
    sp.available += 1;
    sp.queue.copy_within(1..4, 0);
    sp.len -= 1;
    sp.started = f;
    sp.token += 1;
    if sp.len > 0 {
        sp_start_worker(s, ci);
    }
}

fn sp_reduce(s: &mut Sim, ci: usize, v: i32) {
    let f = s.f;
    let sp = &mut s.st_mut::<State>(ci).special;
    if sp.available == SPECIAL_CHARGES {
        return;
    }
    let remain = sp.started + sp.queue[0] - f;
    if v >= remain {
        sp_reset(s, ci);
        return;
    }
    sp.queue[0] = remain - v;
    sp_start_worker(s, ci);
}

/// gcsim's DiscardActionCooldown: the cooldown in progress replaced by a
/// new one at the end of the queue, the charges unchanged.
fn sp_discard(s: &mut Sim, ci: usize, dur: i32) {
    let f = s.f;
    let dur = s.cd_reduction(ci, true, dur);
    let sp = &mut s.st_mut::<State>(ci).special;
    if sp.available == SPECIAL_CHARGES || sp.len == 0 {
        return;
    }
    sp.queue.copy_within(1..4, 0);
    sp.len -= 1;
    sp.started = f;
    sp.token += 1;
    sp.queue[sp.len] = dur;
    sp.len += 1;
    sp_start_worker(s, ci);
}
