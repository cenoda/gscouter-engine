//! Zhongli's hold skill, stele and Jade Shield, from gcsim's
//! internal/characters/zhongli. The rotation uses no other ability.

use crate::content::Character;
use crate::gcsimdata::chars::zhongli as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    stele_count: i32,
    max_stele: i32,
    /// The snapshot of the stele's resonance ticks.
    stele_snap: AttackEvent,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const STELE: ModKey = ModKey::new("zhongli-stele");
const JADE_SHIELD: ModKey = ModKey::new("zhongli-jade-shield");
/// The Jade Shield shreds every element; these are the ones this team's
/// attacks have.
const SHRED: [(ModKey, Element); 4] = [
    (ModKey::new("zhongli-pyro"), Element::Pyro),
    (ModKey::new("zhongli-hydro"), Element::Hydro),
    (ModKey::new("zhongli-geo"), Element::Geo),
    (ModKey::new("zhongli-physical"), Element::Physical),
];

const CB_PARTICLE: u16 = 1 << 0;

const TASK_NEW_STELE: u8 = 0;
const TASK_JADE_SHIELD: u8 = 1;
const TASK_SHRED: u8 = 2;
const TASK_RESONANCE: u8 = 3;

impl Character for Char {
    const KEY: &'static str = "zhongli";
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        State {
            stele_count: 0,
            max_stele: if cs.cons >= 1 { 2 } else { 1 },
            stele_snap: AttackEvent::new(AttackInfo::new(0, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 0.0, 0.0), 0),
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::Skill if p.get("hold").unwrap_or(0) > 0 => None,
            Action::Skill => Some("skill (press)".into()),
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let shielded = || Some(Ok(Num::Int(s.shields.iter().any(|x| x.owner as usize == ci && x.key == JADE_SHIELD) as i64)));
        match (fields[0].as_str(), fields.get(1).map(|f| f.as_str())) {
            ("shielded", _) | ("tags", Some("shielded")) => shielded(),
            _ => None,
        }
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL_HOLD[lvl]);
        ai.abil = "Stone Stele (Hold)";
        ai.flat_dmg = a4_skill(s, ci);
        s.queue_attack(ai, 0, 0, g::SKILL_HOLD_HITMARK);
        let st = s.st::<State>(ci);
        if st.stele_count < st.max_stele {
            s.add_task(Task::Char { c: ci as u8, id: TASK_NEW_STELE, arg: 0 }, g::SKILL_HOLD_HITMARK);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_JADE_SHIELD, arg: 0 }, g::SKILL_HOLD_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 720, 47);
        ActInfo::new(Frames::Table(&g::SKILL_HOLD_FRAMES), g::SKILL_HOLD_FRAMES[0], g::SKILL_HOLD_FRAMES[Action::Dash as usize], AnimState::Skill)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NEW_STELE => new_stele(s, ci),
            TASK_JADE_SHIELD => jade_shield(s, ci),
            TASK_SHRED => shred(s, ci, arg),
            TASK_RESONANCE => resonance(s, ci, arg),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16) {
        if cbs & CB_PARTICLE != 0 {
            particle_cb(s, ci);
        }
    }

    fn construct_expired(s: &mut Sim, ci: usize, _: ModKey) {
        let st = s.st_mut::<State>(ci);
        if st.stele_count > 0 {
            st.stele_count -= 1;
        }
    }
}

fn a4_skill(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 4 {
        return 0.0;
    }
    0.019 * s.max_hp(ci)
}

fn stele(s: &Sim, ci: usize) -> Option<&Construct> {
    s.constructs.iter().find(|c| c.owner as usize == ci && c.key == STELE)
}

fn new_stele(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 1);
    let flat = a4_skill(s, ci);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 50.0, g::SKILL[lvl]);
    ai.abil = "Stone Stele (Initial)";
    ai.flat_dmg = flat;
    s.queue_attack(ai, CB_PARTICLE, 0, 0);

    let f = s.f;
    let had = stele(s, ci).is_some();
    let st = s.st_mut::<State>(ci);
    if had && st.stele_count == st.max_stele {
        // replaced: the old one is destroyed
        if st.stele_count > 0 {
            st.stele_count -= 1;
        }
    }
    st.stele_count = 1;
    s.constructs.retain(|c| !(c.owner as usize == ci && c.key == STELE));
    s.constructs.push(Construct { owner: ci as u8, key: STELE, src: f, expiry: f + 1860 });

    let mut tick = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL_TICK[lvl]);
    tick.abil = "Stone Stele (Tick)";
    tick.flat_dmg = flat;
    s.st_mut::<State>(ci).stele_snap = s.snapshot_event(tick, 0);
    s.add_task(Task::Char { c: ci as u8, id: TASK_RESONANCE, arg: f }, 120);
}

fn resonance(s: &mut Sim, ci: usize, src: i32) {
    if stele(s, ci).map(|c| c.src) != Some(src) {
        return;
    }
    let ev = s.st::<State>(ci).stele_snap;
    s.queue_attack_with_snap(ev, CB_PARTICLE, 0);
    s.add_task(Task::Char { c: ci as u8, id: TASK_RESONANCE, arg: src }, 120);
}

fn particle_cb(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, PARTICLE_ICD) {
        return;
    }
    s.add_status(ci, PARTICLE_ICD, 90, true);
    if s.rng.float64() < 0.5 {
        s.queue_particle(1.0, Element::Geo, s.particle_delay[ci]);
    }
}

fn jade_shield(s: &mut Sim, ci: usize) {
    let f = s.f;
    s.shields.retain(|x| !(x.owner as usize == ci && x.key == JADE_SHIELD));
    s.shields.push(Shield { owner: ci as u8, key: JADE_SHIELD, expiry: f + 1200 });
    // gcsim queues a shred every 18 frames up to 1200 at once; a chain of
    // tasks runs on the same frames
    shred(s, ci, 0);
}

fn shred(s: &mut Sim, ci: usize, i: i32) {
    if s.shields.iter().any(|x| x.owner as usize == ci && x.key == JADE_SHIELD) {
        for (key, e) in SHRED {
            s.enemy_add_resist(key, e, -0.2, 60);
        }
    }
    if i + 18 <= 1200 {
        s.add_task(Task::Char { c: ci as u8, id: TASK_SHRED, arg: i + 18 }, 18);
    }
}
