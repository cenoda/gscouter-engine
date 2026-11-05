//! The anemo traveler (aetheranemo, lumineanemo), from gcsim's
//! internal/characters/traveler/common/anemo: normal and charged attacks
//! with A1's Slitting Wind, the skill (tap, and held for 1 to 6 ticks, with
//! the element it absorbs), the burst with its absorbed element, A4's heal
//! on a kill by the skill (damage mode), C2's energy recharge, C6's
//! resistance shred, and the story-quest buffs. gcsim's package has no
//! code for C1, C3, C4 and C5 beyond the talent levels.

use crate::chars::traveler::{self, Normals};
use crate::chars::CharState;
use crate::content::{Character, Event, Owner};
use crate::gamedata::chars::traveler_common_anemo as g;
use crate::sim::*;
use crate::types::*;

pub struct Traveler<const G: usize>;

/// Aether's and Lumine's states are distinct types holding the same.
pub struct State<const G: usize>(Inner);

pub struct Inner {
    story: crate::chars::traveler_common::Story,
    /// What the skill and the burst absorbed (None before they do), with
    /// the skill's ICD tag for it.
    e_absorb: Option<Element>,
    e_icd: IcdTag,
    e_area: Option<Pattern>,
    q_absorb: Option<Element>,
    q_area: Option<Pattern>,
    /// The burst's absorbed hit, snapshot as it is used.
    q_abs_snap: Option<AttackEvent>,
    /// The hold skill's absorbed hits, by kind: initial cutting, max
    /// cutting, storm.
    e_abs: [Option<AttackInfo>; 3],
}

const N: Normals = crate::traveler_normals!(g);

const BURST: ModKey = ModKey::new("amcburst");
const PRESS_PARTICLE_ICD: ModKey = ModKey::new(g::PRESS_PARTICLE_ICD_KEY);
const HOLD_PARTICLE_ICD: ModKey = ModKey::new(g::HOLD_PARTICLE_ICD_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const C2: ModKey = ModKey::new("amc-c2");

const CB_PRESS_PARTICLE: u16 = 1 << 0;
const CB_HOLD_PARTICLE: u16 = 1 << 1;
/// C6 on the element of the hit (anemo, or the one absorbed).
const CB_C6: u16 = 1 << 2;

// arg: the kind of absorbed hit (0, 1, 2)
const TASK_E_ABS: u8 = 0;
// arg: checks done << 8 | the checks to do
const TASK_E_CHECK: u8 = 1;
const TASK_Q_ABS: u8 = 2;
const TASK_Q_CHECK: u8 = 3;
const TASK_A4_HEAL: u8 = 4;

fn st(s: &Sim, ci: usize) -> &Inner {
    match &s.chars[ci].state {
        CharState::Aetheranemo(x) => &x.0,
        CharState::Lumineanemo(x) => &x.0,
        _ => unreachable!(),
    }
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut Inner {
    match &mut s.chars[ci].state {
        CharState::Aetheranemo(x) => &mut x.0,
        CharState::Lumineanemo(x) => &mut x.0,
        _ => unreachable!(),
    }
}

const ABSORB: [Element; 4] = [Element::Cryo, Element::Pyro, Element::Hydro, Element::Electro];

fn c6_key(e: Element) -> ModKey {
    match e {
        Element::Anemo => ModKey::new("amc-c6-anemo"),
        Element::Cryo => ModKey::new("amc-c6-cryo"),
        Element::Pyro => ModKey::new("amc-c6-pyro"),
        Element::Hydro => ModKey::new("amc-c6-hydro"),
        Element::Electro => ModKey::new("amc-c6-electro"),
        _ => unreachable!(),
    }
}

impl<const G: usize> Character for Traveler<G> {
    const KEY: &'static str = if G == 0 { "aetheranemo" } else { "lumineanemo" };
    type State = State<G>;

    fn new_state(cs: &CharSetup) -> State<G> {
        State(Inner { story: crate::chars::traveler_common::Story::new(cs), e_absorb: None, e_icd: icd::TAG_NONE, e_area: None, q_absorb: None, q_area: None, q_abs_snap: None, e_abs: [None; 3] })
    }

    fn init(s: &mut Sim, ci: usize) {
        crate::chars::traveler_common::story_buffs(s, ci);
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::TargetDied, Owner::Char(ci as u8));
        }
        if s.chars[ci].cons >= 2 {
            s.add_stat_mod(ci, C2, st::ER, 0.16, -1, false);
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        traveler::unsupported(a)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        traveler::attack(s, ci, G, &N, |s| a1(s, ci, G))
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        traveler::charge(s, ci, G, &N)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let mut ticks = 0;
        if p.get("hold") == Some(1) {
            ticks = 6;
        }
        if let Some(t) = p.get("hold_ticks").filter(|&t| t > 0) {
            ticks = t;
        }
        let ticks = ticks.min(6) as i32;
        if ticks == 0 {
            press(s, ci)
        } else {
            hold(s, ci, ticks)
        }
    }

    fn frames(gender: u8, hitmark: i16, next: Action) -> i32 {
        g::SKILL_HOLD_DELAY_FRAMES[gender as usize][next as usize] + hitmark as i32
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // 9 hits, the first at 94 and then every 30 frames
        let duration = g::BURST_HITMARKS[G] + 30 * 8;
        s.add_core_status(BURST, duration);
        let area = s.box_on_player(0.0, -1.5, 2.5, 2.5);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_ART_ANEMO, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST_DOT[lvl]);
        ai.abil = "Gust Surge";
        ai.pattern = s.box_on_target(0.0, -1.5, 3.0, 3.0);
        let c6 = if s.chars[ci].cons >= 6 { CB_C6 } else { 0 };
        let ev = s.snapshot_event(ai, c6);
        // the absorbed hit's element is set when it lands; the snapshot's
        // stats do not depend on it
        let mut abs = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::NoElement, 50.0, g::BURST_ABSORB_DOT[lvl]);
        abs.abil = "Gust Surge (Absorbed)";
        abs.pattern = s.box_on_target(0.0, -1.0, 2.5, 2.5);
        let abs = s.snapshot_event(abs, 0);
        let t = st_mut(s, ci);
        t.q_absorb = None;
        t.q_area = Some(area);
        t.q_abs_snap = Some(abs);
        for i in 0..9 {
            s.queue_attack_with_snap(ev, c6, 94 + 30 * i);
            s.add_task(Task::Char { c: ci as u8, id: TASK_Q_ABS, arg: 0 }, 94 + 30 * i);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_Q_CHECK, arg: duration / 18 }, 39);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 3);
        let t = &g::BURST_FRAMES[G];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_E_ABS => {
                let t = st(s, ci);
                let (Some(e), Some(mut ai)) = (t.e_absorb, t.e_abs[arg as usize]) else { return };
                ai.element = e;
                ai.icd_tag = t.e_icd;
                ai.pattern = match arg {
                    0 => s.circle_on_player(0.0, 1.2, 1.7),
                    1 => s.circle_on_player(0.0, 1.2, 3.6),
                    _ => s.fan_on_player(0.0, 0.0, 6.0, 100.0),
                };
                s.queue_attack(ai, 0, 0, 0);
            }
            // every 0.3 s until an element is absorbed or the checks run out
            TASK_E_CHECK => {
                let (done, max) = (arg >> 8, arg & 0xff);
                if done == max {
                    return;
                }
                let area = st(s, ci).e_area.unwrap();
                let e = s.absorb_check_in(area, &ABSORB);
                if e == Element::NoElement {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_E_CHECK, arg: (done + 1) << 8 | max }, 18);
                    return;
                }
                let t = st_mut(s, ci);
                t.e_absorb = Some(e);
                t.e_icd = match e {
                    Element::Cryo => icd::TAG_ELEMENTAL_ART_CRYO,
                    Element::Pyro => icd::TAG_ELEMENTAL_ART_PYRO,
                    Element::Electro => icd::TAG_ELEMENTAL_ART_ELECTRO,
                    _ => icd::TAG_ELEMENTAL_ART_HYDRO,
                };
            }
            TASK_Q_ABS => {
                let t = st(s, ci);
                let (Some(e), Some(mut ev)) = (t.q_absorb, t.q_abs_snap) else { return };
                ev.info.element = e;
                let c6 = if s.chars[ci].cons >= 6 { CB_C6 } else { 0 };
                s.queue_attack_with_snap(ev, c6, 0);
            }
            // arg: the checks left
            TASK_Q_CHECK => {
                if arg == 0 {
                    return;
                }
                let area = st(s, ci).q_area.unwrap();
                let e = s.absorb_check_in(area, &ABSORB);
                if e == Element::NoElement {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_Q_CHECK, arg: arg - 1 }, 18);
                    return;
                }
                st_mut(s, ci).q_absorb = Some(e);
            }
            TASK_A4_HEAL => s.heal_info(ci, Some(ci), true, 0.02, 0.0),
            _ => unreachable!(),
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if crate::chars::traveler_common::story_amount(&st(s, ci).story, m, out) {
            return true;
        }
        false
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if !ev.on_enemy() {
            return;
        }
        if cbs & CB_PRESS_PARTICLE != 0 && !s.mod_active(ci, PRESS_PARTICLE_ICD) {
            s.add_status(ci, PRESS_PARTICLE_ICD, 36, true);
            s.queue_particle(2.0, Element::Anemo, s.particle_delay[ci]);
        }
        if cbs & CB_HOLD_PARTICLE != 0 && !s.mod_active(ci, HOLD_PARTICLE_ICD) {
            s.add_status(ci, HOLD_PARTICLE_ICD, 36, true);
            let n = if s.rng.float64() < 0.33 { 4.0 } else { 3.0 };
            s.queue_particle(n, Element::Anemo, s.particle_delay[ci]);
        }
        // C6: the hit's element's resistance -20% for 10 s
        if cbs & CB_C6 != 0 {
            let e = ev.info.element;
            s.enemy_add_resist(c6_key(e), e, -0.20, 600);
        }
    }

    /// A4: a kill by the skill heals 2% of max HP each second for 5 s,
    /// once every 5 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::TargetDied(ev) = *e else { return };
        if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) || s.mod_active(ci, A4_ICD) {
            return;
        }
        s.add_status(ci, A4_ICD, 300, true);
        for i in 0..5 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A4_HEAL, arg: 0 }, (i + 1) * 60);
        }
    }
}

/// gcsim's SkillPress.
fn press(s: &mut Sim, ci: usize) -> ActInfo {
    let g_ = gender(s, ci);
    let hitmark = 34;
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL_INITIAL_STORM[lvl]);
    ai.abil = "Palm Vortex (Tap)";
    ai.pattern = s.fan_on_player(0.0, 0.0, 6.0, 100.0);
    s.queue_attack(ai, CB_PRESS_PARTICLE, hitmark, hitmark);
    s.set_cd_with_delay(ci, Action::Skill, 5 * 60, hitmark - 5);
    let t = &g::SKILL_PRESS_FRAMES[g_];
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
}

/// gcsim's SkillHold for 1 to 6 ticks.
fn hold(s: &mut Sim, ci: usize, ticks: i32) -> ActInfo {
    let g_ = gender(s, ci);
    let area = s.circle_on_player(0.0, 1.2, 3.0);
    let lvl = s.talent(ci, 1);
    let mut cut = AttackInfo::new(ci, AttackTag::ElementalArtHold, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL_INITIAL_CUTTING[lvl]);
    cut.abil = "Palm Vortex Initial Cutting (Hold)";
    let mut cut_abs = cut;
    cut_abs.abil = "Palm Vortex Initial Cutting Absorbed (Hold)";
    cut_abs.icd_tag = icd::TAG_NONE;
    cut_abs.element = Element::NoElement;
    cut_abs.mult = g::SKILL_INITIAL_CUTTING_ABSORB[lvl];
    let mut max_cut_abs = cut_abs;
    max_cut_abs.abil = "Palm Vortex Max Cutting Absorbed (Hold)";
    max_cut_abs.mult = g::SKILL_MAX_CUTTING_ABSORB[lvl];

    let first = 31;
    let mut hitmark = first;
    for i in 0..ticks {
        let mut ai = cut;
        ai.pattern = s.circle_on_player(0.0, 1.2, 1.7);
        s.queue_attack(ai, 0, hitmark, hitmark);
        s.add_task(Task::Char { c: ci as u8, id: TASK_E_ABS, arg: if i > 1 { 1 } else { 0 } }, hitmark);
        hitmark += 15;
        if i == 1 {
            cut.mult = g::SKILL_MAX_CUTTING[lvl];
            cut.abil = "Palm Vortex Max Cutting (Hold)";
            // 5 frames more from initial to max
            hitmark += 5;
        }
    }
    // a tick back, then 5 frames on, for the storm
    hitmark = hitmark - 15 + 5;
    let mut storm = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL_INITIAL_STORM[lvl]);
    storm.abil = "Palm Vortex Initial Storm (Hold)";
    let mut storm_abs = storm;
    storm_abs.abil = "Palm Vortex Initial Storm Absorbed (Hold)";
    storm_abs.icd_tag = icd::TAG_NONE;
    storm_abs.element = Element::NoElement;
    storm_abs.mult = g::SKILL_INITIAL_STORM_ABSORB[lvl];
    let particle;
    // a max storm from 2 ticks on
    if ticks >= 2 {
        storm.mult = g::SKILL_MAX_STORM[lvl];
        storm.abil = "Palm Vortex Max Storm (Hold)";
        storm_abs.mult = g::SKILL_MAX_STORM_ABSORB[lvl];
        storm_abs.abil = "Palm Vortex Max Storm Absorbed (Hold)";
        particle = CB_HOLD_PARTICLE;
        s.set_cd_with_delay(ci, Action::Skill, 8 * 60, hitmark - 5);
    } else {
        particle = CB_PRESS_PARTICLE;
        s.set_cd_with_delay(ci, Action::Skill, 5 * 60, hitmark - 5);
    }
    storm.pattern = s.fan_on_player(0.0, 0.0, 6.0, 100.0);
    s.queue_attack(storm, particle, hitmark, hitmark);
    s.add_task(Task::Char { c: ci as u8, id: TASK_E_ABS, arg: 2 }, hitmark);
    let t = st_mut(s, ci);
    t.e_absorb = None;
    t.e_icd = icd::TAG_NONE;
    t.e_area = Some(area);
    t.e_abs = [Some(cut_abs), Some(max_cut_abs), Some(storm_abs)];
    // absorbing starts after the first tick
    s.add_task(Task::Char { c: ci as u8, id: TASK_E_CHECK, arg: hitmark / 18 }, first + 1);
    let d = &g::SKILL_HOLD_DELAY_FRAMES[g_];
    ActInfo::new(Frames::Char { id: g_ as u8, arg: hitmark as i16 }, d[0] + hitmark, d[Action::Dash as usize] + hitmark, AnimState::Skill)
}

fn gender(s: &Sim, ci: usize) -> usize {
    (s.chars[ci].body == Body::Girl) as usize
}

/// A1: the last hit of the combo also sends a wind blade.
fn a1(s: &mut Sim, ci: usize, gender: usize) {
    if s.chars[ci].ascension < 1 || s.chars[ci].normal_counter != s.chars[ci].normal_hit_num - 1 {
        return;
    }
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_POLE_EXTRA_ATTACK, Element::Anemo, 25.0, 0.6);
    ai.abil = "Slitting Wind (A1)";
    ai.pattern = s.circle_on_target(1.0);
    s.queue_attack_at(ci, ai, 0, g::A1_HITMARK[gender]);
}
