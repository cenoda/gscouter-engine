//! Zhongli's normal attacks, skill (pressed and held), steles and Jade
//! Shield, the burst, A1, A4 and C1, from gcsim's internal/characters/zhongli.

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
    /// gcsim's Tags["shielded"] and Tags["a1"]: the Jade Shield is up, and
    /// the hits it took (A1's stacks).
    shielded: bool,
    a1: i32,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const STELE: ModKey = ModKey::new("zhongli-stele");
const JADE_SHIELD: ModKey = ModKey::new("zhongli-jade-shield");
const A1: ModKey = ModKey::new("zhongli-a1");
/// The Jade Shield shreds every element, in gcsim's order.
const SHRED: [(ModKey, Element); 8] = [
    (ModKey::new("zhongli-pyro"), Element::Pyro),
    (ModKey::new("zhongli-hydro"), Element::Hydro),
    (ModKey::new("zhongli-cryo"), Element::Cryo),
    (ModKey::new("zhongli-electro"), Element::Electro),
    (ModKey::new("zhongli-geo"), Element::Geo),
    (ModKey::new("zhongli-anemo"), Element::Anemo),
    (ModKey::new("zhongli-physical"), Element::Physical),
    (ModKey::new("zhongli-dendro"), Element::Dendro),
];

const CB_PARTICLE: u16 = 1 << 0;

const TASK_NEW_STELE: u8 = 0;
const TASK_JADE_SHIELD: u8 = 1;
const TASK_SHRED: u8 = 2;
const TASK_RESONANCE: u8 = 3;

impl Character for Char {
    const KEY: &'static str = "zhongli";
    // C2, C4 and C6 are not ported
    const MAX_CONS: i32 = 1;
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        State {
            stele_count: 0,
            max_stele: if cs.cons >= 1 { 2 } else { 1 },
            stele_snap: AttackEvent::new(AttackInfo::new(0, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 0.0, 0.0), 0),
            shielded: false,
            a1: 0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].ascension >= 1 {
            s.shield_bonus_mods.push(ShieldBonusMod { key: A1, owner: Owner::Char(ci as u8), expiry: -1 });
        }
    }

    /// A1: shield strength +5% for each hit the Jade Shield took, up to 5.
    fn shield_bonus(s: &Sim, ci: usize, _: ModKey) -> (f64, bool) {
        let st = s.st::<State>(ci);
        if !st.shielded {
            return (0.0, false);
        }
        (st.a1 as f64 * 0.05, true)
    }

    /// The Jade Shield took a hit (its C6 heal is not ported).
    fn shield_damaged(s: &mut Sim, ci: usize, _: ModKey, broken: bool) {
        let st = s.st_mut::<State>(ci);
        if broken {
            st.shielded = false;
            st.a1 = 0;
        }
        if st.a1 < 5 {
            st.a1 += 1;
        }
    }

    fn shield_expired(s: &mut Sim, ci: usize, _: ModKey) {
        let st = s.st_mut::<State>(ci);
        st.shielded = false;
        st.a1 = 0;
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// Planet Befall, with A4's 33% of max HP when it is cast.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 100.0, g::BURST[lvl]);
        ai.abil = "Planet Befall";
        ai.blunt = true;
        ai.poise = 500.0;
        if s.chars[ci].ascension >= 4 {
            ai.flat_dmg = 0.33 * s.max_hp(ci);
        }
        ai.pattern = s.circle_on_player(0.0, 5.0, 7.5);
        s.queue_attack(ai, 0, 101, 101);
        s.set_cd(ci, Action::Burst, 720);
        s.consume_energy(ci, 7);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let shielded = || Some(Ok(Num::Int(s.st::<State>(ci).shielded as i64)));
        match (fields[0].as_str(), fields.get(1).map(|f| f.as_str())) {
            ("shielded", _) | ("tags", Some("shielded")) => shielded(),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        // A4: from max HP when the attack starts
        let flat = if s.chars[ci].ascension >= 4 { 0.0139 * s.max_hp(ci) } else { 0.0 };
        for i in 0..g::ATTACK_HITS[n] as usize {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.flat_dmg = flat;
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let h = g::ATTACK_HITMARKS[n][i];
            s.queue_attack(ai, 0, h, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], g::ATTACK_EARLIEST_CANCEL[n], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let nostele = p.get("hold_nostele").unwrap_or(0) > 0;
        if p.get("hold").unwrap_or(0) <= 0 && !nostele {
            s.add_task(Task::Char { c: ci as u8, id: TASK_NEW_STELE, arg: 0 }, g::SKILL_PRESS_HIMARK);
            s.set_cd_with_delay(ci, Action::Skill, 240, 22);
            let t = &g::SKILL_PRESS_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill);
        }
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL_HOLD[lvl]);
        ai.abil = "Stone Stele (Hold)";
        ai.blunt = true;
        ai.poise = 142.9;
        ai.flat_dmg = a4_skill(s, ci);
        s.queue_attack(ai, 0, 0, g::SKILL_HOLD_HITMARK);
        let st = s.st::<State>(ci);
        if st.stele_count < st.max_stele && !nostele {
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

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
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

fn steles(s: &Sim, ci: usize) -> impl Iterator<Item = &Construct> {
    s.constructs.iter().filter(move |c| c.owner as usize == ci && c.key == STELE)
}

fn new_stele(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 1);
    let flat = a4_skill(s, ci);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 50.0, g::SKILL[lvl]);
    ai.abil = "Stone Stele (Initial)";
    ai.blunt = true;
    ai.poise = 200.0;
    ai.flat_dmg = flat;
    s.queue_attack(ai, CB_PARTICLE, 0, 0);

    let f = s.f;
    let num = steles(s, ci).count() as i32;
    let refresh = num == s.st::<State>(ci).max_stele;
    s.new_construct(Construct { owner: ci as u8, key: STELE, src: f, expiry: f + 1860 }, refresh);
    s.st_mut::<State>(ci).stele_count = steles(s, ci).count() as i32;

    let mut tick = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL_TICK[lvl]);
    tick.abil = "Stone Stele (Tick)";
    tick.blunt = true;
    tick.poise = 0.0;
    tick.flat_dmg = flat;
    s.st_mut::<State>(ci).stele_snap = s.snapshot_event(tick, 0);
    s.add_task(Task::Char { c: ci as u8, id: TASK_RESONANCE, arg: f }, 120);
}

/// A stele's tick: its own hit, and one for each other construct (not a
/// stele) near it, which is every one with no positions.
fn resonance(s: &mut Sim, ci: usize, src: i32) {
    if !s.constructs.iter().any(|c| c.src == src) {
        return;
    }
    let ev = s.st::<State>(ci).stele_snap;
    let others = s.constructs.iter().filter(|c| !(c.owner as usize == ci && c.key == STELE)).count();
    for _ in 0..1 + others {
        s.queue_attack_with_snap(ev, CB_PARTICLE, 0);
    }
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
    let lvl = s.talent(ci, 1);
    let hp = g::SHIELD_BASE[lvl] + g::SHIELD_PER[lvl] * s.max_hp(ci);
    s.add_shield(Shield { owner: ci as u8, key: JADE_SHIELD, typ: ShieldType::ZhongliJadeShield, target: -1, ele: Element::Geo, hp, expiry: f + 1200, em_bonus: 0.0 });
    s.st_mut::<State>(ci).shielded = true;
    // gcsim queues a shred every 18 frames up to 1200 at once; a chain of
    // tasks runs on the same frames
    shred(s, ci, 0);
}

fn shred(s: &mut Sim, ci: usize, i: i32) {
    if s.st::<State>(ci).shielded {
        for (key, e) in SHRED {
            s.enemy_add_resist(key, e, -0.2, 60);
        }
    }
    if i + 18 <= 1200 {
        s.add_task(Task::Char { c: ci as u8, id: TASK_SHRED, arg: i + 18 }, 18);
    }
}
