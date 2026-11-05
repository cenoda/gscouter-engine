//! Venti, from gcsim's internal/characters/venti: normal attacks, aimed
//! shots (with C1's extra arrows), Skyward Sonnet pressed and held (with
//! C2's RES shred), the high plunge gcsim allows after the skill, Wind's
//! Grand Ode with its absorbed element, A4's energy, C4 and C6. A1 (the
//! upcurrent) and C2's airborne part are not in gcsim.
//!
//! gcsim's high plunge errors unless the last action was his skill with
//! `hold` 0 (`lastAct.Param["hold"] != 0` is its error), and it does not
//! read the player's airborne state; the engine does the same.

use crate::content::{Character, Event};
use crate::gamedata::chars::venti as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// Where the burst stands: 5 in front of the player as it was used.
    q_pos: (f64, f64),
    /// The element the burst absorbed (NoElement before it does).
    q_absorb: Element,
    /// The burst's anemo tick and its absorbed tick, snapshot 104 frames
    /// after the burst.
    q_tick: Option<AttackEvent>,
    q_tick_absorb: Option<AttackEvent>,
    /// Whether his last skill was held (gcsim reads the last action's
    /// params for the high plunge).
    last_skill_hold: bool,
}

const C4: ModKey = ModKey::new("venti-c4");
const C2_ANEMO: ModKey = ModKey::new("venti-c2-anemo");
const C2_PHYS: ModKey = ModKey::new("venti-c2-phys");
const C6_ANEMO: ModKey = ModKey::new("venti-c6-anemo");
const C6_PYRO: ModKey = ModKey::new("venti-c6-pyro");
const C6_HYDRO: ModKey = ModKey::new("venti-c6-hydro");
const C6_ELECTRO: ModKey = ModKey::new("venti-c6-electro");
const C6_CRYO: ModKey = ModKey::new("venti-c6-cryo");

const CB_C2: u16 = 1 << 0;
const CB_PARTICLE_3: u16 = 1 << 1;
const CB_PARTICLE_4: u16 = 1 << 2;
const CB_C6_ANEMO: u16 = 1 << 3;
const CB_C6_ABSORB: u16 = 1 << 4;

const TASK_SNAP: u8 = 0;
const TASK_TICK: u8 = 1;
// arg: the checks done so far
const TASK_ABSORB_CHECK: u8 = 2;
const TASK_A4: u8 = 3;

const NORMAL_NAMES: [&str; 6] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4", "Normal 5"];

/// gcsim's absorb checks: every 18 frames from the 4th tick, at most
/// (480 - 24*4) / 18 times.
const ABSORB_CHECKS: i32 = (480 - 24 * 4) / 18;

impl Character for Char {
    const KEY: &'static str = "venti";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { q_pos: (0.0, 0.0), q_absorb: Element::NoElement, q_tick: None, q_tick_absorb: None, last_skill_hold: false }
    }

    fn init(s: &mut Sim, ci: usize) {
        // C4: OnParticleReceived
        if s.chars[ci].cons >= 4 {
            s.subscribe(EventKind::ParticleReceived, Owner::Char(ci as u8));
        }
    }

    /// gcsim reads travel on his normal attacks, hold (0 or 1), travel and
    /// weakspot on his aimed shots, hold on his skill and nothing on his
    /// burst and high plunge; he has no charged attack or low plunge.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let bad = |known: &[&str]| p.0.iter().find(|(k, _)| !known.iter().any(|n| k == *n)).map(|(k, _)| format!("{name} param {k}"));
        match a {
            Action::Attack => bad(&["travel"]),
            Action::Aim => {
                if !matches!(p.get("hold"), None | Some(0) | Some(1)) {
                    return Some("aim[hold]".into());
                }
                bad(&["hold", "travel", "weakspot"])
            }
            Action::Skill => bad(&["hold"]),
            Action::Burst | Action::HighPlunge => bad(&[]),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::HighPlunge {
            let last = &s.player.last;
            if last.char as usize != ci || last.action != Action::Skill || s.st::<State>(ci).last_skill_hold {
                return Some("high_plunge should be preceded by hold skill".into());
            }
            return None;
        }
        crate::content::template_invalid(s, a)
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            // NewBoxHit(Player, PrimaryTarget, {Y: -0.5}, 0.1, 1)
            ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
            let h = g::ATTACK_HITMARKS[n][i];
            s.queue_attack(ai, 0, h, h + travel);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_VENTI, Element::Anemo, 25.0, g::FULLAIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        ai.deployable = true;
        // its hitlag applies on weak spot hits only (HitlagOnHeadshotOnly)
        if p.get("weakspot") == Some(1) {
            ai.weak_point = true;
            ai.hitlag_halt = 0.12 * 60.0;
        }
        let mut c1_abil = "Fully-Charged Aimed Shot (C1)";
        if hold < 1 {
            ai.abil = "Aimed Shot";
            c1_abil = "Aimed Shot (C1)";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        let h = g::AIMED_HITMARKS[hold];
        // NewBoxHit(Player, PrimaryTarget, {Y: -0.5}, 0.1, 1)
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        s.queue_attack(ai, 0, h, h + travel);
        // C1: two more arrows of a third of its DMG
        if s.chars[ci].cons >= 1 {
            ai.abil = c1_abil;
            ai.mult /= 3.0;
            for _ in 0..2 {
                ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
                s.queue_attack(ai, 0, h, h + travel);
            }
        }
        let table = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Table(table), table[0], h, AnimState::Aim)
    }

    /// Skyward Sonnet: pressed, a circle of 3 on the enemy; held, a circle
    /// of 6 on the player.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(0) != 0;
        s.st_mut::<State>(ci).last_skill_hold = hold;
        let lvl = s.talent(ci, 1);
        let c2 = if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 50.0, g::SKILL_PRESS[lvl]);
        let (cd, cd_start, h, cb, table, queue_after) = if hold {
            ai.abil = "Skyward Sonnett (Hold)";
            ai.mult = g::SKILL_HOLD[lvl];
            ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
            (900, 34, 74, CB_PARTICLE_4, &g::SKILL_HOLD_FRAMES, g::SKILL_HOLD_FRAMES[Action::HighPlunge as usize])
        } else {
            ai.abil = "Skyward Sonnett (Press)";
            ai.pattern = s.circle_on_target(3.0);
            (360, 21, 51, CB_PARTICLE_3, &g::SKILL_PRESS_FRAMES, g::SKILL_PRESS_FRAMES[Action::Dash as usize])
        };
        s.queue_attack(ai, c2 | cb, 0, h);
        s.set_cd_with_delay(ci, Action::Skill, cd, cd_start);
        ActInfo::new(Frames::Table(table), table[0], queue_after, AnimState::Skill)
    }

    fn high_plunge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.player.airborne = Airborne::Grounded;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::HIGH_PLUNGE[lvl]);
        ai.abil = "High Plunge";
        ai.ignore_infusion = true;
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.5);
        let h = g::HIGH_PLUNGE_HITMARK;
        s.queue_attack(ai, 0, h, h);
        let t = &g::HIGH_PLUNGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Plunge)
    }

    /// Wind's Grand Ode: 20 anemo ticks every 24 frames from 106, a check
    /// for an element to absorb every 18 frames from the 4th tick, and 15
    /// absorbed ticks from the check that finds one.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let q_pos = s.point_from_player(0.0, 5.0);
        let st = s.st_mut::<State>(ci);
        st.q_absorb = Element::NoElement;
        st.q_pos = q_pos;
        s.add_task(Task::Char { c: ci as u8, id: TASK_SNAP, arg: 0 }, 104);
        for i in 0..20 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg: 0 }, 106 + 24 * i);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_ABSORB_CHECK, arg: 0 }, 106 + 24 * 3);
        if s.chars[ci].ascension >= 4 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, 480 + g::BURST_START);
        }
        s.set_cd_with_delay(ci, Action::Burst, 15 * 60, 81);
        s.consume_energy(ci, 84);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SNAP => {
                let lvl = s.talent(ci, 2);
                let (x, y) = s.st::<State>(ci).q_pos;
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST_ANEMO, icd::GROUP_VENTI, Element::Anemo, 25.0, g::BURST_DOT[lvl]);
                ai.abil = "Wind's Grand Ode";
                ai.pattern = Pattern::Circle { x, y, r: 4.0 };
                let c6 = if s.chars[ci].cons >= 6 { CB_C6_ANEMO } else { 0 };
                let tick = s.snapshot_event(ai, c6);
                // the absorbed tick's element and ICD tag are set when it
                // absorbs; the snapshot's stats do not depend on them
                let mut abs = ai;
                abs.abil = "Wind's Grand Ode (Absorbed)";
                abs.mult = g::BURST_ABSORB_DOT[lvl];
                abs.element = Element::NoElement;
                abs.pattern = Pattern::Circle { x, y, r: 6.0 };
                let c6 = if s.chars[ci].cons >= 6 { CB_C6_ABSORB } else { 0 };
                let abs = s.snapshot_event(abs, c6);
                let st = s.st_mut::<State>(ci);
                st.q_tick = Some(tick);
                st.q_tick_absorb = Some(abs);
            }
            TASK_TICK => {
                let ev = s.st::<State>(ci).q_tick.unwrap();
                let cbs = ev.callbacks;
                s.queue_attack_with_snap(ev, cbs, 0);
            }
            TASK_ABSORB_CHECK => {
                if arg == ABSORB_CHECKS {
                    return;
                }
                // a 2.5 by 2.5 box 1 behind the burst, in the default
                // direction (NewBoxHitOnTarget on a point)
                let (x, y) = s.st::<State>(ci).q_pos;
                let area = Pattern::Box { x, y: y - 1.0, w: 2.5, h: 2.5, dx: DEFAULT_DIR.0, dy: DEFAULT_DIR.1 };
                let e = s.absorb_check_in(area, &[Element::Pyro, Element::Hydro, Element::Electro, Element::Cryo]);
                if e == Element::NoElement {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_ABSORB_CHECK, arg: arg + 1 }, 18);
                    return;
                }
                let st = s.st_mut::<State>(ci);
                st.q_absorb = e;
                let mut ev = st.q_tick_absorb.unwrap();
                ev.info.element = e;
                ev.info.icd_tag = match e {
                    Element::Pyro => icd::TAG_ELEMENTAL_BURST_PYRO,
                    Element::Hydro => icd::TAG_ELEMENTAL_BURST_HYDRO,
                    Element::Electro => icd::TAG_ELEMENTAL_BURST_ELECTRO,
                    _ => icd::TAG_ELEMENTAL_BURST_CRYO,
                };
                let cbs = ev.callbacks;
                for i in 0..15 {
                    s.queue_attack_with_snap(ev, cbs, i * 24);
                }
            }
            // A4: 15 energy to him, and to the characters of the absorbed
            // element
            TASK_A4 => {
                s.add_energy(ci, 15.0);
                let e = s.st::<State>(ci).q_absorb;
                if e == Element::NoElement {
                    return;
                }
                for i in 0..s.chars.len() {
                    if s.chars[i].element == e {
                        s.add_energy(i, 15.0);
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        // C2: -12% anemo and physical RES for 10 s
        if cbs & CB_C2 != 0 {
            s.enemy_add_resist(C2_ANEMO, Element::Anemo, -0.12, 600);
            s.enemy_add_resist(C2_PHYS, Element::Physical, -0.12, 600);
        }
        // the skill's particles, once per cast: one enemy, hit once
        if cbs & CB_PARTICLE_3 != 0 {
            s.queue_particle(3.0, Element::Anemo, s.particle_delay[ci]);
        }
        if cbs & CB_PARTICLE_4 != 0 {
            s.queue_particle(4.0, Element::Anemo, s.particle_delay[ci]);
        }
        // C6: -20% RES to the tick's element for 10 s
        if cbs & CB_C6_ANEMO != 0 {
            s.enemy_add_resist(C6_ANEMO, Element::Anemo, -0.20, 600);
        }
        if cbs & CB_C6_ABSORB != 0 {
            let e = ev.info.element;
            let key = match e {
                Element::Pyro => C6_PYRO,
                Element::Hydro => C6_HYDRO,
                Element::Electro => C6_ELECTRO,
                _ => C6_CRYO,
            };
            s.enemy_add_resist(key, e, -0.20, 600);
        }
    }

    /// C4: 25% anemo DMG for 10 s when a particle reaches him on the field.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::ParticleReceived = *e else { return };
        if s.player.active as usize != ci {
            return;
        }
        s.add_stat_mod(ci, C4, st::ANEMOP, 0.25, 600, true);
    }
}
