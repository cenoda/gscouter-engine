//! Xilonen, from gcsim's internal/characters/xilonen: normal attacks and
//! the Blade Roller (her normal attacks in Nightsoul's Blessing), charged
//! attack, Yohual's Scratch (the skill, its Nightsoul's Blessing and its
//! recast), Ocelotlicue Point! (the damaging and the healing burst), the
//! Samplers' RES shred, the dash's and jump's Nightsoul points, A1, A4 and
//! every constellation.
//!
//! Her Nightsoul's Blessing is the general one (`crate::nightsoul`): 45
//! points, 90 at most, that drain 0.5 every 6 frames on her own queue; it
//! ends when they run out (not in a normal attack), when she leaves the
//! field, when the skill is pressed again, or after 9 s (waiting for the
//! end of a normal attack). Her plunges are those from Xianyun's airborne
//! state: geo Nightsoul damage while she can use Nightsoul (the high one;
//! gcsim forbids the low one then).

use crate::content::{Character, Event};
use crate::gamedata::chars::xilonen as g;
use crate::chars::plunge;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The frame her blessing started, -1 after it ended (gcsim's
    /// nightsoulSrc, 0 before the first).
    nightsoul_src: i32,
    sample_src: i32,
    c2_src: i32,
    /// The other members of pyro, hydro, cryo or electro.
    samplers_converted: i32,
    /// The elements her Samplers shred, as bits by Element.
    shred_elements: u16,
    /// The player's LastStamUse as the skill froze stamina.
    skill_last_stam_f: i32,
    /// C4's stacks by character (gcsim's tag "xilonen-c4").
    c4_tags: [i32; 4],
}

const SKILL_RECAST_CD: ModKey = ModKey::new(g::SKIL_RECAST_CD);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const ACTIVE_SAMPLER: ModKey = ModKey::new(g::ACTIVE_SAMPLER_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const A1_ICD: ModKey = ModKey::new(g::A1_ICD_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const C2: ModKey = ModKey::new(g::C2KEY);
const C2_BUFF: ModKey = ModKey::new(g::C2_BUFF_KEY);
const C4: ModKey = ModKey::new(g::C4KEY);
const C6: ModKey = ModKey::new(g::C6KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

/// gcsim's "xilonen-e-shred-<element>".
fn shred_key(e: Element) -> ModKey {
    match e {
        Element::Geo => ModKey::new("xilonen-e-shred-geo"),
        Element::Pyro => ModKey::new("xilonen-e-shred-pyro"),
        Element::Hydro => ModKey::new("xilonen-e-shred-hydro"),
        Element::Cryo => ModKey::new("xilonen-e-shred-cryo"),
        Element::Electro => ModKey::new("xilonen-e-shred-electro"),
        _ => unreachable!(),
    }
}

const CB_PARTICLE: u16 = 1 << 0;
const CB_A1: u16 = 1 << 1;

/// exitNightsoul, as the blessing's timer callback.
const TASK_EXIT: u8 = 0;
/// nightsoulPointReduceTask, with its src.
const TASK_REDUCE: u8 = 1;
/// activeGeoSampler, with its src.
const TASK_GEO_SAMPLER: u8 = 2;
/// activeSamplers, with its src (on the active character's queue).
const TASK_SAMPLERS: u8 = 3;
/// applyC2Buff, with src * 4 + the character.
const TASK_C2_BUFF: u8 = 4;
/// C6's heals (the fight's queue).
const TASK_C6_HEAL: u8 = 5;
/// The healing burst's heals (the fight's queue).
const TASK_BURST_HEAL: u8 = 6;
/// After a plunge's hitmark: out of Nightsoul, the blessing ends.
const TASK_PLUNGE_EXIT: u8 = 7;

/// The Blade Roller's OnRemoved.
const REMOVED_ROLLER: u8 = 0;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];
const ROLLER_NAMES: [&str; 4] = ["Blade Roller 0", "Blade Roller 1", "Blade Roller 2", "Blade Roller 3"];

/// The airborne plunges (Xianyun's), from gcsim's plunge.go.
const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

impl Character for Char {
    const KEY: &'static str = "xilonen";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { nightsoul_src: 0, sample_src: 0, c2_src: 0, samplers_converted: 0, shred_elements: 0, skill_last_stam_f: 0, c4_tags: [0; 4] }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].ns.max_points = 90.0;
        s.chars[ci].ns.set_extend_states(&[AnimState::Normal, AnimState::Plunge]);
        let (mut converted, mut shred) = (0, 0u16);
        for i in 0..s.chars.len() {
            if i == ci {
                continue;
            }
            match s.chars[i].element {
                e @ (Element::Pyro | Element::Hydro | Element::Cryo | Element::Electro) => {
                    converted += 1;
                    shred |= 1 << e as u16;
                }
                _ => shred |= 1 << Element::Geo as u16,
            }
        }
        if s.chars.len() < 4 {
            shred |= 1 << Element::Geo as u16;
        }
        let st = s.st_mut::<State>(ci);
        st.samplers_converted = converted;
        st.shred_elements = shred;

        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // A1: with two converted Samplers or more, its swap hook (in the
        // one subscription below: none comes between them), otherwise DMG
        if asc >= 1 && converted < 2 {
            s.chars[ci].mods.push(Mod::owned(A1, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        // A4
        if asc >= 4 {
            s.subscribe(EventKind::NightsoulBurst, o);
        }
        // C2: geo's Sampler always on, and the geo members' DMG +50%
        if cons >= 2 && converted < 3 {
            geo_sampler(s, ci, -1);
            for i in 0..s.chars.len() {
                if s.chars[i].element == Element::Geo {
                    let mut m = Mod::new(C2_BUFF, ModKind::Attack, st::DMGP, false, false, -1);
                    m.value = 0.5;
                    s.add_mod(i, m, -1);
                }
            }
        }
        if cons >= 4 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        if cons >= 6 {
            s.subscribe(EventKind::Attack, o);
            s.subscribe(EventKind::Dash, o);
            s.subscribe(EventKind::Plunge, o);
        }
        // A1's swap hook and onExitField
        s.subscribe(EventKind::Swap, o);
    }

    /// gcsim reads no params of hers but a plunge's collision.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            Action::HighPlunge | Action::LowPlunge => plunge::unknown_param(a, p, &[]),
            _ => Some(name.into()),
        }
    }

    /// No charged attack in Nightsoul's Blessing; then the template's.
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::Charge && can_use_nightsoul(s, ci) {
            return Some("xilonen cannot charge in nightsoul blessing".into());
        }
        if let Some(why) = plunge::grounded(s, a) {
            return Some(why);
        }
        // gcsim checks this as the plunge runs
        if a == Action::LowPlunge && can_use_nightsoul(s, ci) {
            return Some("xilonen cannot low_plunge while in nightsoul blessing".into());
        }
        crate::content::template_invalid(s, a)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge_xy(s, ci, false, p)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge_xy(s, ci, true, p)
    }

    /// The skill waits out its recast's 1 s lockout (with no failure).
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        if a == Action::Skill && s.mod_active(ci, SKILL_RECAST_CD) {
            return Some(Some(Failure::NoFailure));
        }
        None
    }

    /// Nothing costs stamina while she can use Nightsoul.
    fn action_stam(s: &Sim, ci: usize, _: Action) -> Option<f64> {
        can_use_nightsoul(s, ci).then_some(0.0)
    }

    /// The dash uses 5 points, a jump out of a dash 20.
    fn template_action(s: &mut Sim, ci: usize, a: Action) {
        match a {
            Action::Dash if can_use_nightsoul(s, ci) => reduce_points(s, ci, 5.0),
            Action::Jump if s.current_state() == AnimState::Dash && can_use_nightsoul(s, ci) => reduce_points(s, ci, 20.0),
            _ => {}
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if can_use_nightsoul(s, ci) {
            return roller(s, ci);
        } else if s.ns_has_blessing(ci) {
            exit_nightsoul(s, ci);
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let marks = g::ATTACK_HITMARKS[n];
        for (i, &h) in marks.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAMES[n] * 60.0;
            ai.hitlag_factor = 0.01;
            ai.def_halt = true;
            ai.pattern = s.fan_on_player(0.0, g::ATTACK_OFFSETS[n][i], g::ATTACK_HITBOXES[n][i], g::ATTACK_FAN_ANGLES[n][i]);
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
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CA[lvl]);
        ai.abil = "Charge";
        ai.blunt = true;
        ai.poise = 120.0;
        ai.pattern = s.box_on_player(0.0, -1.8, 2.0, 4.5);
        let h = g::CHARGE_HITMARK;
        s.queue_attack(ai, 0, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Charge)
    }

    /// Yohual's Scratch: the rush and her Nightsoul's Blessing; in the
    /// blessing, the blessing's end.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.ns_has_blessing(ci) {
            exit_nightsoul(s, ci);
            return ActInfo::new(Frames::Const(1), 1, 1, AnimState::Skill);
        }
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL_DMG[lvl]);
        ai.abil = "Yohual's Scratch";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.hitlag_factor = 0.01;
        ai.use_def = true;
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = s.circle_on_player(0.0, 1.0, 0.8);
        let h = g::SKILL_HITMARKS;
        s.queue_attack(ai, CB_PARTICLE, h, h);
        s.add_status(ci, SKILL_RECAST_CD, 60, true);
        // gcsim's RestoreStam: 5, or up to 15
        let stam = s.player.stam;
        let v = if stam >= 15.0 { 5.0 } else { 15.0 - stam };
        s.player.stam = (stam + v).min(240.0);
        enter_nightsoul(s, ci);
        if s.chars[ci].cons >= 4 {
            for i in 0..s.chars.len() {
                s.add_status(i, C4, 15 * 60, true);
                s.st_mut::<State>(ci).c4_tags[i] = 6;
            }
        }
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    /// Ocelotlicue Point!: three hits with fewer than two converted
    /// Samplers, otherwise one hit and the active character's heals.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::BURST_DMG[lvl]);
        ai.abil = "Ocelotlicue Point!";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.blunt = true;
        ai.poise = 75.0;
        ai.use_def = true;
        ai.hitlag_factor = 0.05;
        ai.pattern = s.circle_on_player(0.0, 0.0, 7.0);
        if s.st::<State>(ci).samplers_converted < 2 {
            for (i, &h) in g::BURST_DAMAGE_HITMARKS.iter().enumerate() {
                if i > 0 {
                    ai.abil = "Follow-Up Beat";
                    ai.mult = g::BURST_FOLLOW_DMG[lvl];
                }
                s.queue_attack(ai, 0, h, h);
            }
        } else {
            let h = g::BURST_DAMAGE_HITMARKS[0];
            s.queue_attack(ai, 0, h, h);
            for &h in g::BURST_HEAL_HITMARKS {
                s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_HEAL, arg: 0 }, h);
            }
        }
        s.consume_energy(ci, 16);
        s.set_cd(ci, Action::Burst, 15 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn on_removed(s: &mut Sim, ci: usize, id: u8, _: AnimState) {
        if id == REMOVED_ROLLER && !can_use_nightsoul(s, ci) {
            exit_nightsoul(s, ci);
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_EXIT => exit_nightsoul(s, ci),
            TASK_REDUCE => {
                if s.st::<State>(ci).nightsoul_src != arg {
                    return;
                }
                // 0.5 points every 6 frames
                reduce_points(s, ci, 0.5);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_REDUCE, arg }, 6);
            }
            TASK_GEO_SAMPLER => geo_sampler(s, ci, arg),
            TASK_SAMPLERS => samplers(s, ci, arg),
            TASK_C2_BUFF => {
                let (src, other) = (arg >> 2, (arg & 3) as usize);
                if s.st::<State>(ci).c2_src != src || !s.mod_active(ci, C2) {
                    return;
                }
                let (stat, v) = match s.chars[other].element {
                    Element::Pyro => (st::ATKP, 0.45),
                    Element::Hydro => (st::HPP, 0.45),
                    Element::Cryo => (st::CD, 0.60),
                    _ => unreachable!(),
                };
                s.add_stat_mod(other, C2_BUFF, stat, v, 60, true);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C2_BUFF, arg }, g::C2_INTERVAL as i32);
            }
            TASK_C6_HEAL => {
                if !s.mod_active(ci, C6) {
                    return;
                }
                let bonus = s.stat(ci, st::HEAL);
                let heal = total_def(&s.stats(ci)) * 1.2;
                s.heal_info(ci, None, false, heal, bonus);
            }
            TASK_PLUNGE_EXIT => {
                if !can_use_nightsoul(s, ci) {
                    exit_nightsoul(s, ci);
                }
            }
            TASK_BURST_HEAL => {
                let lvl = s.talent(ci, 2);
                let target = s.player.active as usize;
                let src = g::BURST_HEAL_BASE[lvl] + total_def(&s.stats(ci)) * g::BURST_HEAL_PER[lvl];
                let bonus = s.stat(ci, st::HEAL);
                s.heal(ci, target, src, bonus);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 30, true);
            s.queue_particle(4.0, Element::Geo, s.particle_delay[ci]);
        }
        if cbs & CB_A1 != 0 {
            a1_cb(s, ci, ev);
        }
    }

    /// A1's DMG +30% on her Nightsoul normal and plunging attacks.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        let nightsoul_na = matches!(ev.info.tag, AttackTag::Normal | AttackTag::Plunge) && ev.info.has_tag(add_tag::NIGHTSOUL);
        if m.key == A1 {
            if nightsoul_na {
                ev.stats[st::DMGP] += 0.30;
            }
        } else if m.key == C6 && nightsoul_na {
            // C6: flat damage of 300% of her DEF
            ev.info.flat_dmg += total_def(&s.stats(ci)) * 3.0;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { prev, .. } => {
                // A1's swap hook comes first, then onExitField
                if s.chars[ci].ascension >= 1 && s.st::<State>(ci).samplers_converted >= 2 && s.mod_active(ci, ACTIVE_SAMPLER) {
                    let f = s.f;
                    s.st_mut::<State>(ci).sample_src = f;
                    samplers(s, ci, f);
                }
                if prev == ci {
                    exit_nightsoul(s, ci);
                }
            }
            // A4: DEF +20% for 15 s
            Event::NightsoulBurst(_) => s.add_stat_mod(ci, A4, st::DEFP, 0.20, 15 * 60, true),
            // C4: a normal, charged or plunging attack of a member with
            // stacks left adds 65% of her DEF
            Event::EnemyHit(ev) => {
                if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
                    return;
                }
                let a = ev.info.actor as usize;
                if !s.mod_active(a, C4) || s.st::<State>(ci).c4_tags[a] == 0 {
                    return;
                }
                let amt = 0.65 * total_def(&s.stats(ci));
                s.st_mut::<State>(ci).c4_tags[a] -= 1;
                s.hit_flat += amt;
            }
            Event::Attack | Event::Dash | Event::Plunge => {
                if s.player.active as usize == ci && s.ns_has_blessing(ci) {
                    apply_c6(s, ci);
                }
            }
            _ => {}
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<crate::gcsl::Num, String>> {
        (fields[0] == "nightsoul").then(|| s.ns_condition(ci, fields))
    }
}

/// C4's stacks left on a character, for `.<char>.tags.xilonen-c4`.
pub fn c4_tag(s: &Sim, on: usize) -> i64 {
    match s.chars.iter().position(|c| c.kind == CharKind::Xilonen) {
        Some(xi) => s.st::<State>(xi).c4_tags[on] as i64,
        None => 0,
    }
}

fn can_use_nightsoul(s: &Sim, ci: usize) -> bool {
    s.ns_points(ci) >= 0.001 || s.mod_active(ci, C6)
}

fn duration_mul(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].cons < 1 { 1.0 } else { 1.0 + 0.45 }
}

fn consumption_mul(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].cons < 1 { 1.0 } else { 1.0 - 0.3 }
}

/// reduceNightsoulPoints: none used under C6; out of points, the blessing
/// ends, unless in a normal or plunging attack.
fn reduce_points(s: &mut Sim, ci: usize, v: f64) {
    if s.mod_active(ci, C6) {
        return;
    }
    let amt = v * consumption_mul(s, ci);
    s.ns_consume(ci, amt);
    if matches!(s.current_state(), AnimState::Normal | AnimState::Plunge) {
        return;
    }
    if s.ns_points(ci) < 0.001 {
        exit_nightsoul(s, ci);
    }
}

fn enter_nightsoul(s: &mut Sim, ci: usize) {
    let f = s.f;
    s.st_mut::<State>(ci).nightsoul_src = f;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_REDUCE, arg: f }, 6);
    s.chars[ci].normal_hit_num = g::ROLLER_HIT_NUM;
    s.chars[ci].normal_counter = 0;
    let dur = (9.0 * 60.0 * duration_mul(s, ci)) as i32;
    s.ns_enter_timed(ci, 45.0, dur, TASK_EXIT);
    s.st_mut::<State>(ci).skill_last_stam_f = s.player.last_stam_use;
    s.player.last_stam_use = i32::MAX;
    if s.chars[ci].cons < 2 && s.st::<State>(ci).samplers_converted < 3 {
        geo_sampler(s, ci, f);
    }
}

fn exit_nightsoul(s: &mut Sim, ci: usize) {
    if !s.ns_has_blessing(ci) {
        return;
    }
    s.ns_exit(ci);
    s.ns_clear(ci);
    s.st_mut::<State>(ci).nightsoul_src = -1;
    s.set_cd(ci, Action::Skill, 7 * 60);
    s.chars[ci].normal_hit_num = g::NORMAL_HIT_NUM;
    s.chars[ci].normal_counter = 0;
    s.player.last_stam_use = s.st::<State>(ci).skill_last_stam_f;
    s.delete_mod(ci, C6);
}

/// The Blade Roller: her normal attacks in Nightsoul's Blessing.
fn roller(s: &mut Sim, ci: usize) -> ActInfo {
    let n = s.chars[ci].normal_counter as usize;
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_XILONEN_SKATE, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::ATTACK_E[n][lvl]);
    ai.abil = ROLLER_NAMES[n];
    ai.add_tags = add_tag::NIGHTSOUL;
    ai.blunt = true;
    ai.poise = g::ROLLER_POISE_DMG[n];
    ai.hitlag_halt = g::ROLLER_HITLAG_HALT_FRAMES[n] * 60.0;
    ai.hitlag_factor = 0.01;
    // only the first can be defense halted
    ai.def_halt = n == 0;
    ai.use_def = true;
    ai.ignore_infusion = true;
    let hb = g::ROLLER_HITBOXES[n];
    let oy = g::ROLLER_OFFSETS[n];
    ai.pattern = if n == 0 || n == 3 { s.fan_on_player(0.0, oy, hb[0], hb[1]) } else { s.box_on_player(0.0, oy, hb[0], hb[1]) };
    let h = g::ROLLER_HITMARKS[n];
    s.queue_attack(ai, CB_A1, h, h);
    let atkspd = s.stat(ci, st::ATKSPD);
    let table = &g::ROLLER_FRAMES[n];
    let mut info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
    info.on_removed = OnRemoved::Char(REMOVED_ROLLER);
    s.advance_normal(ci);
    info
}

/// The enemy within 10 of the player (gcsim's EnemiesWithinArea of a
/// circle on the player).
fn enemy_in_range(s: &Sim) -> bool {
    s.enemy_alive && s.enemy_within(s.circle_on_player(0.0, 0.0, 10.0))
}

fn apply_shred(s: &mut Sim, ci: usize, ele: Element) {
    if !enemy_in_range(s) {
        return;
    }
    let lvl = s.talent(ci, 1);
    s.enemy_add_resist(shred_key(ele), ele, -g::SKILL_SHRED[lvl], 60);
}

/// activeGeoSampler: geo's shred every 0.3 s, for the blessing below C2,
/// for good from C2.
fn geo_sampler(s: &mut Sim, ci: usize, src: i32) {
    let st = s.st::<State>(ci);
    if st.samplers_converted >= 3 {
        return;
    }
    if s.chars[ci].cons < 2 && (st.nightsoul_src != src || !s.ns_has_blessing(ci) || s.mod_active(ci, ACTIVE_SAMPLER)) {
        return;
    }
    apply_shred(s, ci, Element::Geo);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_GEO_SAMPLER, arg: src }, g::SAMPLER_INTERVAL as i32);
}

/// activeSamplers: every Sampler's shred every 0.3 s while A1 has them
/// active, on the active character's queue.
fn samplers(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).sample_src != src || !s.mod_active(ci, ACTIVE_SAMPLER) {
        return;
    }
    let shred = s.st::<State>(ci).shred_elements;
    for ele in [Element::Pyro, Element::Hydro, Element::Electro, Element::Cryo, Element::Geo] {
        if shred & 1 << ele as u16 == 0 || (ele == Element::Geo && s.chars[ci].cons >= 2) {
            continue;
        }
        apply_shred(s, ci, ele);
    }
    let active = s.player.active as usize;
    s.queue_char_task(active, Task::Char { c: ci as u8, id: TASK_SAMPLERS, arg: src }, g::SAMPLER_INTERVAL as i32);
}

/// A1 with two converted Samplers or more: a Nightsoul hit gains 35
/// points, once every 0.1 s; at 90, A4's Nightsoul Burst, then the
/// Samplers' shred for 15 s and C2.
fn a1_cb(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    if s.chars[ci].ascension < 1 || s.st::<State>(ci).samplers_converted < 2 || s.ns_points(ci) < 0.001 || s.mod_active(ci, A1_ICD) {
        return;
    }
    s.add_status(ci, A1_ICD, 6, true);
    s.ns_generate(ci, 35.0);
    if s.ns_points(ci) >= s.chars[ci].ns.max_points {
        // a4MaxPoints
        if s.chars[ci].ascension >= 4 && !s.mod_active(ci, A4_ICD) {
            s.add_status(ci, A4_ICD, 14 * 60, true);
            s.emit(Event::NightsoulBurst(ev));
        }
        // a1MaxPoints
        let pts = s.ns_points(ci);
        s.ns_consume(ci, pts);
        s.add_status(ci, ACTIVE_SAMPLER, 15 * 60, false);
        if s.chars[ci].cons >= 2 {
            s.add_status(ci, C2, 15 * 60, true);
        }
        let f = s.f;
        s.st_mut::<State>(ci).sample_src = f;
        samplers(s, ci, f);
        c2_activate(s, ci);
    }
}

/// C2 as the Samplers activate: pyro, hydro and cryo members' buffs every
/// 0.3 s while it lasts; electro members 25 energy and their burst's
/// cooldown -6 s.
fn c2_activate(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 2 {
        return;
    }
    let f = s.f;
    s.st_mut::<State>(ci).c2_src = f;
    for i in 0..s.chars.len() {
        match s.chars[i].element {
            Element::Pyro | Element::Hydro | Element::Cryo => {
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C2_BUFF, arg: f << 2 | i as i32 }, g::C2_INTERVAL as i32);
            }
            Element::Electro => {
                s.add_energy(i, 25.0);
                s.reduce_cd(i, Action::Burst, 6 * 60);
            }
            _ => {}
        }
    }
}

/// C6 on her attack, dash or plunge in the blessing: no points used and
/// the blessing's end put off for 5 s, flat damage, and three heals.
fn apply_c6(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, C6_ICD) {
        return;
    }
    s.add_status(ci, C6_ICD, 15 * 60, true);
    s.add_mod(ci, Mod::owned(C6, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), g::C6_DURATION);
    let dur = s.ns_duration(ci) + g::C6_DURATION;
    s.ns_set_exit_timer(ci, dur, TASK_EXIT);
    for i in 1..=3 {
        s.add_task(Task::Char { c: ci as u8, id: TASK_C6_HEAL, arg: 0 }, i * 90);
    }
}

/// The airborne plunges, scaling with DEF. While she can use Nightsoul the
/// high plunge and its collision are geo Nightsoul damage with A1's
/// callback, on the skill's frames and collision hitmark (the plunge's
/// hitmark stays the plain one, as gcsim has it). After the hitmark, out
/// of Nightsoul, the blessing ends.
fn plunge_xy(s: &mut Sim, ci: usize, high: bool, p: &Params) -> ActInfo {
    let ns = high && can_use_nightsoul(s, ci);
    let geo = |ai: &mut AttackInfo| {
        ai.element = Element::Geo;
        ai.ignore_infusion = true;
        ai.add_tags = add_tag::NIGHTSOUL;
    };
    let cbs = if ns { CB_A1 } else { 0 };
    let mut ai = plunge::hit_info(s, ci, high, &PLUNGE);
    ai.use_def = true;
    if ns {
        geo(&mut ai);
    }
    if !high && plunge::wants_collision(p) {
        let mut c = plunge::collision_info(s, ci, &PLUNGE);
        c.use_def = true;
        let h = PLUNGE.collision_hitmark;
        s.queue_attack(c, 0, h, h);
    }
    if high && plunge::wants_collision(p) {
        let mut c = plunge::collision_info(s, ci, &PLUNGE);
        c.use_def = true;
        if ns {
            geo(&mut c);
        }
        let h = if ns { g::SKILL_COLLISION_HITMARK } else { PLUNGE.collision_hitmark };
        s.queue_attack(c, cbs, h, h);
    }
    let h = plunge::hitmark(&PLUNGE, high);
    s.queue_attack(ai, cbs, h, h);
    s.add_task(Task::Char { c: ci as u8, id: TASK_PLUNGE_EXIT, arg: 0 }, h);
    if ns {
        let t = &g::SKILL_HIGH_PLUNGE_FRAMES;
        return ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Plunge);
    }
    plunge::act(&PLUNGE, high)
}
