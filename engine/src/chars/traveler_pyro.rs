//! The pyro Traveler (luminepyro and aetherpyro), from gcsim's
//! internal/characters/traveler/common/pyro: normal attacks and the
//! charged attack (pyro Nightsoul attacks from C6 in the blessing),
//! Flowfire Blade (the tap's Blazing Threshold, the hold's Scorching
//! Threshold), Plains Scorcher, A1, A4 and every constellation. Lumine's
//! plunges are those from Xianyun's airborne state; gcsim has none for
//! Aether (an error there, as in gcsim).
//!
//! Its Nightsoul is the general one (`crate::nightsoul`), 80 points at
//! most: the skill enters a 12 s blessing with 42 points (plus what is
//! left outside it), which drain 0.7 every 6 frames on its own queue and
//! end the blessing when they run out.
//!
//! One module holds both: `luminepyro` and `aetherpyro` are `Char<1>` and
//! `Char<0>` (gcsim's gender), each with its own State type wrapping the
//! one here.

use super::traveler_common;
use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gamedata::chars::traveler_common_pyro as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char<const G: u8>;

pub struct State<const G: u8>(Inner);

pub struct Inner {
    /// 0 for Aether, 1 for Lumine (gcsim's gender).
    gender: usize,
    /// The src of the current blessing, -1 after it ended (gcsim's
    /// nightsoulSrc, 0 before the first).
    nightsoul_src: i32,
    /// C2's point gains in the current skill (gcsim's
    /// c2ActivationsPerSkill).
    c2_activations: i32,
    story: traveler_common::Story,
}

fn st(s: &Sim, ci: usize) -> &Inner {
    match &s.chars[ci].state {
        CharState::LuminePyro(x) => &x.0,
        CharState::AetherPyro(x) => &x.0,
        _ => unreachable!(),
    }
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut Inner {
    match &mut s.chars[ci].state {
        CharState::LuminePyro(x) => &mut x.0,
        CharState::AetherPyro(x) => &mut x.0,
        _ => unreachable!(),
    }
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const SCORCHING: ModKey = ModKey::new(g::SCORING_THRESHOLD_KEY);
const SCORCHING_ICD: ModKey = ModKey::new(g::SCORCHING_THRESHOLD_ICD_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ON_REACT_ICD);
const C1: ModKey = ModKey::new(g::C1_ATTACK_MOD_KEY);
const C2: ModKey = ModKey::new(g::C2_STATUS_KEY);
const C4: ModKey = ModKey::new("travelerpyro-c4");
const C6: ModKey = ModKey::new(g::C6_ATTACK_MOD_KEY);

const CB_PARTICLE: u16 = 1 << 0;

/// exitNightsoul, as the blessing's timer callback.
const TASK_EXIT: u8 = 0;
/// enterNightsoul, with its src (-1: the frame it runs, as the hold's).
const TASK_ENTER: u8 = 1;
/// nightsoulPointReduceFunc, with its src.
const TASK_REDUCE: u8 = 2;
/// blazingThresholdHit, with its src.
const TASK_BLAZING: u8 = 3;
/// nightsoulGainFunc, with its count.
const TASK_NS_GAIN: u8 = 4;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];
const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];

/// Lumine's airborne plunges, from gcsim's plunge.go.
const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES[1], &g::HIGH_PLUNGE_FRAMES[1], g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

impl<const G: u8> Character for Char<G> {
    const KEY: &'static str = if G == 1 { "luminepyro" } else { "aetherpyro" };
    type State = State<G>;

    fn new_state(cs: &CharSetup) -> State<G> {
        State(Inner { gender: G as usize, nightsoul_src: 0, c2_activations: 0, story: traveler_common::Story::new(cs) })
    }

    fn init(s: &mut Sim, ci: usize) {
        traveler_common::story_buffs(s, ci);
        s.chars[ci].ns.max_points = 80.0;
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // scorchingThresholdOnDamage
        s.subscribe(EventKind::EnemyDamage, o);
        // a4Init's reactions and OnNightsoulBurst, then c2Init's reactions
        // (handled in the same subscription, after A4's)
        if asc >= 4 || cons >= 2 {
            s.subscribe(EventKind::Reaction, o);
        }
        if asc >= 4 {
            s.subscribe(EventKind::NightsoulBurst, o);
        }
        // c1Init: on every character
        if cons >= 1 {
            for i in 0..s.chars.len() {
                s.add_mod(i, Mod::owned(C1, ModKind::Attack, o, st::NONE, false, false, -1), -1);
            }
        }
        if cons >= 6 {
            s.add_mod(ci, Mod::owned(C6, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Skill => match p.0.iter().find(|(k, _)| k != "hold") {
                Some((k, _)) => Some(format!("{name} param {k}")),
                None => None,
            },
            Action::Attack | Action::Charge | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            Action::HighPlunge | Action::LowPlunge => plunge::unknown_param(a, p, &[]),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, ci: usize, a: Action, p: &Params) -> Option<String> {
        if a == Action::Skill && !matches!(p.get("hold"), None | Some(0) | Some(1)) {
            return Some(format!("invalid hold param supplied, got {}", p.get("hold").unwrap()));
        }
        // gcsim implements no plunges for Aether
        if G == 0 && matches!(a, Action::LowPlunge | Action::HighPlunge) {
            return Some(format!("{}: action low_plunge not implemented", Self::KEY));
        }
        if let Some(why) = plunge::grounded(s, a) {
            return Some(why);
        }
        let _ = ci;
        crate::content::template_invalid(s, a)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "nightsoul" => Some(s.ns_condition(ci, fields)),
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
        if s.chars[ci].cons >= 6 && s.ns_has_blessing(ci) {
            nightsoul_pyro(&mut ai);
        }
        let off = g::ATTACK_OFFSETS[sex][n];
        let hb = g::ATTACK_HITBOXES[sex][n];
        ai.pattern = if (sex == 0 && (n == 0 || n == 2)) || (sex == 1 && n == 1) {
            s.box_on_player(0.0, off, hb[0], hb[1])
        } else {
            s.fan_on_player(0.0, off, hb[0], g::ATTACK_FAN_ANGLES[sex][n])
        };
        let h = g::ATTACK_HITMARKS[sex][n];
        s.queue_attack(ai, 0, h, h);
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
        if s.chars[ci].cons >= 6 && s.ns_has_blessing(ci) {
            nightsoul_pyro(&mut ai);
            // Lumine's C6 charged attack shares the enhanced one's ICD
            if sex == 1 {
                ai.icd_tag = icd::TAG_TRAVELER_ENHANCED_CA;
            }
        }
        let marks = g::CHARGE_HITMARKS[sex];
        for (i, mult) in g::CHARGE[sex].iter().enumerate() {
            ai.mult = mult[lvl];
            ai.abil = CHARGE_NAMES[i];
            ai.pattern = s.circle_on_player(0.0, 0.0, 2.2);
            s.queue_attack(ai, 0, marks[i], marks[i]);
        }
        let t = &g::CHARGE_FRAMES[sex];
        ActInfo::new(Frames::Table(t), t[0], marks[marks.len() - 1], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        // c2OnSkill
        if s.chars[ci].cons >= 2 {
            s.add_status(ci, C2, 12 * 60, true);
            st_mut(s, ci).c2_activations = 0;
        }
        if p.get("hold").unwrap_or(0) == 0 {
            // SkillTap: the blessing at 19, Blazing Threshold from 24
            let src = s.f + g::ENTER_NIGHTSOUL_DELAY;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ENTER, arg: src }, g::ENTER_NIGHTSOUL_DELAY);
            s.set_cd_with_delay(ci, Action::Skill, 18 * 60, g::TAP_CD_START);
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BLAZING, arg: src }, g::SKILL_TAP_HITMARK);
            s.delete_mod(ci, SCORCHING);
            let t = &g::SKILL_FRAMES[sex];
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill);
        }
        // SkillHold: the blessing at 48 (its src the frame it runs), the
        // hold's hit, and Scorching Threshold from now
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ENTER, arg: -1 }, 48);
        s.set_cd_with_delay(ci, Action::Skill, 18 * 60, g::HOLD_CD_START);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_TRAVELER_HOLD_DMG, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::HOLD_DMG[lvl]);
        ai.abil = "Flowfire Blade (Hold DMG)";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.pattern = s.circle_on_target(3.0);
        s.queue_attack(ai, 0, g::SKILL_HOLD_HITMARK, g::SKILL_HOLD_HITMARK);
        s.add_status(ci, SCORCHING, -1, false);
        let t = &g::SKILL_HOLD_FRAMES[sex];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        s.set_cd(ci, Action::Burst, 18 * 60);
        s.consume_energy(ci, 19);
        // c4AddMod
        if s.chars[ci].cons >= 4 {
            s.add_stat_mod(ci, C4, st::PYROP, 0.2, 9 * 60, true);
        }
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BURST[lvl]);
        ai.abil = "Plains Scorcher";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.pattern = s.circle_on_target(4.5);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NS_GAIN, arg: 0 }, g::NIGHTSOUL_GAIN_DELAYS[0]);
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
            TASK_EXIT => exit_nightsoul(s, ci),
            TASK_ENTER => {
                let src = if arg < 0 { s.f } else { arg };
                enter_nightsoul(s, ci, src);
            }
            TASK_REDUCE => {
                if st(s, ci).nightsoul_src != arg || !s.ns_has_blessing(ci) {
                    return;
                }
                s.ns_consume(ci, g::NIGHTSOUL_REDUCE_VAL);
                if s.ns_points(ci) <= 0.00001 && s.ns_has_blessing(ci) {
                    exit_nightsoul(s, ci);
                }
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_REDUCE, arg }, g::NIGHTSOUL_REDUCE_DELAY);
            }
            TASK_BLAZING => {
                if st(s, ci).nightsoul_src != arg || !s.ns_has_blessing(ci) {
                    return;
                }
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_TRAVELER_BLAZING_THRESHOLD, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BLAZING_THRESHOLD[lvl]);
                ai.abil = "Blazing Threshold DMG";
                ai.add_tags = add_tag::NIGHTSOUL;
                let r = if s.chars[ci].ascension >= 1 && s.ns_points(ci) >= 20.0 { 3.0 } else { 0.5 };
                ai.pattern = s.circle_on_player(0.0, 0.0, r);
                s.queue_attack(ai, CB_PARTICLE, 0, 0);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BLAZING, arg }, g::BLAZING_THRESHOLD_INTERVAL);
            }
            TASK_NS_GAIN => {
                s.ns_generate(ci, 7.0);
                if arg < 3 {
                    let next = arg + 1;
                    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NS_GAIN, arg: next }, g::NIGHTSOUL_GAIN_DELAYS[next as usize]);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, (2.9 * 60.0) as i32, true);
            s.queue_particle(1.0, Element::Pyro, s.particle_delay[ci]);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        traveler_common::story_amount(&st(s, ci).story, m, out)
    }

    fn attack_mod(s: &Sim, ci: usize, on: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C1 {
            // the character must be active, the Traveler in its blessing
            if s.player.active as usize != on || !s.ns_has_blessing(ci) {
                return;
            }
            let mut v = 0.06;
            if s.mod_active(on, crate::nightsoul::NIGHTSOUL_BLESSING) {
                v += 0.09;
            }
            ev.stats[st::DMGP] += v;
        } else if m.key == C6 {
            if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) && ev.info.has_tag(add_tag::NIGHTSOUL) {
                ev.stats[st::CD] += 0.4;
            }
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyDamage(ev) => scorching_threshold(s, ci, ev),
            Event::Reaction(r, ev) => {
                if !matches!(r, Reaction::Burning | Reaction::Vaporize | Reaction::Melt | Reaction::Overload | Reaction::Burgeon | Reaction::SwirlPyro | Reaction::CrystallizePyro) {
                    return;
                }
                // A4: a reaction by the active character in the blessing
                if s.chars[ci].ascension >= 4 && ev.info.actor == s.player.active && s.ns_has_blessing(ci) && !s.mod_active(ci, A4_ICD) {
                    s.add_status(ci, A4_ICD, 12 * 60, true);
                    s.add_energy(ci, 5.0);
                }
                // C2: 14 points, twice a skill
                if s.chars[ci].cons >= 2 && s.mod_active(ci, C2) && st(s, ci).c2_activations < 2 {
                    st_mut(s, ci).c2_activations += 1;
                    s.ns_generate(ci, 14.0);
                }
            }
            Event::NightsoulBurst(_) => s.add_energy(ci, 4.0),
            _ => {}
        }
    }
}

/// C6's conversion of a normal, charged or plunging attack in the blessing.
fn nightsoul_pyro(ai: &mut AttackInfo) {
    ai.element = Element::Pyro;
    ai.ignore_infusion = true;
    ai.add_tags = add_tag::NIGHTSOUL;
}

/// Scorching Threshold: while the hold's status lasts, damage on the enemy
/// (not burning, not hydro swirl, not zero) sets off a hit every 3 s.
fn scorching_threshold(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    if s.mod_active(ci, SCORCHING_ICD) || !s.mod_active(ci, SCORCHING) {
        return;
    }
    if matches!(ev.info.tag, AttackTag::BurningDamage | AttackTag::SwirlHydro) || ev.damage == 0.0 {
        return;
    }
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_TRAVELER_SCORCHING_THRESHOLD, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SCORCHING_THRESHOLD[lvl]);
    ai.abil = "Scorching Threshold";
    ai.add_tags = add_tag::NIGHTSOUL;
    let r = if s.chars[ci].ascension >= 1 && s.ns_points(ci) >= 20.0 { 4.0 } else { 1.5 };
    ai.pattern = s.circle_on_player(0.0, 0.0, r);
    let d = g::SCORCHING_THRESHOLD_HITMARK_DELAY;
    s.queue_attack(ai, CB_PARTICLE, d, d);
    s.add_status(ci, SCORCHING_ICD, g::SCORCHING_THRESHOLD_ICD, true);
}

/// enterNightsoul: 42 points (plus those left outside the blessing), for
/// 12 s, draining from now.
fn enter_nightsoul(s: &mut Sim, ci: usize, src: i32) {
    let mut points = 42.0;
    if !s.ns_has_blessing(ci) {
        points += s.ns_points(ci);
    }
    st_mut(s, ci).nightsoul_src = src;
    s.ns_enter_timed(ci, points, 12 * 60, TASK_EXIT);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_REDUCE, arg: src }, g::NIGHTSOUL_REDUCE_DELAY);
}

fn exit_nightsoul(s: &mut Sim, ci: usize) {
    s.delete_mod(ci, SCORCHING);
    s.ns_exit(ci);
    s.ns_clear(ci);
    st_mut(s, ci).nightsoul_src = -1;
}

/// Lumine's lowPlungeXY and highPlungeXY: pyro Nightsoul hits from C6 in
/// the blessing (not the collision).
fn plunge_xy(s: &mut Sim, ci: usize, high: bool, p: &Params) -> ActInfo {
    if plunge::wants_collision(p) {
        let ai = plunge::collision_info(s, ci, &PLUNGE);
        let h = PLUNGE.collision_hitmark;
        s.queue_attack(ai, 0, h, h);
    }
    let mut ai = plunge::hit_info(s, ci, high, &PLUNGE);
    if s.chars[ci].cons >= 6 && s.ns_has_blessing(ci) {
        nightsoul_pyro(&mut ai);
    }
    let h = plunge::hitmark(&PLUNGE, high);
    s.queue_attack(ai, 0, h, h);
    plunge::act(&PLUNGE, high)
}
