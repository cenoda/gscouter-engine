//! The cryo Traveler (luminecryo and aethercryo), from gcsim's
//! internal/characters/traveler/common/cryo: normal and charged attacks
//! (the Icepoint charged attack, Freezing Ice, from the True Moon story
//! buff), Lumine's airborne plunges (Aether's are gcsim's error), Ice Fog
//! Piercer and its Frostpierce Star (its crystals and Frostglow),
//! Frostbound Javelin, the Radiance states (`stellar.rs`: it turns
//! superconduct into Stellar-Conduct and cryo swirl into Stellar Swirl, and
//! adds Base DMG to direct and reaction Stellar DMG), A1, A4 and every
//! constellation.
//!
//! One module holds both: `luminecryo` and `aethercryo` are `Char<1>` and
//! `Char<0>` (gcsim's gender).

use super::traveler_common;
use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gamedata::chars::traveler_common_cryo as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::stellar::{POLESTAR_FIELD, RADIANCE_SWIRL};
use crate::types::*;

pub struct Char<const G: u8>;

pub struct State<const G: u8>(Inner);

pub struct Inner {
    gender: usize,
    /// gcsim's skillSrc, frostglowStacks, skillTravel, trueMoonBuff and
    /// trueMoonStacks (Icepoint).
    skill_src: i32,
    frostglow: i32,
    skill_travel: i32,
    true_moon: bool,
    icepoint: i32,
    /// The Frostglow stacks C6's mods were given at the last burst.
    c6_stacks: i32,
    story: traveler_common::Story,
}

fn st(s: &Sim, ci: usize) -> &Inner {
    match &s.chars[ci].state {
        CharState::LumineCryo(x) => &x.0,
        CharState::AetherCryo(x) => &x.0,
        _ => unreachable!(),
    }
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut Inner {
    match &mut s.chars[ci].state {
        CharState::LumineCryo(x) => &mut x.0,
        CharState::AetherCryo(x) => &mut x.0,
        _ => unreachable!(),
    }
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const SKILL_ICD: ModKey = ModKey::new(g::SKILL_ICD_KEY);
const CA_TRUE_MOON_ICD: ModKey = ModKey::new(g::CHARGE_TRUE_MOON_ICD_KEY);
const TRUE_MOON_STACK_ICD: ModKey = ModKey::new(g::TRUE_MOON_STACK_ICD_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C2_UPGRADE: ModKey = ModKey::new(g::C2_UPGRADE_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);

/// naCaPlungeCB, on normal, charged and plunging hits.
const CB_NA_CA_PLUNGE: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;
/// crystalCB and c2CB, on the crystals.
const CB_CRYSTAL: u16 = 1 << 2;

/// skillTicker, with its src.
const TASK_TICKER: u8 = 0;
/// The burst's javelins at its spawn frame, with its travel.
const TASK_BURST: u8 = 1;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];
const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];
const CHARGE_SC_NAMES: [&str; 2] = ["Charge 0 (Stellar-Conduct)", "Charge 1 (Stellar-Conduct)"];
const CHARGE_SSW_NAMES: [&str; 2] = ["Charge 0 (Stellar Swirl)", "Charge 1 (Stellar Swirl)"];

/// Lumine's airborne plunges, from gcsim's plunge.go.
const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES[1], &g::HIGH_PLUNGE_FRAMES[1], g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Radiance {
    None,
    StellarConduct,
    StellarSwirl,
}

fn radiance(s: &Sim, ci: usize) -> Radiance {
    if s.mod_active(ci, POLESTAR_FIELD) {
        Radiance::StellarConduct
    } else if s.mod_active(ci, RADIANCE_SWIRL) {
        Radiance::StellarSwirl
    } else {
        Radiance::None
    }
}

impl<const G: u8> Character for Char<G> {
    const KEY: &'static str = if G == 1 { "luminecryo" } else { "aethercryo" };
    type State = State<G>;

    fn new_state(cs: &CharSetup) -> State<G> {
        // Freezing Ice reads the param as on when above 0 (the story's
        // buffs, only at 1)
        let true_moon = cs.params.iter().find(|p| p.0 == crate::gamedata::chars::traveler_common::TRUE_MOON_PARAM).is_none_or(|p| p.1 > 0);
        State(Inner { gender: G as usize, skill_src: 0, frostglow: 0, skill_travel: 0, true_moon, icepoint: 0, c6_stacks: 0, story: traveler_common::Story::new(cs) })
    }

    fn init(s: &mut Sim, ci: usize) {
        traveler_common::story_buffs(s, ci);
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // stellarInit: the Base DMG of direct and reaction Stellar DMG
        s.subscribe(EventKind::EnemyHit, o);
        s.subscribe(EventKind::SpecialReactionAttack, o);
        // trueMoonInit, c1Init and c2Init's damage hook: one subscription
        if st(s, ci).true_moon || cons >= 1 {
            s.subscribe(EventKind::EnemyDamage, o);
        }
        // a4Init
        if asc >= 4 {
            s.add_mod(ci, Mod::owned(A4, ModKind::Stat, o, st::EM, true, false, -1), -1);
        }
        // c2Init: OnStellarConduct and OnStellarSwirl
        if cons >= 2 {
            s.subscribe(EventKind::Reaction, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Skill | Action::Burst => p.0.iter().find(|(k, _)| k != "travel").map(|(k, _)| format!("{name} param {k}")),
            Action::Attack | Action::Charge => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            Action::HighPlunge | Action::LowPlunge => plunge::unknown_param(a, p, &[]),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        // gcsim implements no plunges for Aether
        if G == 0 {
            match a {
                Action::LowPlunge => return Some(format!("{}: action low_plunge not implemented", Self::KEY)),
                Action::HighPlunge => return Some(format!("{}: action high_plunge not implemented", Self::KEY)),
                _ => {}
            }
        }
        if let Some(why) = plunge::grounded(s, a) {
            return Some(why);
        }
        crate::content::template_invalid(s, a)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "frostglow" => Some(Ok(Num::Int(st(s, ci).frostglow as i64))),
            "icepoint" => Some(Ok(Num::Int(st(s, ci).icepoint as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[sex][n] * 60.0;
        ai.def_halt = true;
        let off = g::ATTACK_OFFSETS[sex][n];
        let hb = g::ATTACK_HITBOXES[sex][n];
        ai.pattern = if (sex == 0 && (n == 0 || n == 2)) || (sex == 1 && n == 1) {
            s.box_on_player(0.0, off, hb[0], hb[1])
        } else {
            s.fan_on_player(0.0, off, hb[0], g::ATTACK_FAN_ANGLES[sex][n])
        };
        a1_conversion(s, ci, &mut ai);
        let h = g::ATTACK_HITMARKS[sex][n];
        s.queue_attack(ai, CB_NA_CA_PLUNGE, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[sex][n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, 0.0);
        let freezing_ice = charge_true_moon(s, ci);
        let marks = g::CHARGE_HITMARKS[sex];
        for (i, mult) in g::CHARGE[sex].iter().enumerate() {
            ai.mult = mult[lvl];
            ai.abil = CHARGE_NAMES[i];
            ai.pattern = s.circle_on_player(0.0, 0.0, 2.2);
            if freezing_ice {
                ai.element = Element::Cryo;
                ai.ignore_infusion = true;
                ai.icd_tag = icd::TAG_TRAVELER_ENHANCED_CA;
                ai.mult += 1.4;
                ai.hitlag_halt = 0.05 * 60.0;
                ai.hitlag_factor = 0.05;
                ai.def_halt = true;
                ai.pattern = s.circle_on_player(0.0, 1.0, 3.2);
                match radiance(s, ci) {
                    Radiance::StellarConduct => {
                        ai.abil = CHARGE_SC_NAMES[i];
                        ai.tag = AttackTag::DirectStellarConduct;
                        ai.icd_tag = icd::TAG_NONE;
                        ai.durability = 0.0;
                        ai.ignore_def = 1.0;
                    }
                    Radiance::StellarSwirl => {
                        ai.abil = CHARGE_SSW_NAMES[i];
                        ai.tag = AttackTag::DirectStellarSwirl;
                        ai.icd_tag = icd::TAG_NONE;
                        ai.durability = 0.0;
                        ai.ignore_def = 1.0;
                    }
                    Radiance::None => {}
                }
            } else {
                a1_conversion(s, ci, &mut ai);
            }
            s.queue_attack(ai, CB_NA_CA_PLUNGE, marks[i], marks[i]);
        }
        let t = &g::CHARGE_FRAMES[sex];
        ActInfo::new(Frames::Table(t), t[0], marks[marks.len() - 1], AnimState::Charge)
    }

    /// Ice Fog Piercer and its Frostpierce Star.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        let travel = p.get("travel").map_or(9, |t| t as i32);
        st_mut(s, ci).skill_travel = travel;
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL[lvl]);
        ai.abil = "Ice Fog Piercer";
        ai.pattern = s.fan_on_player(0.0, -0.3, 8.0, 60.0);
        s.queue_attack(ai, CB_PARTICLE, g::SKILL_HITMARK, g::SKILL_HITMARK);
        let src = s.f;
        st_mut(s, ci).skill_src = src;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_TICKER, arg: src }, g::SKILL_FIRST_TICK_DELAY);
        let c4 = if s.chars[ci].cons >= 4 { 3 * 60 } else { 0 };
        s.add_status(ci, SKILL, g::SKILL_HITMARK + 12 * 60 + c4, true);
        s.set_cd_with_delay(ci, Action::Skill, 15 * 60, 17);
        let t = &g::SKILL_FRAMES[sex];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        let travel = p.get("travel").map_or(6, |t| t as i32);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST, arg: travel }, g::BURST_SPAWN_FRAME);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 6);
        let t = &g::BURST_FRAMES[sex];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge_xy(s, ci, false, p)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge_xy(s, ci, true, p)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_TICKER => {
                if st(s, ci).skill_src != arg || !s.mod_active(ci, SKILL) {
                    return;
                }
                if radiance(s, ci) != Radiance::StellarConduct {
                    queue_crystal(s, ci, g::SKILL_TICK_HITMARKS);
                }
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_TICKER, arg }, g::SKILL_INTERVAL);
            }
            TASK_BURST => burst_javelins(s, ci, arg),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_NA_CA_PLUNGE != 0 && radiance(s, ci) == Radiance::StellarConduct && s.mod_active(ci, SKILL) && !s.mod_active(ci, SKILL_ICD) {
            s.add_status(ci, SKILL_ICD, g::SKILL_SSC_ICD as i32, true);
            queue_crystal(s, ci, &[g::SKILL_SSC_DELAY]);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, (0.3 * 60.0) as i32, true);
            s.queue_particle(3.0, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_CRYSTAL != 0 {
            // crystalCB
            let stv = st_mut(s, ci);
            stv.frostglow = (stv.frostglow + 1).min(g::FROSTGLOW_MAX);
            // c2CB: the active character's EM
            if s.chars[ci].cons >= 2 {
                let a = s.player.active as usize;
                s.add_mod(a, Mod::owned(C2, ModKind::Stat, Owner::Char(ci as u8), st::EM, false, true, -1), 5 * 60);
            }
        }
    }

    fn mod_amount(s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == A4 {
            // 8% of ATK without extra mods, up to 160
            let stats = s.select_stats(ci, [st::BASEATK, st::ATKP, st::ATK], true);
            out[st::EM] += (total_atk(&stats) * 0.08).min(160.0);
            return true;
        }
        if m.key == C2 {
            out[st::EM] += if s.mod_active(on, C2_UPGRADE) { 120.0 } else { 60.0 };
            return true;
        }
        traveler_common::story_amount(&st(s, ci).story, m, out)
    }

    /// C6: Stellar Glimmer DMG of the other members +5% per Frostglow the
    /// burst used.
    fn react_bonus(s: &Sim, ci: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if m.key != C6 || !matches!(info.tag, AttackTag::DirectStellarConduct | AttackTag::DirectStellarSwirl | AttackTag::ReactionStellarSwirl) {
            return 0.0;
        }
        0.05 * st(s, ci).c6_stacks as f64
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyHit(ev) => {
                if ev.info.tag.is_stellar_direct() {
                    s.hit_base_bonus += stellar_bonus(s, ci);
                }
            }
            Event::SpecialReactionAttack(ev) => {
                if ev.info.tag.is_stellar_react() {
                    s.special_base_bonus += stellar_bonus(s, ci);
                }
            }
            Event::EnemyDamage(ev) => {
                // trueMoonInit: Icepoint, once every 2 s
                if st(s, ci).true_moon && ev.info.tag.is_stellar() && !s.mod_active(ci, TRUE_MOON_STACK_ICD) {
                    s.add_status(ci, TRUE_MOON_STACK_ICD, 2 * 60, true);
                    let stv = st_mut(s, ci);
                    stv.icepoint = (stv.icepoint + 1).min(3);
                }
                let glimmer = matches!(ev.info.tag, AttackTag::DirectStellarConduct | AttackTag::DirectStellarSwirl | AttackTag::ReactionStellarSwirl);
                let cons = s.chars[ci].cons;
                // C1: 5 energy on its own Stellar Glimmer DMG, once every 0.5 s
                if cons >= 1 && glimmer && ev.info.actor as usize == ci && !s.mod_active(ci, C1_ICD) {
                    s.add_status(ci, C1_ICD, 30, true);
                    s.add_energy(ci, 5.0);
                }
                // C2: Stellar Glimmer DMG upgrades the EM for what is left
                if cons >= 2 && glimmer {
                    let a = ev.info.actor as usize;
                    if s.mod_active(a, C2) {
                        let dur = s.mod_duration(a, g::C2_KEY);
                        s.add_status(a, C2_UPGRADE, dur, true);
                    }
                }
            }
            Event::Reaction(r, ev) => {
                // C2: a Stellar Glimmer reaction upgrades it for 5 s
                if !matches!(r, Reaction::StellarConduct | Reaction::StellarSwirl) || ev.on_player || ev.on_gadget {
                    return;
                }
                let a = ev.info.actor as usize;
                if s.mod_active(a, C2) {
                    s.add_status(a, C2_UPGRADE, 5 * 60, true);
                }
            }
            _ => {}
        }
    }
}

/// stellarInit's Base DMG: 0.35% per 100 ATK, up to 7%.
fn stellar_bonus(s: &Sim, ci: usize) -> f64 {
    (s.total_atk(ci) / 100.0 * 0.0035).min(0.07)
}

/// A1: in Radiance: Stellar-Conduct with the Star out, normal, charged
/// and plunging attacks are cryo and deal 80% of ATK more.
fn a1_conversion(s: &Sim, ci: usize, ai: &mut AttackInfo) {
    if s.chars[ci].ascension < 1 || radiance(s, ci) != Radiance::StellarConduct || !s.mod_active(ci, SKILL) {
        return;
    }
    ai.element = Element::Cryo;
    ai.ignore_infusion = true;
    ai.mult += 0.8;
}

/// chargeAttackTruemoon: whether this charged attack is Freezing Ice (at 3
/// Icepoint with the Star out, once every 15 s), which takes the Icepoint
/// and gives 2 Frostglow.
fn charge_true_moon(s: &mut Sim, ci: usize) -> bool {
    let stv = st(s, ci);
    if !stv.true_moon || stv.icepoint < 3 || !s.mod_active(ci, SKILL) || s.mod_active(ci, CA_TRUE_MOON_ICD) {
        return false;
    }
    let first = g::CHARGE_HITMARKS[stv.gender][0];
    s.add_status(ci, CA_TRUE_MOON_ICD, 15 * 60 + first, true);
    let stv = st_mut(s, ci);
    stv.icepoint = 0;
    stv.frostglow = (stv.frostglow + 2).min(g::FROSTGLOW_MAX);
    true
}

/// queueCrystal: the Star's crystals on the enemy.
fn queue_crystal(s: &mut Sim, ci: usize, hitmarks: &[i32]) {
    let lvl = s.talent(ci, 1);
    let travel = st(s, ci).skill_travel;
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_TRAVELER_CRYO_ICICLE, icd::GROUP_TRAVELER_CRYO_ICICLE, Element::Cryo, 25.0, g::CRYSTAL[lvl]);
    ai.abil = "Frostpierce Star";
    for &d in hitmarks {
        ai.pattern = s.circle_on_target(1.2);
        s.queue_attack(ai, CB_CRYSTAL, d, d + travel);
    }
}

/// The burst's javelins: more DMG per Frostglow, two more at 8; in
/// Radiance, Stellar Glimmer DMG.
fn burst_javelins(s: &mut Sim, ci: usize, travel: i32) {
    let lvl = s.talent(ci, 2);
    let stacks = st(s, ci).frostglow;
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST[lvl] + g::FLOW_GLOW_BONUS[lvl] * stacks as f64);
    ai.abil = "Frostbound Javelin";
    match radiance(s, ci) {
        Radiance::StellarConduct => {
            ai.abil = "Frostbound Javelin (Stellar-Conduct)";
            ai.tag = AttackTag::DirectStellarConduct;
            ai.durability = 0.0;
            ai.mult = g::BURST_SSC[lvl] + g::FLOW_GLOW_BONUS_SSC[lvl] * stacks as f64;
            ai.ignore_def = 1.0;
        }
        Radiance::StellarSwirl => {
            ai.abil = "Frostbound Javelin (Stellar Swirl)";
            ai.tag = AttackTag::DirectStellarSwirl;
            ai.durability = 0.0;
            ai.mult = g::BURST_S_SW[lvl] + g::FROST_GLOW_BONUS_S_SW[lvl] * stacks as f64;
            ai.ignore_def = 1.0;
        }
        Radiance::None => {}
    }
    let hits = if stacks == g::FROSTGLOW_MAX { 5 } else { 3 };
    for &d in &g::BURST_TICK_HITMARKS[..hits] {
        ai.pattern = s.circle_on_enemy(0.0, -0.3, 4.5);
        s.queue_attack(ai, 0, 0, travel + d);
    }
    // c6OnBurst
    if s.chars[ci].cons >= 6 {
        st_mut(s, ci).c6_stacks = stacks;
        for i in 0..s.chars.len() {
            if i != ci {
                s.add_mod(i, Mod::owned(C6, ModKind::ReactBonus, Owner::Char(ci as u8), st::NONE, false, true, -1), 15 * 60);
            }
        }
    }
    st_mut(s, ci).frostglow = 0;
}

/// Lumine's lowPlungeXY and highPlungeXY, with A1's conversion on the
/// collision and the hit.
fn plunge_xy(s: &mut Sim, ci: usize, high: bool, p: &Params) -> ActInfo {
    if plunge::wants_collision(p) {
        let mut ai = plunge::collision_info(s, ci, &PLUNGE);
        a1_conversion(s, ci, &mut ai);
        let h = PLUNGE.collision_hitmark;
        s.queue_attack(ai, CB_NA_CA_PLUNGE, h, h);
    }
    let mut ai = plunge::hit_info(s, ci, high, &PLUNGE);
    a1_conversion(s, ci, &mut ai);
    let h = plunge::hitmark(&PLUNGE, high);
    s.queue_attack(ai, CB_NA_CA_PLUNGE, h, h);
    plunge::act(&PLUNGE, high)
}
