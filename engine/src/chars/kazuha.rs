//! Kaedehara Kazuha, from gcsim's internal/characters/kaedeharakazuha:
//! normal and charged attacks, the skill pressed and held with its plunge,
//! the burst with its absorbed element, A1, A4 and every constellation.
//! Plunges other than after the skill need Xianyun, whom the engine does
//! not implement.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::kazuha as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    a1_absorb: Element,
    /// The params of the last skill: held, glide cancelled.
    skill_hold: bool,
    skill_glide: bool,
    q_absorb: Element,
    q_field_src: i32,
    q_tick: AttackEvent,
    q_tick_absorb: AttackEvent,
    /// A4's per-element guard: no second proc in the same frame.
    a4_icd: [i32; 4],
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST_STATUS: ModKey = ModKey::new(g::BURST_STATUS);
const C2: ModKey = ModKey::new("kazuha-c2");
const C6_INFUSION: ModKey = ModKey::new("kazuha-c6-infusion");
const C6_DMG: ModKey = ModKey::new("kazuha-c6-dmgup");
const A4: [ModKey; 4] = [ModKey::new("kazuha-a4-pyro"), ModKey::new("kazuha-a4-hydro"), ModKey::new("kazuha-a4-electro"), ModKey::new("kazuha-a4-cryo")];

const CB_PARTICLE_3: u16 = 1 << 0;
const CB_PARTICLE_4: u16 = 1 << 1;

// arg: normal counter * 8 + hit
const TASK_NORMAL: u8 = 0;
// arg: count * 16 + max count
const TASK_A1_CHECK: u8 = 1;
const TASK_Q_CHECK: u8 = 2;
const TASK_Q_SNAPSHOT: u8 = 3;
const TASK_Q_START: u8 = 4;
const TASK_Q_TICK: u8 = 5;
// arg: the field's frame
const TASK_C2: u8 = 6;
const TASK_C6: u8 = 7;

/// The elements Chihayaburu and the burst absorb, in gcsim's order.
const ABSORB: [Element; 4] = [Element::Pyro, Element::Hydro, Element::Electro, Element::Cryo];

fn absorb_check(s: &Sim) -> Element {
    s.absorb_check(&ABSORB)
}

impl Character for Char {
    const KEY: &'static str = "kaedeharakazuha";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        let empty = AttackEvent::new(AttackInfo::new(0, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, 0.0), 0);
        State {
            a1_absorb: Element::NoElement,
            skill_hold: false,
            skill_glide: false,
            q_absorb: Element::NoElement,
            q_field_src: 0,
            q_tick: empty,
            q_tick_absorb: empty,
            a4_icd: [-1; 4],
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::Reaction, Owner::Char(ci as u8));
        }
        s.utility_dash = Some(ci as u8);
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst | Action::HighPlunge => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        match a {
            Action::HighPlunge if s.player.last.action != Action::Skill => Some("high_plunge can only be used while airborne".into()),
            Action::HighPlunge if s.st::<State>(ci).skill_glide => Some("only plunge after skill without glide cancel".into()),
            _ => crate::content::template_invalid(s, a),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        for (i, &h) in g::ATTACK_HITMARKS[n].iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: (n * 8 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::CHARGE.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = ["Charge 0", "Charge 1"][i];
            s.queue_attack(ai, 0, g::CHARGE_HITMARKS[i], g::CHARGE_HITMARKS[i]);
        }
        ActInfo::new(Frames::Table(&g::CHARGE_FRAMES), g::CHARGE_FRAMES[0], *g::CHARGE_HITMARKS.last().unwrap(), AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(0) != 0;
        let glide = p.get("glide_cancel").unwrap_or(0).clamp(0, 1) as usize;
        let st = s.st_mut::<State>(ci);
        st.a1_absorb = Element::NoElement;
        st.skill_hold = hold;
        st.skill_glide = glide == 1;
        let lvl = s.talent(ci, 1);
        let (c1, c6) = (s.chars[ci].cons >= 1, s.chars[ci].cons >= 6);
        let (ai, cb, hitmark, cd, cd_start, frames, shortest) = if !hold {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL[lvl]);
            ai.abil = "Chihayaburu (Press)";
            let shortest = if glide == 1 { Action::Jump } else { Action::HighPlunge };
            (ai, CB_PARTICLE_3, g::SKILL_PRESS_HITMARK, if c1 { 324 } else { 360 }, g::SKILL_PRESS_CD_START, &g::SKILL_PRESS_FRAMES[glide], shortest)
        } else {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 50.0, g::SKILL_HOLD[lvl]);
            ai.abil = "Chihayaburu (Hold)";
            let shortest = if glide == 1 { Action::Swap } else { Action::HighPlunge };
            (ai, CB_PARTICLE_4, g::SKILL_HOLD_HITMARK, if c1 { 486 } else { 540 }, g::SKILL_HOLD_CD_START, &g::SKILL_HOLD_FRAMES[glide], shortest)
        };
        s.queue_attack(ai, cb, 0, hitmark);
        if s.chars[ci].ascension >= 1 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_A1_CHECK, arg: hitmark / 6 }, 1);
        }
        if c6 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_C6, arg: 0 }, hitmark);
        }
        s.set_cd_with_delay(ci, Action::Skill, cd, cd_start);
        ActInfo::new(Frames::Table(frames), frames[0], frames[shortest as usize], AnimState::Skill)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let (hitmark, frames) = if s.st::<State>(ci).skill_hold { (g::PLUNGE_HOLD_HITMARK, &g::PLUNGE_HOLD_FRAMES) } else { (g::PLUNGE_PRESS_HITMARK, &g::PLUNGE_PRESS_FRAMES) };
        let lvl = s.talent(ci, 0);
        if p.get("collision").unwrap_or(0) > 0 {
            let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 0.0, g::COLLISION[lvl]);
            ai.abil = "Plunge Collision";
            ai.ignore_infusion = true;
            s.queue_attack(ai, 0, hitmark - 6, hitmark - 6);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::HIGH_PLUNGE[lvl]);
        ai.abil = "High Plunge";
        ai.blunt = true;
        ai.poise = 150.0;
        ai.ignore_infusion = true;
        s.queue_attack(ai, 0, hitmark, hitmark);
        // A1: the absorbed element
        let absorbed = s.st::<State>(ci).a1_absorb;
        if absorbed != Element::NoElement {
            let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, absorbed, 25.0, 2.0);
            ai.abil = "Soumon Swordsmanship (A1)";
            ai.blunt = true;
            ai.poise = 20.0;
            ai.ignore_infusion = true;
            s.queue_attack(ai, 0, hitmark - 1, hitmark - 1);
            s.st_mut::<State>(ci).a1_absorb = Element::NoElement;
        }
        ActInfo::new(Frames::Table(frames), frames[0], frames[Action::Dash as usize], AnimState::Plunge)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.st_mut::<State>(ci).q_absorb = Element::NoElement;
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 50.0, g::BURST_SLASH[lvl]);
        ai.abil = "Kazuha Slash";
        ai.hitlag_halt = 0.05 * 60.0;
        ai.hitlag_factor = 0.05;
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.add_task(Task::Char { c: ci as u8, id: TASK_Q_CHECK, arg: 310 / 18 }, g::BURST_HITMARK - 1);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_Q_SNAPSHOT, arg: 0 }, g::BURST_HITMARK - 1);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_Q_START, arg: 0 }, g::BURST_HITMARK + 1);
        if s.chars[ci].cons >= 1 {
            s.reset_cd(ci, Action::Skill);
        }
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 4);
        ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_FRAMES[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = ((arg / 8) as usize, (arg % 8) as usize);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.hitlag_factor = g::ATTACK_HITLAG_FACTOR[n][i];
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.def_halt = g::ATTACK_DEF_HALT[n][i];
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_A1_CHECK | TASK_Q_CHECK => {
                // arg: the checks left
                if arg == 0 {
                    return;
                }
                let e = absorb_check(s);
                let st = s.st_mut::<State>(ci);
                if id == TASK_A1_CHECK {
                    st.a1_absorb = e;
                } else {
                    st.q_absorb = e;
                }
                if e == Element::NoElement {
                    s.add_task(Task::Char { c: ci as u8, id, arg: arg - 1 }, if id == TASK_A1_CHECK { 6 } else { 18 });
                }
            }
            TASK_Q_SNAPSHOT => {
                // the ticks snapshot right before the slash
                let lvl = s.talent(ci, 2);
                let mut dot = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST_DOT[lvl]);
                dot.abil = "Kazuha Slash (DoT)";
                dot.hitlag_factor = 0.05;
                let mut absorb = dot;
                absorb.abil = "Kazuha Slash (Absorb DoT)";
                absorb.mult = g::BURST_ELE_DOT[lvl];
                absorb.element = Element::NoElement;
                let (tick, tick_absorb) = (s.snapshot_event(dot, 0), s.snapshot_event(absorb, 0));
                let st = s.st_mut::<State>(ci);
                st.q_tick = tick;
                st.q_tick_absorb = tick_absorb;
                s.add_core_status(BURST_STATUS, (g::BURST_FIRST_TICK - (g::BURST_HITMARK - 1)) + 117 * 4);
                if s.chars[ci].cons >= 2 {
                    let f = s.f;
                    s.st_mut::<State>(ci).q_field_src = f;
                    c2(s, ci, f);
                }
            }
            TASK_Q_START => {
                for i in 0..5 {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_Q_TICK, arg: 0 }, (g::BURST_FIRST_TICK - (g::BURST_HITMARK + 1)) + 117 * i);
                }
                if s.chars[ci].cons >= 6 {
                    c6(s, ci);
                }
            }
            TASK_Q_TICK => {
                let st = s.st::<State>(ci);
                let (absorbed, tick, mut tick_absorb) = (st.q_absorb, st.q_tick, st.q_tick_absorb);
                if absorbed != Element::NoElement {
                    tick_absorb.info.element = absorbed;
                    s.queue_attack_with_snap(tick_absorb, 0, 0);
                }
                s.queue_attack_with_snap(tick, 0, 0);
            }
            TASK_C2 => c2(s, ci, arg),
            TASK_C6 => c6(s, ci),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if s.mod_active(ci, PARTICLE_ICD) {
            return;
        }
        s.add_status(ci, PARTICLE_ICD, 12, true);
        let count = if cbs & CB_PARTICLE_4 != 0 { 4.0 } else { 3.0 };
        s.queue_particle(count, Element::Anemo, s.particle_delay[ci]);
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        // C6
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            ev.stats[st::DMGP] += 0.002 * s.stat(ci, st::EM);
        }
    }

    fn mod_amount(_: &Sim, _: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        // C2
        out[st::EM] += 200.0;
        true
    }

    /// A4: a swirl by Kazuha gives the party a DMG bonus of the swirled
    /// element from his EM, for 8 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        let (i, stat) = match r {
            Reaction::SwirlPyro => (0, st::PYROP),
            Reaction::SwirlHydro => (1, st::HYDROP),
            Reaction::SwirlElectro => (2, st::ELECTROP),
            Reaction::SwirlCryo => (3, st::CRYOP),
            _ => return,
        };
        if ev.info.actor as usize != ci {
            return;
        }
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        if f < st.a4_icd[i] {
            return;
        }
        st.a4_icd[i] = f + 1;
        let dmg = 0.0004 * s.stat_ex(ci, st::EM, true);
        for c in 0..s.chars.len() {
            s.add_mod(c, Mod::stat(A4[i], stat, dmg, true, true, -1), 60 * 8);
        }
    }
}

/// C2: every half second while the field lasts, EM +200 to the active
/// character and to Kazuha, for a second.
fn c2(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).q_field_src != src || s.core_status(g::BURST_STATUS) == 0 {
        return;
    }
    s.add_task(Task::Char { c: ci as u8, id: TASK_C2, arg: src }, 30);
    let active = s.player.active as usize;
    s.add_mod(active, Mod::owned(C2, ModKind::Stat, Owner::Char(ci as u8), st::EM, false, true, -1), 60);
    if active != ci {
        s.add_mod(ci, Mod::owned(C2, ModKind::Stat, Owner::Char(ci as u8), st::EM, false, true, -1), 60);
    }
}

/// C6: anemo infusion, and DMG from EM, for 5 s.
fn c6(s: &mut Sim, ci: usize) {
    s.add_infusion(ci, C6_INFUSION, Element::Anemo, 60 * 5, true, &[AttackTag::Normal, AttackTag::Extra, AttackTag::Plunge]);
    s.add_mod(ci, Mod::owned(C6_DMG, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 60 * 5);
}
