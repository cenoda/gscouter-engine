//! Clorinde, from gcsim's internal/characters/clorinde: normal and charged
//! attacks, Hunter's Vigil (the Night Watch state, its Swift Hunt shots and
//! Surging Blade, Impale the Night by her Bond of Life with its heals),
//! Last Lightfall, her ReceiveHeal in Night Watch, A1, A4 and every
//! constellation. gcsim has no plunges for her.
//!
//! Her Night Watch normal attacks keep their own counter (gcsim's
//! normalSCounter). It is kept here in the character's normal counter: the
//! skill that starts Night Watch resets the base counter, which then stays
//! 0 until Night Watch ends, so the base counter is 0 wherever gcsim reads
//! it while the Swift Hunt counter is in use (`State::s_mode`); every reset
//! of the engine's counter resets both, as her ResetNormalCounter does.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::clorinde as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The normal counter holds the Swift Hunt counter.
    s_mode: bool,
    /// gcsim's MultipleRefreshNoRemove: each stack's start frame.
    a1_stacks: [Option<i32>; 3],
    a4_stacks: [Option<i32>; 2],
    a1_percent: f64,
    a1_cap: f64,
    prev_hp_debt: f64,
    c6_stacks: i32,
    /// Her own Impale the Night heal is being given.
    own_heal: bool,
    /// The Swift Hunt shots, taken as they start, by counter.
    shots: [AttackInfo; 3],
    shot_bol: [bool; 3],
}

const STATE: ModKey = ModKey::new(g::SKILL_STATE_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const ARKHE_ICD: ModKey = ModKey::new(g::ARKHE_ICD_KEY);
const A1: ModKey = ModKey::new(g::CLORINDE_A1_BUFF_KEY);
const A4: ModKey = ModKey::new(g::CLORINDE_A4_BUFF_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C4: ModKey = ModKey::new("clorinde-c4-burst-bonus");
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);
const C6_GLIMBRIGHT_ICD: ModKey = ModKey::new(g::C6_GLIMBRIGHT_ICD_KEY);
const C6_CR: ModKey = ModKey::new("clorinde-c6-cr-bonus");
const C6_CD: ModKey = ModKey::new("clorinde-c6-cd-bonus");

const CB_PARTICLE: u16 = 1 << 0;

/// c6skill, as the skill starts.
const TASK_C6_SKILL: u8 = 0;
/// skillHeal at the full (arg 1) or low (arg 0) Bond of Life.
const TASK_HEAL: u8 = 1;
/// c6 after a full-Bond Impale the Night, in Night Watch.
const TASK_C6: u8 = 2;
/// A Swift Hunt shot at its hitmark, with its counter.
const TASK_SHOT: u8 = 3;
/// The burst's Bond of Life.
const TASK_BURST_BOL: u8 = 4;
/// A1's stack expiry, with src * 4 + index.
const TASK_A1_STACK: u8 = 5;
/// A4's stack expiry, with src * 4 + index.
const TASK_A4_STACK: u8 = 6;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];
const PIERCING_NAMES: [&str; 3] = ["Swift Hunt (Piercing Shot) 0", "Swift Hunt (Piercing Shot) 1", "Swift Hunt (Piercing Shot) 2"];
const NORMAL_SHOT_NAMES: [&str; 3] = ["Swift Hunt (Normal shot) 0", "Swift Hunt (Normal shot) 1", "Swift Hunt (Normal shot) 2"];

impl Character for Char {
    const KEY: &'static str = "clorinde";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        let shot = AttackInfo::new(0, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Electro, 25.0, 0.0);
        State {
            s_mode: false,
            a1_stacks: [None; 3],
            a4_stacks: [None; 2],
            a1_percent: 0.0,
            a1_cap: 0.0,
            prev_hp_debt: 0.0,
            c6_stacks: 0,
            own_heal: false,
            shots: [shot; 3],
            shot_bol: [false; 3],
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        if asc >= 1 {
            let st = s.st_mut::<State>(ci);
            (st.a1_percent, st.a1_cap) = if cons >= 2 { (g::C2_A1_PERCENT_BUFF, g::C2_A1_FLAT_DMG) } else { (g::A1_PERCENT_BUFF, g::A1_FLAT_DMG as f64) };
            s.subscribe(EventKind::Reaction, o);
        }
        if asc >= 4 {
            let r = s.hp_debt_ratio(ci);
            s.st_mut::<State>(ci).prev_hp_debt = r;
            s.subscribe(EventKind::HpDebt, o);
        }
        if cons >= 1 {
            s.subscribe(EventKind::EnemyDamage, o);
        }
        if cons >= 4 {
            s.chars[ci].mods.push(Mod::owned(C4, ModKind::Attack, o, st::NONE, false, false, -1));
        }
    }

    /// gcsim registers no params for her and reads none.
    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// The template's, reading her base normal counter (0 while the Swift
    /// Hunt counter is in use).
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::Charge && s.player.active as usize == ci && s.st::<State>(ci).s_mode {
            return Some("need to use attack right before charge".into());
        }
        crate::content::template_invalid(s, a)
    }

    /// In Night Watch the skill is always ready: Impale the Night.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && s.mod_active(ci, STATE)).then_some(None)
    }

    /// AnimationStartDelay: Xingqiu's and Yelan's N0 delays, by Night Watch.
    fn start_delay(s: &Sim, ci: usize, key: usize) -> Option<i32> {
        if !s.mod_active(ci, STATE) {
            return None;
        }
        match key {
            1 => Some(5),
            2 => Some(10),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, STATE) {
            return skill_attack(s, ci);
        }
        if s.st::<State>(ci).s_mode {
            // the base counter, 0 since Night Watch began
            s.st_mut::<State>(ci).s_mode = false;
            s.chars[ci].normal_counter = 0;
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let marks = g::ATTACK_HITMARKS[n];
        for (i, &h) in marks.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = g::ATTACK_HITLAG_FACTOR[n][i];
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let hb = g::ATTACK_HITBOXES[n][i];
            let oy = g::ATTACK_OFFSETS[n];
            ai.pattern = if n == 3 { s.box_on_player(0.0, oy, hb[0], hb[1]) } else { s.circle_on_player(0.0, oy, hb[0]) };
            s.queue_attack(ai, 0, h, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *marks.last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge";
        ai.hitlag_factor = 0.02;
        // NewCircleHitOnTarget(Player, nil, 4.1)
        ai.pattern = s.circle_on_player(0.0, 0.0, g::CHARGE_RADIUS);
        let h = g::CHARGE_HITMARK;
        s.queue_attack(ai, 0, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Charge)
    }

    /// Hunter's Vigil: Night Watch, or in it Impale the Night.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, STATE) {
            return skill_dash(s, ci);
        }
        s.add_status(ci, STATE, g::SKILL_START + (g::SKILL_STATE_DURATION * 60.0) as i32, true);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6_SKILL, arg: 0 }, 0);
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD, g::SKILL_START);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    /// Last Lightfall.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::BURST_DAMAGE[lvl]);
        ai.abil = "Last Lightfall";
        ai.hitlag_halt = 0.1;
        for &h in g::BURST_HITMARKS {
            // NewBoxHitOnTarget(Player, {Y: -1}, 11.2, 9)
            ai.pattern = s.box_on_player(0.0, -1.0, 11.2, 9.0);
            s.queue_attack(ai, 0, h, h);
        }
        s.set_cd(ci, Action::Burst, g::BURST_CD);
        s.consume_energy(ci, 14);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_BOL, arg: 0 }, 13);
        let t = if s.mod_active(ci, STATE) { &g::BURST_SKILL_STATE_FRAMES } else { &g::BURST_FRAMES };
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_C6_SKILL => {
                if s.chars[ci].cons < 6 {
                    return;
                }
                s.st_mut::<State>(ci).c6_stacks = 6;
                if !s.mod_active(ci, STATE) || s.mod_active(ci, C6_ICD) {
                    return;
                }
                s.add_status(ci, C6_ICD, g::C6_ICD, true);
                s.add_stat_mod(ci, C6_CR, st::CR, 0.1, g::C6_ICD, false);
                s.add_stat_mod(ci, C6_CD, st::CD, 0.7, g::C6_ICD, false);
            }
            TASK_HEAL => {
                let mult = if arg == 1 { g::SKILL_LUNGE_FULL_BOL_HEAL } else { g::SKILL_LUNGE_LOW_BOL_HEAL };
                let amt = s.hp_debt(ci) * mult;
                let bonus = s.stat(ci, st::HEAL);
                s.st_mut::<State>(ci).own_heal = true;
                s.heal(ci, ci, amt, bonus);
                s.st_mut::<State>(ci).own_heal = false;
            }
            TASK_C6 => {
                if s.mod_active(ci, STATE) {
                    c6(s, ci);
                }
            }
            TASK_SHOT => {
                let n = arg as usize;
                let (ai, bol) = (s.st::<State>(ci).shots[n], s.st::<State>(ci).shot_bol[n]);
                s.queue_attack(ai, CB_PARTICLE, 0, 0);
                arkhe_attack(s, ci);
                if bol {
                    s.modify_hp_debt_ratio(ci, g::SKILL_BOL_GAIN);
                }
            }
            TASK_BURST_BOL => {
                let lvl = s.talent(ci, 2);
                s.modify_hp_debt_ratio(ci, g::BURST_BOL[lvl]);
            }
            TASK_A1_STACK => {
                let (src, i) = (arg >> 2, (arg & 3) as usize);
                let st = s.st_mut::<State>(ci);
                if st.a1_stacks[i] == Some(src) {
                    st.a1_stacks[i] = None;
                }
            }
            TASK_A4_STACK => {
                let (src, i) = (arg >> 2, (arg & 3) as usize);
                let st = s.st_mut::<State>(ci);
                if st.a4_stacks[i] == Some(src) {
                    st.a4_stacks[i] = None;
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 2 * 60, true);
            s.queue_particle(1.0, Element::Electro, s.particle_delay[ci]);
        }
    }

    /// A4's CRIT Rate by its stacks.
    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == A4 {
            out[st::CR] += stack_count(&s.st::<State>(ci).a4_stacks) as f64 * g::A4_CRIT_BUFF;
            return true;
        }
        false
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A1 {
            // electro normal attacks and the burst: flat damage by her ATK
            // and the stacks, up to the cap (gcsim adds it and rejects the
            // mod)
            match ev.info.tag {
                AttackTag::Normal if ev.info.element == Element::Electro => {}
                AttackTag::ElementalBurst => {}
                _ => return,
            }
            let st = s.st::<State>(ci);
            let amt = (total_atk(&ev.stats) * st.a1_percent * stack_count(&st.a1_stacks) as f64).min(st.a1_cap);
            ev.info.flat_dmg += amt;
        } else if m.key == C4 && ev.info.tag == AttackTag::ElementalBurst {
            ev.stats[st::DMGP] += (s.hp_debt_ratio(ci) * 100.0 * 0.02).min(2.0);
        }
    }

    /// Her ReceiveHeal: in Night Watch, a heal not her own Impale the Night
    /// becomes Bond of Life (80%, all of it with A4), and she gets no HP.
    fn receive_heal(s: &mut Sim, ci: usize, caller: usize, amount: f64) -> Option<f64> {
        if !s.mod_active(ci, STATE) || (caller == ci && s.st::<State>(ci).own_heal) {
            return None;
        }
        let factor = if s.chars[ci].ascension >= 4 { 1.0 } else { g::HEALING_BOL };
        s.modify_hp_debt(ci, amount * factor);
        Some(0.0)
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // A1: electro reactions on the enemy (a hyperbloom on its core)
            Event::Reaction(r, ev) => {
                let hit = match r {
                    Reaction::Hyperbloom => true,
                    Reaction::ElectroCharged
                    | Reaction::LunarCharged
                    | Reaction::Superconduct
                    | Reaction::StellarConduct
                    | Reaction::Aggravate
                    | Reaction::Quicken
                    | Reaction::Overload
                    | Reaction::SwirlElectro
                    | Reaction::CrystallizeElectro => !ev.on_player,
                    _ => false,
                };
                if hit {
                    add_stack(s, ci, true);
                    s.add_mod(ci, Mod::owned(A1, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), g::CLORDINE_A1_BUFF_DURATION);
                }
            }
            // A4: a change of her Bond of Life while it was at 100% or more
            Event::HpDebt { on, change } if on == ci => {
                if s.st::<State>(ci).prev_hp_debt >= 1.0 && (-change).abs() >= g::TOLERANCE {
                    add_stack(s, ci, false);
                    s.add_mod(ci, Mod::owned(A4, ModKind::Stat, Owner::Char(ci as u8), st::NONE, false, true, -1), g::CLORDINE_A4_BUFF_DURATION);
                }
                let r = s.hp_debt_ratio(ci);
                s.st_mut::<State>(ci).prev_hp_debt = r;
            }
            // C1: her electro normal attack hits in Night Watch, once every
            // 1.2 s, two Nightwatch Shades
            Event::EnemyDamage(ev) => {
                if !s.mod_active(ci, STATE) || s.mod_active(ci, C1_ICD) {
                    return;
                }
                if ev.info.tag != AttackTag::Normal || ev.info.element != Element::Electro || ev.info.actor as usize != ci {
                    return;
                }
                s.add_status(ci, C1_ICD, g::C1_ICD, false);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_CLORINDE_ELEMENTAL_ART, Element::Electro, 25.0, g::C1_ATK_P);
                ai.abil = "Nightwatch Shade (C1)";
                ai.hitlag_factor = 0.01;
                ai.ignore_infusion = true;
                for &h in g::C1_HITMARKS {
                    // NewCircleHitOnTarget(Player, {Y: -3}, 4)
                    ai.pattern = s.circle_on_player(0.0, -3.0, 4.0);
                    s.queue_attack(ai, CB_PARTICLE, h, h);
                }
            }
            _ => {}
        }
    }

    /// `.clorinde.normal`: gcsim's NextNormalCounter, the Swift Hunt
    /// counter in Night Watch, otherwise the base one.
    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<crate::gcsl::Num, String>> {
        if fields[0] != "normal" {
            return None;
        }
        let n = if s.mod_active(ci, STATE) || !s.st::<State>(ci).s_mode { s.chars[ci].normal_counter } else { 0 };
        Some(Ok(crate::gcsl::Num::Int(n as i64 + 1)))
    }
}

fn stack_count(stacks: &[Option<i32>]) -> usize {
    stacks.iter().filter(|x| x.is_some()).count()
}

/// MultipleRefreshNoRemove.Add: an empty stack, or the oldest, starts now,
/// and ends in 15 s unless started again.
fn add_stack(s: &mut Sim, ci: usize, a1: bool) {
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    let stacks: &mut [Option<i32>] = if a1 { &mut st.a1_stacks } else { &mut st.a4_stacks };
    let mut idx = 0;
    for i in 0..stacks.len() {
        match stacks[i] {
            None => {
                idx = i;
                break;
            }
            Some(v) => {
                if stacks[idx].is_some_and(|w| v < w) {
                    idx = i;
                }
            }
        }
    }
    stacks[idx] = Some(f);
    let (id, dur) = if a1 { (TASK_A1_STACK, g::CLORDINE_A1_BUFF_DURATION) } else { (TASK_A4_STACK, g::CLORDINE_A4_BUFF_DURATION) };
    s.queue_char_task(ci, Task::Char { c: ci as u8, id, arg: f << 2 | idx as i32 }, dur);
}

/// A Swift Hunt shot in Night Watch: piercing below a full Bond of Life
/// (which it grows), a normal shot at 100% or more, with Surging Blade.
fn skill_attack(s: &mut Sim, ci: usize) -> ActInfo {
    let n = s.chars[ci].normal_counter as usize;
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_ENHANCED_NA[lvl]);
    ai.abil = PIERCING_NAMES[n];
    ai.ignore_infusion = true;
    let gain_bol;
    if s.hp_debt_ratio(ci) < 1.0 {
        // NewBoxHitOnTarget(PrimaryTarget, nil, 2, 14)
        ai.pattern = s.box_on_enemy(0.0, 0.0, 2.0, 14.0);
        gain_bol = true;
    } else {
        ai.abil = NORMAL_SHOT_NAMES[n];
        ai.mult = g::SKILL_NA[lvl];
        ai.pattern = s.circle_on_target(0.6);
        gain_bol = false;
    }
    let st = s.st_mut::<State>(ci);
    st.shots[n] = ai;
    st.shot_bol[n] = gain_bol;
    st.s_mode = true;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SHOT, arg: n as i32 }, g::SKILL_ATTACK_HITMARKS[n]);
    // AdvanceNormalIndex in Night Watch
    s.chars[ci].normal_counter = ((n + 1) % g::SKILL_HIT_NUM as usize) as i32;
    let atkspd = s.stat(ci, st::ATKSPD);
    let table = &g::SKILL_ATTACK_FRAMES[n];
    ActInfo::new(Frames::Attack { table, atkspd }, atk_spd_adjust(table[0], atkspd), atk_spd_adjust(g::SKILL_ATTACK_HITMARKS[n], atkspd), AnimState::Normal)
}

/// Surging Blade, once every 10 s.
fn arkhe_attack(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, ARKHE_ICD) {
        return;
    }
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, g::ARKHE_DAMAGE[lvl]);
    ai.abil = "Surging Blade";
    ai.hitlag_factor = 0.01;
    ai.def_halt = true;
    // NewCircleHitOnTarget(PrimaryTarget, nil, 4.5)
    ai.pattern = s.circle_on_target(4.5);
    s.queue_attack(ai, 0, g::ARKHE_HITMARK, g::ARKHE_HITMARK);
    s.add_status(ci, ARKHE_ICD, g::ARKHE_CD * 60, true);
}

/// Impale the Night, by her Bond of Life.
fn skill_dash(s: &mut Sim, ci: usize) -> ActInfo {
    s.chars[ci].normal_counter = 0;
    let ratio = s.hp_debt_ratio(ci);
    let lvl = s.talent(ci, 1);
    let lunge = |mult: f64, abil: &'static str| {
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Electro, 25.0, mult);
        ai.abil = abil;
        ai.hitlag_factor = 0.01;
        ai.ignore_infusion = true;
        ai
    };
    let h = g::SKILL_DASH_HITMARK;
    if ratio >= 1.0 {
        for _ in 0..3 {
            let mut ai = lunge(g::SKILL_LUNGE_FULL_BOL[lvl], "Impale the Night (100%+ BoL)");
            ai.pattern = s.circle_on_target(0.8);
            s.queue_attack(ai, CB_PARTICLE, h, h);
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_HEAL, arg: 1 }, g::SKILL_HEAL_FRAME);
        if s.chars[ci].cons >= 6 && s.st::<State>(ci).c6_stacks > 0 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6, arg: 0 }, 37);
        }
    } else if ratio.abs() < g::TOLERANCE {
        let mut ai = lunge(g::SKILL_LUNGE_NO_BOL[lvl], "Impale the Night (0% BoL)");
        ai.pattern = s.circle_on_target(0.6);
        s.queue_attack(ai, CB_PARTICLE, h, h);
    } else {
        let mut ai = lunge(g::SKILL_LUNGE_LOW_BOL[lvl], "Impale the Night (<100% BoL)");
        ai.pattern = s.circle_on_target(0.8);
        s.queue_attack(ai, CB_PARTICLE, h, h);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_HEAL, arg: 0 }, g::SKILL_HEAL_FRAME);
    }
    let t = &g::SKILL_DASH_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Skill as usize], AnimState::Skill)
}

/// C6's Glimbright Shade, once a second while its stacks last.
fn c6(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, C6_GLIMBRIGHT_ICD) {
        return;
    }
    s.st_mut::<State>(ci).c6_stacks -= 1;
    s.add_status(ci, C6_GLIMBRIGHT_ICD, 60, true);
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_CLORINDE_ELEMENTAL_ART, Element::Electro, 25.0, g::C6_GLIMBRIGHT_ATK_P as f64);
    ai.abil = "Glimbright Shade (C6)";
    ai.ignore_infusion = true;
    // NewCircleHitOnTarget(PrimaryTarget, nil, 8)
    ai.pattern = s.circle_on_target(8.0);
    s.queue_attack(ai, 0, 0, 0);
}
