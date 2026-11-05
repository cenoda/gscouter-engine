//! Ifa, from gcsim's internal/characters/ifa: normal attacks, charged
//! attack, Airborne Disease Prevention (the skill: Nightsoul's Blessing,
//! the Tonic Shots of his normal and charged attacks in it, the cancel and
//! the plunging cancel), Compound Sedation Field (the burst and its
//! Sedation Marks), the low plunge after the skill ends, the dash in the
//! blessing, A1, A4 and every constellation.
//!
//! His Nightsoul's Blessing is the general one (`crate::nightsoul`): 80
//! points without a limit, draining 0.8 every 6 frames on the fight's
//! queue; it ends when they run out or when the skill is pressed again.
//! Nothing costs stamina in it and he cannot swap out while he has points.

use crate::content::Character;
use crate::gamedata::chars::ifa as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    nightsoul_src: i32,
    skill_particle_icd: bool,
    skill_last_stam_f: i32,
    sedation_mark_counter: i32,
    /// The ATK SPD read by the last held Tonic Shot, for its frames.
    hold_atkspd: f64,
}

const PLUNGE_AVAILABLE: ModKey = ModKey::new(g::PLUNGE_AVAILABLE_KEY);
const HEAL_TAP_ICD: ModKey = ModKey::new(g::HEAL_TAP_ICD_KEY);
const HEAL_HOLD_ICD: ModKey = ModKey::new(g::HEAL_HOLD_ICD_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ENERGY_ICD_KEY);
const A1: ModKey = ModKey::new("ifa-a1");
const A4: ModKey = ModKey::new("ifa-a4");
const C4: ModKey = ModKey::new("ifa-c4");

const CB_PARTICLE: u16 = 1 << 0;
const CB_HEAL_TAP: u16 = 1 << 1;
const CB_HEAL_HOLD: u16 = 1 << 2;
const CB_C1: u16 = 1 << 3;

/// nightsoulPointReduceFunc, with its src.
const TASK_REDUCE: u8 = 0;
/// A Tonic Shot, 3 (tap) or 1 (hold) frames after the action: arg is
/// travel * 4 + the kind (TONIC_*).
const TASK_TONIC: u8 = 1;
/// The skill plunge's end.
const TASK_PLUNGE_END: u8 = 2;
/// The burst's Sedation Marks.
const TASK_SEDATION: u8 = 3;

const TONIC_TAP: i32 = 0;
const TONIC_HOLD: i32 = 1;
const TONIC_C6: i32 = 2;

/// frames_live: the burst's, by whether he is in the blessing.
const FR_BURST: u8 = 0;
/// A held Tonic Shot's, with C6's extension in arg.
const FR_HOLD: u8 = 1;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];

impl Character for Char {
    const KEY: &'static str = "ifa";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { nightsoul_src: 0, skill_particle_icd: false, skill_last_stam_f: 0, sedation_mark_counter: 0, hold_atkspd: 0.0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let asc = s.chars[ci].ascension;
        // a1Init: swirl, electro-charged, lunar-charged and Stellar Swirl
        // bonus from the team's Nightsoul points, on every member
        if asc >= 1 {
            for i in 0..s.chars.len() {
                s.add_mod(i, Mod::owned(A1, ModKind::ReactBonus, o, st::NONE, false, false, -1), -1);
            }
        }
        if asc >= 4 {
            s.subscribe(EventKind::NightsoulBurst, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let allowed: &[&str] = match a {
            Action::Attack | Action::Charge => &["travel"],
            Action::Skill => &["hold", "collision"],
            Action::LowPlunge => &["collision"],
            Action::Burst => &[],
            _ => return Some(name.into()),
        };
        p.0.iter().find(|(k, _)| !allowed.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
    }

    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        match a {
            Action::LowPlunge if !s.mod_active(ci, PLUNGE_AVAILABLE) => Some("only plunge after skill ends".into()),
            Action::Jump if s.ns_has_blessing(ci) => Some("ifa jump in nightsoul blessing not implemented".into()),
            _ => crate::content::template_invalid(s, a),
        }
    }

    /// No swap while he has points.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        if a == Action::Swap && s.ns_points(ci) > 0.0 {
            return Some(Some(Failure::SwapCd));
        }
        None
    }

    fn action_stam(s: &Sim, ci: usize, _: Action) -> Option<f64> {
        s.ns_has_blessing(ci).then_some(0.0)
    }

    /// The dash in the blessing: 13.5 points, the dash's cooldown, no
    /// stamina.
    fn own_dash(s: &mut Sim, ci: usize, _: &Params) -> Option<ActInfo> {
        if !s.ns_has_blessing(ci) {
            return None;
        }
        reduce_points(s, ci, 13.5);
        s.apply_dash_cd();
        let t = &g::SKILL_DASH_FRAMES;
        Some(ActInfo::new(Frames::Table(t), t[0], t[Action::Skill as usize], AnimState::Dash))
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").unwrap_or(0) as i32;
        if s.ns_has_blessing(ci) {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_TONIC, arg: travel * 4 + TONIC_TAP }, 3);
            let atkspd = s.stat(ci, st::ATKSPD);
            return ActInfo::new(Frames::Attack { table: &g::SKILL_ATTACK_TAP_FRAMES, atkspd }, g::ATTACK_SKILL_INTERVAL, 0, AnimState::Normal);
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.circle_on_target(g::ATTACK_RADIUS);
        s.queue_attack_at(ci, ai, 0, g::ATTACK_HITMARKS[n] + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], g::ATTACK_HITMARKS[n], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").unwrap_or(0) as i32;
        if s.ns_has_blessing(ci) {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_TONIC, arg: travel * 4 + TONIC_HOLD }, 1);
            let ext = c6_on_hold(s, ci);
            let atkspd = s.stat(ci, st::ATKSPD);
            if ext == 0 {
                return ActInfo::new(Frames::Attack { table: &g::SKILL_ATTACK_HOLD_FRAMES, atkspd }, g::ATTACK_SKILL_INTERVAL, 0, AnimState::Normal);
            }
            s.st_mut::<State>(ci).hold_atkspd = atkspd;
            return ActInfo::new(Frames::CharLive { id: FR_HOLD, arg: ext as i16 }, g::ATTACK_SKILL_INTERVAL + ext, 0, AnimState::Normal);
        }
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.pattern = s.circle_on_target(g::CHARGE_RADIUS);
        let h = g::CHARGE_HITMARK + travel;
        s.queue_attack(ai, 0, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Charge)
    }

    /// Out of the blessing, the blessing; in it, its end, or with hold=1
    /// the plunging cancel.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.ns_has_blessing(ci) {
            if p.get("hold") == Some(1) {
                return skill_plunge(s, ci, p);
            }
            exit_nightsoul(s, ci);
            let t = &g::SKILL_CANCEL_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::LowPlunge as usize], AnimState::Skill);
        }
        enter_nightsoul(s, ci);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Burst as usize], AnimState::Skill)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        s.delete_mod(ci, PLUNGE_AVAILABLE);
        if p.get("collision").unwrap_or(0) > 0 {
            plunge_collision(s, ci, g::COLLISION_HITMARK);
        }
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::PLUNGE_LOW[lvl]);
        ai.abil = "Low Plunge Attack";
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
        let h = g::LOW_PLUNGE_HITMARK;
        s.queue_attack(ai, 0, h, h);
        let t = &g::LOW_PLUNGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Plunge)
    }

    /// Compound Sedation Field.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST_DMG[lvl]);
        ai.abil = "Compound Sedation Field";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.pattern = s.circle_on_target(6.0);
        s.st_mut::<State>(ci).sedation_mark_counter = 0;
        s.add_task(Task::Char { c: ci as u8, id: TASK_SEDATION, arg: 0 }, g::BURST_HITMARK);
        let h = g::BURST_HITMARK;
        s.queue_attack(ai, 0, h, h);
        // c4OnBurst: EM +100 for 15 s
        if s.chars[ci].cons >= 4 {
            s.add_stat_mod(ci, C4, st::EM, 100.0, 15 * 60, true);
        }
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 4);
        ActInfo::new(Frames::CharLive { id: FR_BURST, arg: 0 }, g::BURST_FRAMES_GROUNDED[0], g::BURST_FRAMES_NS[Action::Dash as usize], AnimState::Burst)
    }

    fn frames_live(s: &Sim, ci: usize, id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_BURST => {
                if s.ns_has_blessing(ci) {
                    g::BURST_FRAMES_NS[next as usize]
                } else {
                    g::BURST_FRAMES_GROUNDED[next as usize]
                }
            }
            FR_HOLD => atk_spd_adjust(g::SKILL_ATTACK_HOLD_FRAMES[next as usize] + arg as i32, s.st::<State>(ci).hold_atkspd),
            _ => unreachable!(),
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_REDUCE => {
                if s.st::<State>(ci).nightsoul_src != arg {
                    return;
                }
                reduce_points(s, ci, 0.8);
                s.add_task(Task::Char { c: ci as u8, id: TASK_REDUCE, arg }, 6);
            }
            TASK_TONIC => {
                if !s.ns_has_blessing(ci) {
                    return;
                }
                let (travel, kind) = (arg >> 2, arg & 3);
                let lvl = s.talent(ci, 1);
                let mult = if kind == TONIC_C6 { 1.2 } else { g::SKILL_DMG[lvl] };
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_IFA_SKILL, icd::GROUP_IFA_SKILL_HIT, Element::Anemo, 25.0, mult);
                ai.abil = if kind == TONIC_C6 { "Tonic Shot C6" } else { "Tonic Shot" };
                ai.add_tags = add_tag::NIGHTSOUL;
                ai.pattern = s.circle_on_target(3.0);
                let cbs = match kind {
                    TONIC_TAP => CB_PARTICLE | CB_HEAL_TAP | CB_C1,
                    TONIC_HOLD => CB_PARTICLE | CB_HEAL_HOLD | CB_C1,
                    _ => 0,
                };
                s.queue_attack(ai, cbs, travel, travel);
            }
            TASK_PLUNGE_END => {
                exit_nightsoul(s, ci);
                s.delete_mod(ci, PLUNGE_AVAILABLE);
                s.player.swap_cd = 0;
            }
            TASK_SEDATION => {
                if s.enemy_alive && s.enemy_within(s.circle_on_target(6.0)) {
                    sedation_mark(s, ci);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.st::<State>(ci).skill_particle_icd {
            s.st_mut::<State>(ci).skill_particle_icd = true;
            let n = if s.rng.float64() < 0.3 { 5.0 } else { 4.0 };
            s.queue_particle(n, Element::Anemo, s.particle_delay[ci]);
        }
        if cbs & CB_HEAL_TAP != 0 && !s.mod_active(ci, HEAL_TAP_ICD) {
            s.add_status(ci, HEAL_TAP_ICD, 6, true);
            heal_skill(s, ci);
        }
        if cbs & CB_HEAL_HOLD != 0 && !s.mod_active(ci, HEAL_HOLD_ICD) {
            s.add_status(ci, HEAL_HOLD_ICD, 6, true);
            heal_skill(s, ci);
        }
        if cbs & CB_C1 != 0 && s.chars[ci].cons >= 1 && !s.mod_active(ci, C1_ICD) {
            s.add_energy(ci, 6.0);
            s.add_status(ci, C1_ICD, 8 * 60, true);
        }
    }

    /// A1: while he is in the blessing, 1.5% swirl and electro-charged DMG
    /// per point of the team's Nightsoul, 0.2% lunar-charged and Stellar
    /// Swirl DMG, up to 150 points (200 from C2, whose points above 60
    /// count four times).
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if !s.ns_has_blessing(ci) {
            return 0.0;
        }
        let mut ns: f64 = s.chars.iter().map(|c| c.ns.points).sum();
        let cons = s.chars[ci].cons;
        if cons >= 2 {
            ns += (ns - 60.0).max(0.0) * 3.0;
        }
        let cap = if cons >= 2 && s.chars[ci].ascension >= 1 { 50.0 } else { 0.0 };
        ns = ns.min(150.0 + cap);
        match info.tag {
            AttackTag::SwirlPyro | AttackTag::SwirlHydro | AttackTag::SwirlElectro | AttackTag::SwirlCryo | AttackTag::ECDamage => 0.015 * ns,
            AttackTag::ReactionLunarCharge | AttackTag::DirectLunarCharged | AttackTag::ReactionStellarSwirl | AttackTag::DirectStellarSwirl => 0.002 * ns,
            _ => 0.0,
        }
    }

    /// A4: EM +80 for 10 s on a Nightsoul Burst.
    fn event(s: &mut Sim, ci: usize, e: &crate::content::Event) {
        if let crate::content::Event::NightsoulBurst(_) = e {
            s.add_stat_mod(ci, A4, st::EM, 80.0, 10 * 60, true);
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<crate::gcsl::Num, String>> {
        (fields[0] == "nightsoul").then(|| s.ns_condition(ci, fields))
    }
}

fn reduce_points(s: &mut Sim, ci: usize, v: f64) {
    s.ns_consume(ci, v);
    if s.ns_points(ci) < 0.001 {
        exit_nightsoul(s, ci);
    }
}

fn enter_nightsoul(s: &mut Sim, ci: usize) {
    s.st_mut::<State>(ci).skill_particle_icd = false;
    s.ns_enter(ci, 80.0);
    let f = s.f;
    s.st_mut::<State>(ci).nightsoul_src = f;
    s.add_task(Task::Char { c: ci as u8, id: TASK_REDUCE, arg: f }, 4);
    s.st_mut::<State>(ci).skill_last_stam_f = s.player.last_stam_use;
}

fn exit_nightsoul(s: &mut Sim, ci: usize) {
    if !s.ns_has_blessing(ci) {
        return;
    }
    s.player.swap_cd = 37;
    s.ns_exit(ci);
    s.ns_clear(ci);
    s.st_mut::<State>(ci).nightsoul_src = -1;
    s.set_cd(ci, Action::Skill, 450);
    s.chars[ci].normal_hit_num = g::NORMAL_HIT_NUM;
    s.chars[ci].normal_counter = 0;
    s.player.last_stam_use = s.st::<State>(ci).skill_last_stam_f;
    s.add_status(ci, PLUNGE_AVAILABLE, 26, true);
}

/// The plunging cancel: a Nightsoul low plunge, and the blessing's end at
/// its hitmark.
fn skill_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
    s.delete_mod(ci, PLUNGE_AVAILABLE);
    if p.get("collision").unwrap_or(0) > 0 {
        plunge_collision(s, ci, g::COLLISION_HITMARK);
    }
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::PLUNGE_LOW[lvl]);
    ai.abil = "Low Plunge Attack";
    ai.add_tags = add_tag::NIGHTSOUL;
    ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
    let h = g::SKILL_CANCEL_PLUNGE_HITMARK;
    s.queue_attack(ai, 0, h, h);
    s.add_task(Task::Char { c: ci as u8, id: TASK_PLUNGE_END, arg: 0 }, h);
    let t = &g::SKILL_CANCEL_PLUNGE_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Plunge)
}

fn plunge_collision(s: &mut Sim, ci: usize, delay: i32) {
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 0.0, g::COLLISION[lvl]);
    ai.abil = "Plunge Collision";
    if s.ns_has_blessing(ci) {
        ai.add_tags = add_tag::NIGHTSOUL;
    }
    ai.pattern = s.circle_on_player(0.0, 0.0, 1.5);
    s.queue_attack(ai, 0, delay, delay);
}

/// Tonic Shot's heal of the party.
fn heal_skill(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 1);
    let em = s.stat(ci, st::EM);
    let amt = g::SKILL_HEAL[lvl] * em + g::SKILL_HEAL_FLAT[lvl];
    let bonus = s.stat(ci, st::HEAL);
    s.heal_info(ci, None, false, amt, bonus);
}

/// C6: half the held Tonic Shots fire another, 11 frames longer.
fn c6_on_hold(s: &mut Sim, ci: usize) -> i32 {
    if s.chars[ci].cons < 6 || s.rng.float64() > 0.5 {
        return 0;
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_TONIC, arg: TONIC_C6 }, g::C6_EXTENSION_FRAME + 1);
    g::C6_EXTENSION_FRAME
}

/// A Sedation Mark on the enemy: up to four per burst, of the first of
/// pyro, hydro, electro and cryo in its aura.
fn sedation_mark(s: &mut Sim, ci: usize) {
    let st = s.st_mut::<State>(ci);
    if st.sedation_mark_counter >= g::SEDATION_MARK_CAP {
        return;
    }
    st.sedation_mark_counter += 1;
    let Some(ele) = [Element::Pyro, Element::Hydro, Element::Electro, Element::Cryo].into_iter().find(|&e| s.enemy_aura_contains(e)) else {
        return;
    };
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_IFA_SEDATION_MARK, icd::GROUP_IFA_SEDATION_MARK, ele, 25.0, g::BURST_MARK[lvl]);
    ai.abil = "Sedation Mark";
    ai.add_tags = add_tag::NIGHTSOUL;
    ai.pattern = s.circle_on_target(2.5);
    let h = g::SEDATION_MARK_HITMARK;
    s.queue_attack(ai, 0, h, h);
}
