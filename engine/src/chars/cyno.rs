//! Cyno, from gcsim's internal/characters/cyno: normal and charged
//! attacks, the skill, the burst's Pactsworn Pathclearer (an EM buff that
//! is also the state: enhanced electro normal and charged attacks on their
//! own combo, the skill Mortuary Rite with its own cooldown, extensions,
//! the "PP slide", the end on swapping out), A1 (Endseer and Judication's
//! Duststalker Bolts, the dash extension), A4, the revelation (a skill out
//! of Pactsworn enters it for 6 s; Radiance: Stellar-Conduct turns the
//! bolts into direct Stellar-Conduct hits), constellations 1 to 6 with
//! their revelation parts (Together We Rise, which passes to the next
//! character on a swap), and the plunges from Xianyun's airborne state, in
//! and out of Pactsworn.
//!
//! As in gcsim, C4's energy (Cyno's electro reactions restore 3 to the
//! others, five times a burst) has no constellation check, and the
//! enhanced combo's counter is not the normal attack's: Pactsworn attacks
//! leave the normal attack's counter at 0, so a charged attack in
//! Pactsworn is gcsim's "need to use attack right before charge".

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gamedata::chars::cyno as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    revelation: bool,
    /// Pactsworn's combo counter (gcsim's normalBCounter), valid while
    /// the last action was Cyno's normal attack (any other action resets
    /// it, as gcsim's ResetAllNormalCounter does).
    b_counter: i32,
    burst_extension: i32,
    c2_stacks: i32,
    c2_rev_stacks: i32,
    c4_counter: i32,
    c6_stacks: i32,
    a1_extended: bool,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const A1_DMG: ModKey = ModKey::new("cyno-a1-dmg");
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const C1: ModKey = ModKey::new(g::C1_KEY);
const C1_REV: ModKey = ModKey::new(g::C1_REV_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C2_ICD: ModKey = ModKey::new(g::C2_ICD);
const C2_REV: ModKey = ModKey::new("cyno-c2-revelation");
const C2_REV_ICD: ModKey = ModKey::new(g::C2_REV_ICD_KEY);
const C4_REV_ICD: ModKey = ModKey::new(g::C4_REV_ICD_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const CB_PARTICLE_E: u16 = 1 << 0;
const CB_PARTICLE_B: u16 = 1 << 1;
const CB_C2: u16 = 1 << 2;
const CB_C6: u16 = 1 << 3;

const TASK_SKILL_CD: u8 = 0;
const TASK_A1: u8 = 1;

const PACTSWORN_NAMES: [&str; 5] = ["Pactsworn Pathclearer 0", "Pactsworn Pathclearer 1", "Pactsworn Pathclearer 2", "Pactsworn Pathclearer 3", "Pactsworn Pathclearer 4"];

/// The plunges out of Pactsworn: physical; gcsim's CanQueueAfter is the
/// frames to a skill.
const PLUNGE: plunge::Spec = plunge::Spec {
    low_queue: Action::Skill,
    high_queue: Action::Skill,
    ..plunge::Spec::physical(
        g::LOW_PLUNGE_HITMARK,
        g::HIGH_PLUNGE_HITMARK,
        g::COLLISION_HITMARK,
        &g::LOW_PLUNGE_FRAMES,
        &g::HIGH_PLUNGE_FRAMES,
        g::LOW_PLUNGE_POISE_DMG,
        g::HIGH_PLUNGE_POISE_DMG,
        g::LOW_PLUNGE_RADIUS,
        g::HIGH_PLUNGE_RADIUS,
        g::LOW_PLUNGE,
        g::HIGH_PLUNGE,
        g::COLLISION,
    )
};

fn pactsworn(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, BURST)
}

/// Pactsworn's combo counter, 0 after any action but Cyno's normal
/// attack.
fn b_counter(s: &Sim, ci: usize) -> i32 {
    let last = &s.player.last;
    if last.char as usize == ci && last.action == Action::Attack {
        s.st::<State>(ci).b_counter
    } else {
        0
    }
}

/// gcsim's isStellarRadiance.
fn radiance(s: &Sim, ci: usize) -> bool {
    s.st::<State>(ci).revelation && s.mod_active(ci, crate::stellar::POLESTAR_FIELD)
}

/// A4: Pactsworn's normal attacks deal 150% of his EM more.
fn a4_normal(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 4 {
        return 0.0;
    }
    s.stat(ci, st::EM) * 1.5
}

/// A4: Duststalker Bolts deal 250% of his EM more, 600% as direct
/// Stellar-Conduct hits.
fn a4_bolt(s: &Sim, ci: usize, ssc: bool) -> f64 {
    if s.chars[ci].ascension < 4 {
        return 0.0;
    }
    s.stat(ci, st::EM) * if ssc { 6.0 } else { 2.5 }
}

/// Pactsworn, as the burst or the revelation adds it: 100 EM.
fn add_pactsworn(s: &mut Sim, ci: usize, dur: i32) {
    s.add_mod(ci, Mod::stat(BURST, st::EM, 100.0, false, true, -1), dur);
}

/// gcsim's tryBurstPPSlide: Pactsworn lasts until an action's last hit.
fn try_pp_slide(s: &mut Sim, ci: usize, hitmark: i32) {
    let dur = s.mod_duration(ci, BURST.name);
    if 0 < dur && dur < hitmark {
        s.extend_mod(ci, BURST, hitmark - dur + 1);
    }
}

/// C1: 20% normal attack SPD for 10 s.
fn c1(s: &mut Sim, ci: usize) {
    s.add_mod(ci, Mod::owned(C1, ModKind::Stat, Owner::Char(ci as u8), st::ATKSPD, false, true, -1), 600);
}

/// C1 with the revelation: Together We Rise, 200 EM.
fn add_c1_rev(s: &mut Sim, on: usize, dur: i32) {
    s.add_mod(on, Mod::stat(C1_REV, st::EM, 200.0, false, true, -1), dur);
}

/// C6: four stacks of Day of the Jackal, at most 8, for 8 s.
fn c6_on_a1_skill(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 6 {
        return;
    }
    s.add_status(ci, C6, 8 * 60, true);
    let st = s.st_mut::<State>(ci);
    st.c6_stacks = (st.c6_stacks + 4).min(8);
}

/// The callbacks of a normal attack: C2's, and C6's while it has stacks.
fn normal_cbs(s: &Sim, ci: usize) -> u16 {
    let c = &s.chars[ci];
    let mut cbs = 0;
    if c.cons >= 2 {
        cbs |= CB_C2;
    }
    if c.cons >= 6 && s.st::<State>(ci).c6_stacks != 0 && s.mod_active(ci, C6) {
        cbs |= CB_C6;
    }
    cbs
}

/// gcsim's revelationOnSkill: a skill out of Pactsworn enters it for 6 s,
/// which skills cannot extend.
fn revelation_on_skill(s: &mut Sim, ci: usize) {
    if !s.st::<State>(ci).revelation || pactsworn(s, ci) {
        return;
    }
    let dur = 6 * 60;
    add_pactsworn(s, ci, dur);
    s.st_mut::<State>(ci).burst_extension = 2;
    if s.chars[ci].ascension >= 1 {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, 234);
    }
    if s.chars[ci].cons >= 1 {
        add_c1_rev(s, ci, dur);
    }
}

impl Character for Char {
    const KEY: &'static str = "cyno";
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        let revelation = cs.params.iter().find(|p| p.0 == "revelation").is_none_or(|p| p.1 > 0);
        State { revelation, ..State::default() }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let revelation = s.st::<State>(ci).revelation;
        // the skill's cooldown is also Mortuary Rite's (gcsim's override
        // of ReduceActionCooldown and ResetActionCooldown)
        s.chars[ci].skill_cd_twin = true;
        // burstInit and c1Init: swaps; a1Extension: dashes
        s.subscribe(EventKind::Swap, o);
        s.subscribe(EventKind::Dash, o);
        if s.chars[ci].cons >= 2 && revelation {
            s.subscribe(EventKind::EnemyHit, o);
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(C2_REV, ModKind::ReactBonus, o, st::NONE, false, true, -1));
            }
        }
        // c4Init, at every constellation
        s.subscribe(EventKind::Reaction, o);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// Plunges only while airborne (Xianyun's jump).
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    /// In Pactsworn the skill is Mortuary Rite, on its own cooldown.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        if a != Action::Skill {
            return None;
        }
        let i = if pactsworn(s, ci) { crate::sim::Char::cd_index(Action::LowPlunge) } else { crate::sim::Char::cd_index(Action::Skill) };
        Some((s.chars[ci].cd[i].available <= 0).then_some(Failure::SkillCd))
    }

    /// gcsim's NextNormalCounter: Pactsworn's combo in Pactsworn.
    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "normal" if pactsworn(s, ci) => Some(Ok(Num::Int(b_counter(s, ci) as i64 + 1))),
            _ => None,
        }
    }

    /// AnimationXingqiuN0StartDelay: 12 in Pactsworn, else 10.
    fn start_delay(s: &Sim, ci: usize, key: usize) -> Option<i32> {
        (key == 2 && pactsworn(s, ci)).then_some(12)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if pactsworn(s, ci) {
            return attack_b(s, ci);
        }
        let cbs = normal_cbs(s, ci);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let (off, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            // gcsim gives these in frames as written, not times 60
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i];
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            ai.pattern = if n == 1 || n == 2 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.fan_on_player(0.0, off, hb[0], g::ATTACK_FAN_ANGLES[n]) };
            let h = g::ATTACK_HITMARKS[n][i];
            s.queue_attack(ai, cbs, h, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let h = *g::ATTACK_HITMARKS[n].last().unwrap();
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let b = pactsworn(s, ci);
        if b {
            try_pp_slide(s, ci, g::CHARGE_B_HITMARK);
        }
        let (mult, h, t) = if b { (g::CHARGE_B[s.talent(ci, 2)], g::CHARGE_B_HITMARK, &g::CHARGE_B_FRAMES) } else { (g::CHARGE[s.talent(ci, 0)], g::CHARGE_HITMARK, &g::CHARGE_FRAMES) };
        let ele = if b { Element::Electro } else { Element::Physical };
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, ele, 25.0, mult);
        ai.abil = if b { "Pactsworn Pathclearer Charge" } else { "Charge Attack" };
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        ai.ignore_infusion = b;
        ai.pattern = s.circle_on_target(0.8);
        s.queue_attack(ai, 0, 0, h);
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if pactsworn(s, ci) {
            return skill_b(s, ci);
        }
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL[lvl]);
        ai.abil = "Secret Rite: Chasmic Soulfarer";
        ai.pattern = s.circle_on_target(1.0);
        s.queue_attack(ai, CB_PARTICLE_E, g::SKILL_HITMARK, g::SKILL_HITMARK);
        s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_CD, arg: 0 }, g::SKILL_CD_DELAY);
        revelation_on_skill(s, ci);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let st = s.st_mut::<State>(ci);
        st.burst_extension = 0;
        st.c4_counter = 0;
        st.c6_stacks = 0;
        // 112 frames more than its 10 s
        add_pactsworn(s, ci, 712);
        if s.chars[ci].ascension >= 1 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, 328);
        }
        s.set_cd(ci, Action::Burst, 1200);
        s.consume_energy(ci, 3);
        if s.chars[ci].cons >= 1 {
            c1(s, ci);
        }
        c6_on_a1_skill(s, ci);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if pactsworn(s, ci) {
            return plunge_b(s, ci, true, p);
        }
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if pactsworn(s, ci) {
            return plunge_b(s, ci, false, p);
        }
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            // triggerSkillCD
            TASK_SKILL_CD => {
                s.reset_cd(ci, Action::Skill);
                s.set_cd(ci, Action::Skill, g::SKILL_CD);
                s.set_cd(ci, Action::LowPlunge, g::SKILL_BCD);
            }
            // A1: Endseer at intervals while in Pactsworn
            TASK_A1 => {
                if !pactsworn(s, ci) {
                    return;
                }
                s.st_mut::<State>(ci).a1_extended = false;
                s.add_status(ci, A1, 84, true);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, 234);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & (CB_PARTICLE_E | CB_PARTICLE_B) != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 30, true);
            let count = if cbs & CB_PARTICLE_B != 0 {
                if s.rng.float64() < 0.33 { 2.0 } else { 1.0 }
            } else {
                3.0
            };
            s.queue_particle(count, Element::Electro, s.particle_delay[ci]);
        }
        // C2: a normal attack's hit gives a stack of 10% electro DMG, at
        // most 5, for 4 s, once every 0.1 s
        if cbs & CB_C2 != 0 && !s.mod_active(ci, C2_ICD) {
            s.add_status(ci, C2_ICD, 6, true);
            if !s.mod_active(ci, C2) {
                s.st_mut::<State>(ci).c2_stacks = 0;
            }
            let st = s.st_mut::<State>(ci);
            st.c2_stacks = (st.c2_stacks + 1).min(5);
            s.add_mod(ci, Mod::owned(C2, ModKind::Stat, Owner::Char(ci as u8), st::ELECTROP, false, true, -1), 4 * 60);
        }
        // C6: a stack of Day of the Jackal fires a bolt, once every 0.4 s
        if cbs & CB_C6 != 0 && s.st::<State>(ci).c6_stacks != 0 && s.mod_active(ci, C6) && !s.mod_active(ci, C6_ICD) {
            s.add_status(ci, C6_ICD, 24, true);
            s.st_mut::<State>(ci).c6_stacks -= 1;
            let mut ai = if s.st::<State>(ci).revelation && s.mod_active(ci, C1_REV) {
                let mut ai = AttackInfo::new(ci, AttackTag::DirectStellarConduct, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, 2.0);
                ai.abil = "Raiment: Just Scales (C6) (Stellar-Conduct)";
                ai.flat_dmg = a4_bolt(s, ci, true);
                ai.ignore_def = 1.0;
                ai
            } else {
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArtHold, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Electro, 25.0, 1.0);
                ai.abil = "Raiment: Just Scales (C6)";
                ai.deployable = true;
                ai.flat_dmg = a4_bolt(s, ci, false);
                ai
            };
            ai.pattern = s.circle_on_target(0.3);
            s.queue_attack(ai, 0, 0, 0);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C1 {
            // C1: only while normal attacking
            if s.current_state() != AnimState::Normal {
                return false;
            }
            out[st::ATKSPD] += 0.2;
        } else {
            out[st::ELECTROP] += 0.1 * s.st::<State>(ci).c2_stacks as f64;
        }
        true
    }

    /// A1: Judication's Mortuary Rite deals 35% more.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.abil == g::SKILL_B_NAME {
            ev.stats[st::DMGP] += 0.35;
        }
    }

    /// C2 with the revelation: 16% Stellar-Conduct DMG a stack to the
    /// characters with Together We Rise.
    fn react_bonus(s: &Sim, ci: usize, on: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if info.tag != AttackTag::DirectStellarConduct || !s.mod_active(on, C1_REV) {
            return 0.0;
        }
        0.16 * s.st::<State>(ci).c2_rev_stacks as f64
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { prev, next } => {
                if prev != ci {
                    return;
                }
                // Pactsworn ends on swapping out
                if pactsworn(s, ci) {
                    s.delete_mod(ci, BURST);
                }
                // C1 with the revelation: Together We Rise goes to the next
                // character for what is left of it
                if s.chars[ci].cons >= 1 && s.st::<State>(ci).revelation {
                    let dur = s.mod_duration(ci, C1_REV.name);
                    if dur > 0 {
                        s.delete_mod(ci, C1_REV);
                        add_c1_rev(s, next, dur);
                    }
                }
            }
            // A1: a dash in Endseer extends it by 17 frames, once
            Event::Dash => {
                if s.st::<State>(ci).a1_extended || s.player.active as usize != ci || !s.mod_active(ci, A1) {
                    return;
                }
                s.extend_mod(ci, A1, 17);
                s.st_mut::<State>(ci).a1_extended = true;
            }
            // C2 with the revelation: normal and charged attacks on the
            // enemy by a character with Together We Rise add a stack, at
            // most 5, once every 0.1 s per character
            Event::EnemyHit(ev) => {
                if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
                    return;
                }
                let on = ev.info.actor as usize;
                if !s.mod_active(on, C1_REV) || s.mod_active(on, C2_REV_ICD) {
                    return;
                }
                s.add_status(on, C2_REV_ICD, 6, true);
                let st = s.st_mut::<State>(ci);
                st.c2_rev_stacks = (st.c2_rev_stacks + 1).min(5);
            }
            Event::Reaction(r, ev) => c4(s, ci, r, ev),
            _ => {}
        }
    }
}

/// gcsim's c4Init: Cyno's electro reactions on the enemy restore 3 energy
/// to the others, five times a burst; with the revelation, those of a
/// character with Together We Rise give Cyno 20, once every 18 s.
/// Hyperbloom counts on a dendro core too.
fn c4(s: &mut Sim, ci: usize, r: Reaction, ev: &AttackEvent) {
    match r {
        Reaction::Overload | Reaction::ElectroCharged | Reaction::LunarCharged | Reaction::Superconduct | Reaction::StellarConduct | Reaction::Quicken | Reaction::Aggravate | Reaction::SwirlElectro => {
            if !ev.on_enemy() {
                return;
            }
        }
        Reaction::Hyperbloom => {}
        _ => return,
    }
    let actor = ev.info.actor as usize;
    if actor != ci {
        if !s.st::<State>(ci).revelation || !s.mod_active(actor, C1_REV) || s.mod_active(ci, C4_REV_ICD) {
            return;
        }
        s.add_status(ci, C4_REV_ICD, 18 * 60, true);
        s.add_energy(ci, 20.0);
        return;
    }
    let st = s.st_mut::<State>(ci);
    if st.c4_counter > 4 {
        return;
    }
    st.c4_counter += 1;
    for i in 0..s.chars.len() {
        if i != ci {
            s.add_energy(i, 3.0);
        }
    }
}

/// Pactsworn's normal attacks (gcsim's attackB): electro, on their own
/// combo of five, each hit queued on Cyno's queue at its hitmark.
fn attack_b(s: &mut Sim, ci: usize) -> ActInfo {
    let n = b_counter(s, ci) as usize;
    let last = *g::ATTACK_B_HITMARKS[n].last().unwrap();
    try_pp_slide(s, ci, last);
    let cbs = normal_cbs(s, ci);
    let lvl = s.talent(ci, 2);
    let flat = a4_normal(s, ci);
    let (off, hb) = (g::ATTACK_B_OFFSETS[n], g::ATTACK_B_HITBOXES[n]);
    for (i, mult) in g::ATTACK_B[n].iter().enumerate() {
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Electro, 25.0, mult[lvl]);
        ai.abil = PACTSWORN_NAMES[n];
        ai.hitlag_factor = 0.01;
        // in frames as gcsim writes them
        ai.hitlag_halt = g::ATTACK_B_HITLAG_HALT_FRAME[n][i];
        ai.def_halt = g::ATTACK_B_DEF_HALT[n][i];
        ai.flat_dmg = flat;
        ai.ignore_infusion = true;
        if n == 2 {
            ai.blunt = true;
            ai.poise = 64.0248;
        }
        if n == 4 {
            ai.blunt = true;
            ai.poise = 77.7;
        }
        ai.pattern = if n == 2 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
        s.queue_attack_at(ci, ai, cbs, g::ATTACK_B_HITMARKS[n][i]);
    }
    s.st_mut::<State>(ci).b_counter = if n as i32 + 1 == g::BURST_HIT_NUM { 0 } else { n as i32 + 1 };
    let atkspd = s.stat(ci, st::ATKSPD);
    let table = &g::ATTACK_B_FRAMES[n];
    ActInfo::new(Frames::Attack { table, atkspd }, table[0], last, AnimState::Normal)
}

/// Mortuary Rite (gcsim's skillB), with Judication in Endseer: 35% more
/// DMG and three Duststalker Bolts.
fn skill_b(s: &mut Sim, ci: usize) -> ActInfo {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_B[lvl]);
    ai.abil = g::SKILL_B_NAME;
    ai.blunt = true;
    ai.poise = 75.0;
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = 0.03 * 60.0;
    ai.pattern = s.circle_on_player(0.0, 1.5, 6.0);
    let h = g::SKILL_B_HITMARK;
    if !s.mod_active(ci, A1) {
        s.queue_attack(ai, CB_PARTICLE_B, h, h);
    } else {
        // a1Buff: the game's own 1 s modifier
        s.add_mod(ci, Mod::owned(A1_DMG, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 60);
        if s.chars[ci].cons >= 1 && s.mod_active(ci, C1) {
            c1(s, ci);
        }
        c6_on_a1_skill(s, ci);
        s.queue_attack(ai, CB_PARTICLE_B, h, h);
        let mut bolt = if radiance(s, ci) {
            let mut b = AttackInfo::new(ci, AttackTag::DirectStellarConduct, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, 2.0);
            b.abil = "Duststalker Bolt (Stellar-Conduct)";
            b.ignore_def = 1.0;
            b.flat_dmg = a4_bolt(s, ci, true);
            b
        } else {
            let mut b = AttackInfo::new(ci, AttackTag::ElementalArtHold, icd::TAG_ELEMENTAL_ART, icd::GROUP_CYNO_BOLT, Element::Electro, 25.0, 1.0);
            b.abil = "Duststalker Bolt";
            b.flat_dmg = a4_bolt(s, ci, false);
            b
        };
        bolt.pattern = s.circle_on_target(0.3);
        for _ in 0..3 {
            s.queue_attack(bolt, CB_PARTICLE_B, h, h);
        }
    }
    // up to two extensions of 4 s a burst
    if s.st::<State>(ci).burst_extension < 2 {
        s.extend_mod(ci, BURST, 240);
        s.st_mut::<State>(ci).burst_extension += 1;
    }
    try_pp_slide(s, ci, h);
    s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_CD, arg: 0 }, g::SKILL_BCD_DELAY);
    let t = &g::SKILL_B_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
}

/// The plunges in Pactsworn: electro, of the burst's talent level.
fn plunge_b(s: &mut Sim, ci: usize, high: bool, p: &Params) -> ActInfo {
    let lvl = s.talent(ci, 2);
    if plunge::wants_collision(p) {
        let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, g::COLLISION_B[lvl]);
        ai.abil = "Plunge Collision";
        ai.ignore_infusion = true;
        ai.pattern = s.circle_on_player(0.0, 1.0, 1.0);
        s.queue_attack(ai, 0, g::COLLISION_HITMARK_B, g::COLLISION_HITMARK_B);
    }
    let (mult, poise, r, h, t, abil) = if high {
        (g::HIGH_PLUNGE_B[lvl], g::HIGH_PLUNGE_POISE_DMGB, g::HIGH_PLUNGE_RADIUS_B, g::HIGH_PLUNGE_HITMARK_B, &g::HIGH_PLUNGE_FRAMES_B, "High Plunge (Pactsworn Pathclearer)")
    } else {
        (g::LOW_PLUNGE_B[lvl], g::LOW_PLUNGE_POISE_DMGB, g::LOW_PLUNGE_RADIUS_B, g::LOW_PLUNGE_HITMARK_B, &g::LOW_PLUNGE_FRAMES_B, "Low Plunge (Pactsworn Pathclearer)")
    };
    let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, mult);
    ai.abil = abil;
    ai.blunt = true;
    ai.poise = poise;
    ai.ignore_infusion = true;
    ai.pattern = s.circle_on_player(0.0, 1.0, r);
    s.queue_attack(ai, 0, h, h);
    ActInfo::new(Frames::Table(t), t[0], t[Action::Skill as usize], AnimState::Plunge)
}
