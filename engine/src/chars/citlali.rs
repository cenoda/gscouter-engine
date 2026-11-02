//! Citlali, from gcsim's internal/characters/citlali: normal and charged
//! attacks, Dawnfrost Darkstar (the skill's hit, its shield, the cryo on
//! the player and her Nightsoul's Blessing, with Itzpapa's Opal Fire and
//! its Frostfall Storm), Edict of Entwined Splendor (Ice Storm and the
//! Spiritvessel Skulls), A1 (on melt and frozen: Nightsoul points and the
//! enemy's hydro and pyro RES), A4 (Nightsoul points on a Nightsoul Burst
//! and her EM as flat damage) and every constellation.
//!
//! Her Nightsoul's Blessing is the general one (`crate::nightsoul`): up to
//! 100 points, for 20 s from the skill; at 50 points (or any, at C6)
//! Itzpapa enters Opal Fire, which drains 0.8 points every 6 frames and
//! strikes every 59 frames. The blessing's end keeps her points, as gcsim
//! has it. C1's leap and aimed parts are not in gcsim (it has no plunges
//! for her), so they are not here either.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::citlali as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// gcsim's opalFireSrc: the frame Opal Fire started, -1 out of it.
    opal_fire_src: i32,
    /// C1's Stellar Blades left.
    stellar_blades: i32,
    /// C6's Cifra of the Secret Law points.
    c6_stacks: f64,
    /// The charged attack's hit, queued at its hitmark (its pattern is
    /// taken as the attack starts).
    charge: AttackInfo,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const OPAL_FIRE: ModKey = ModKey::new(g::OPAL_FIRE_STATE_KEY);
const A1_ICD: ModKey = ModKey::new(g::NIGHT_SOUL_GENERATION_ICD);
const A1_HYDRO: ModKey = ModKey::new("citlali-a1-hydro");
const A1_PYRO: ModKey = ModKey::new("citlali-a1-pyro");
const C2: ModKey = ModKey::new("citlali-c2-em");
const C4_ICD: ModKey = ModKey::new(g::C4_SKULL_ICD);
const C6: ModKey = ModKey::new("citlali-c6");
const SHIELD: ModKey = ModKey::new("citlali-shield");

const MAX_C6_STACKS: f64 = g::MAX_C6_STACKS as f64;

const CB_PARTICLE: u16 = 1 << 0;
const CB_C4: u16 = 1 << 1;

/// exitNightsoul, as the blessing's timer callback.
const TASK_EXIT: u8 = 0;
/// The skill's cooldown, blessing and cryo on the player, 18 frames in.
const TASK_SKILL_BLESSING: u8 = 1;
/// addShield, 37 frames in.
const TASK_SHIELD: u8 = 2;
/// nightsoulPointReduceTask, with its src.
const TASK_REDUCE: u8 = 3;
/// itzpapaHitTask, with its src.
const TASK_ITZPAPA: u8 = 4;
/// The burst's 24 points, 115 frames in.
const TASK_BURST_POINTS: u8 = 5;
/// A Spiritvessel Skull: 3 points and its hit.
const TASK_SKULL: u8 = 6;
/// The charged attack's hit, with its travel.
const TASK_CHARGE: u8 = 7;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];

impl Character for Char {
    const KEY: &'static str = "citlali";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            opal_fire_src: -1,
            stellar_blades: 0,
            c6_stacks: 0.0,
            charge: AttackInfo::new(0, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 0.0),
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].ns.max_points = 100.0;
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // A1: OnMelt and OnFrozen
        if asc >= 1 {
            s.subscribe(EventKind::Reaction, o);
        }
        // A4
        if asc >= 4 {
            s.subscribe(EventKind::NightsoulBurst, o);
        }
        // C1
        if cons >= 1 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        // C2: her own EM +125, the active member's +250 while she is
        // shielding or in her blessing
        if cons >= 2 {
            s.chars[ci].mods.push(Mod::stat(C2, st::EM, 125.0, false, false, -1));
            for i in 0..s.chars.len() {
                if i != ci {
                    s.chars[i].mods.push(Mod::owned(C2, ModKind::Stat, o, st::EM, false, false, -1));
                }
            }
        }
        // C6: the others' pyro and hydro DMG, her own DMG, by the stacks
        if cons >= 6 {
            for i in 0..s.chars.len() {
                if i != ci {
                    s.chars[i].mods.push(Mod::owned(C6, ModKind::Stat, o, st::NONE, false, true, -1));
                }
            }
            s.chars[ci].mods.push(Mod::owned(C6, ModKind::Attack, o, st::NONE, false, true, -1));
        }
    }

    /// gcsim allows only `travel` on her normal and charged attacks; it
    /// reads no params of the skill or burst. She has no plunges.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge => p.0.iter().find(|(k, _)| k != "travel").map(|(k, _)| format!("{name} param {k}")),
            Action::Skill | Action::Burst => None,
            _ => Some(name.into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = g::ATTACK_HITLAG_FACTOR;
        ai.deployable = true;
        // NewCircleHit(Player, PrimaryTarget, nil, 0.8)
        ai.pattern = s.circle_on_target(g::ATTACK_RADIUS);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.hitlag_factor = 0.05;
        ai.deployable = true;
        // NewCircleHit(Player, PrimaryTarget, nil, 0.8), now
        ai.pattern = s.circle_on_target(g::CHARGE_RADIUS);
        s.st_mut::<State>(ci).charge = ai;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CHARGE, arg: travel }, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Charge)
    }

    /// Dawnfrost Darkstar.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL[lvl]);
        ai.abil = "Obsidian Tzitzimitl DMG";
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.hitlag_factor = 0.01;
        // NewCircleHitOnTarget(Player, nil, 6)
        ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
        let h = g::OBSIDIAN_TZITZIMITL_HITMARK;
        s.queue_attack(ai, CB_PARTICLE, h, h);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_BLESSING, arg: 0 }, 18);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SHIELD, arg: 0 }, 37);
        if s.chars[ci].cons >= 1 {
            s.st_mut::<State>(ci).stellar_blades = 10;
        }
        if s.chars[ci].cons >= 6 {
            let current = s.ns_points(ci);
            s.ns_consume(ci, current);
            s.st_mut::<State>(ci).c6_stacks = MAX_C6_STACKS.min(current);
        }
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Burst as usize], AnimState::Skill)
    }

    /// Edict of Entwined Splendor.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 50.0, g::ICE_STORM[lvl]);
        ai.abil = g::ICE_STORM_ABIL;
        ai.add_tags = add_tag::NIGHTSOUL;
        ai.flat_dmg = a4_dmg(s, ci, 12.0);
        // NewCircleHitOnTarget(PrimaryTarget, nil, 6.5)
        ai.pattern = s.circle_on_target(6.5);

        s.consume_energy(ci, 8);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_POINTS, arg: 0 }, 115);
        let h = g::ICE_STORM_HITMARK;
        s.queue_attack(ai, 0, h, h);
        // a skull for each enemy within 7 of the player (up to three), each
        // after its own random delay
        if s.enemy_alive && s.enemy_within(s.circle_on_player(0.0, 0.0, 7.0)) {
            let d = h + g::SPIRIT_VESSEL_SKULL_HITMARK + 30 + s.rng.intn(31);
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKULL, arg: 0 }, d);
        }
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_EXIT => exit_nightsoul(s, ci),
            TASK_SKILL_BLESSING => {
                s.set_cd(ci, Action::Skill, 16 * 60);
                let pts = s.ns_points(ci) + 24.0;
                s.ns_enter_timed(ci, pts, 20 * 60, TASK_EXIT);
                try_enter_opal_fire(s, ci);
                s.apply_self_infusion(AURA_CRYO, 25.0, 6);
            }
            TASK_SHIELD => {
                let lvl = s.talent(ci, 1);
                let hp = g::SHIELD_EM[lvl] * s.stat(ci, st::EM) + g::SHIELD_FLAT[lvl];
                let f = s.f;
                s.add_shield(Shield { owner: ci as u8, key: SHIELD, typ: ShieldType::CitlaliSkill, target: -1, ele: Element::Cryo, hp, expiry: f + 20 * 60, em_bonus: 0.0 });
            }
            TASK_REDUCE => {
                if s.st::<State>(ci).opal_fire_src != arg || !s.mod_active(ci, OPAL_FIRE) {
                    return;
                }
                // 0.8 points every 6 frames
                let prev = s.ns_points(ci);
                s.ns_consume(ci, 0.8);
                let cons = s.chars[ci].cons;
                if cons >= 6 {
                    let diff = prev - s.ns_points(ci);
                    let st = s.st_mut::<State>(ci);
                    st.c6_stacks = MAX_C6_STACKS.min(st.c6_stacks + diff);
                }
                if s.ns_points(ci) < 0.001 && cons < 6 {
                    s.st_mut::<State>(ci).opal_fire_src = -1;
                    s.delete_mod(ci, OPAL_FIRE);
                    return;
                }
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_REDUCE, arg }, 6);
            }
            TASK_ITZPAPA => {
                if s.st::<State>(ci).opal_fire_src != arg || !s.mod_active(ci, OPAL_FIRE) {
                    return;
                }
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_CITLALI_FROSTFALL_STORM, icd::GROUP_CITLALI_FROSTFALL_STORM, Element::Cryo, 25.0, g::FROSTFALL[lvl]);
                ai.abil = g::FROST_FALL_ABIL;
                ai.add_tags = add_tag::NIGHTSOUL;
                ai.flat_dmg = a4_dmg(s, ci, 0.9);
                ai.hitlag_factor = 0.01;
                // NewCircleHitOnTarget(Player().Pos(), nil, 6), now
                ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
                s.queue_attack(ai, CB_C4, 0, 0);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ITZPAPA, arg }, g::ITZPAPA_INTERVAL);
            }
            TASK_BURST_POINTS => generate_points(s, ci, 24.0),
            TASK_SKULL => {
                generate_points(s, ci, 3.0);
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_CITLALI_SPIRIT_VESSEL, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SPIRIT_VESSEL[lvl]);
                ai.abil = "Spiritvessel Skull DMG";
                ai.add_tags = add_tag::NIGHTSOUL;
                // NewCircleHitOnTarget(enemy.Pos(), nil, 3.5): the enemy
                // does not move
                ai.pattern = s.circle_on_target(3.5);
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_CHARGE => {
                let ai = s.st::<State>(ci).charge;
                s.queue_attack(ai, 0, 0, arg);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, false);
            s.queue_particle(5.0, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_C4 != 0 {
            c4_skull(s, ci);
        }
    }

    /// C2's EM on the active member and C6's pyro and hydro DMG on the
    /// others.
    fn mod_amount(s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C2 {
            // the member Itzpapa follows: the active one
            if s.player.active as usize != on {
                return false;
            }
            if !s.shields.iter().any(|x| x.typ == ShieldType::CitlaliSkill) && !s.ns_has_blessing(ci) {
                return false;
            }
            out[st::EM] += 250.0;
            return true;
        }
        if m.key == C6 {
            let v = 0.015 * s.st::<State>(ci).c6_stacks;
            out[st::PYROP] += v;
            out[st::HYDROP] += v;
            return true;
        }
        false
    }

    /// C6: her DMG +2.5% a stack.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C6 {
            ev.stats[st::DMGP] += 0.025 * s.st::<State>(ci).c6_stacks;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Reaction(Reaction::Melt | Reaction::Frozen, ev) if ev.on_enemy() => a1(s, ci),
            Event::NightsoulBurst(_) => generate_points(s, ci, 4.0),
            // C1: another member's hit while active, with Stellar Blades
            // left, adds 200% of her EM
            Event::EnemyHit(ev) => {
                let a = ev.info.actor as usize;
                if a == ci || s.player.active as usize != a {
                    return;
                }
                if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst | AttackTag::Plunge) {
                    return;
                }
                if s.st::<State>(ci).stellar_blades > 0 {
                    s.hit_flat += s.stat(ci, st::EM) * 2.0;
                    s.st_mut::<State>(ci).stellar_blades -= 1;
                }
            }
            _ => {}
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<crate::gcsl::Num, String>> {
        use crate::gcsl::Num;
        let st = s.st::<State>(ci);
        match fields[0].as_str() {
            "stellar-blade" => Some(Ok(Num::Int(st.stellar_blades as i64))),
            "opal-fire" => Some(Ok(Num::from_bool(s.mod_active(ci, OPAL_FIRE)))),
            "c6-stacks" => Some(Ok(Num::Float(st.c6_stacks))),
            "nightsoul" => Some(s.ns_condition(ci, fields)),
            _ => None,
        }
    }
}

/// a4Dmg: from A4, `mul` times her EM as flat damage.
fn a4_dmg(s: &Sim, ci: usize, mul: f64) -> f64 {
    if s.chars[ci].ascension < 4 {
        return 0.0;
    }
    mul * s.stat(ci, st::EM)
}

/// exitNightsoul (her points stay).
fn exit_nightsoul(s: &mut Sim, ci: usize) {
    let st = s.st_mut::<State>(ci);
    st.c6_stacks = 0.0;
    st.stellar_blades = 0;
    st.opal_fire_src = -1;
    s.delete_mod(ci, OPAL_FIRE);
    s.ns_exit(ci);
}

/// generateNightsoulPoints: the points, then Opal Fire if they reach it.
fn generate_points(s: &mut Sim, ci: usize, amount: f64) {
    s.ns_generate(ci, amount);
    try_enter_opal_fire(s, ci);
}

/// tryEnterOpalFireState: in the blessing, with 50 points or more (any
/// at C6), Opal Fire starts if it is not on: Itzpapa's strikes and the
/// points' drain.
fn try_enter_opal_fire(s: &mut Sim, ci: usize) {
    if !s.ns_has_blessing(ci) {
        return;
    }
    if s.ns_points(ci) < 50.0 && s.chars[ci].cons < 6 {
        return;
    }
    if s.mod_active(ci, OPAL_FIRE) {
        return;
    }
    s.add_status(ci, OPAL_FIRE, -1, false);
    let f = s.f;
    s.st_mut::<State>(ci).opal_fire_src = f;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ITZPAPA, arg: f }, g::ITZPAPA_INTERVAL);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_REDUCE, arg: f }, 6);
}

/// A1 on a melt or frozen on the enemy, in her blessing: 16 points (and
/// C1's three Stellar Blades) once every 8 s, and the enemy's hydro and
/// pyro RES −20% (−40% at C2) for 12 s.
fn a1(s: &mut Sim, ci: usize) {
    if !s.ns_has_blessing(ci) {
        return;
    }
    if !s.mod_active(ci, A1_ICD) {
        s.add_status(ci, A1_ICD, 8 * 60, true);
        generate_points(s, ci, 16.0);
        if s.chars[ci].cons >= 1 {
            s.st_mut::<State>(ci).stellar_blades += 3;
        }
    }
    let amt = if s.chars[ci].cons >= 2 { -0.4 } else { -0.2 };
    s.enemy_add_resist(A1_HYDRO, Element::Hydro, amt, 12 * 60);
    s.enemy_add_resist(A1_PYRO, Element::Pyro, amt, 12 * 60);
}

/// C4 on a Frostfall Storm hit, once every 8 s: 16 points, 8 energy and a
/// skull of 1800% of her EM on the enemy.
fn c4_skull(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 4 || s.mod_active(ci, C4_ICD) {
        return;
    }
    s.add_status(ci, C4_ICD, 8 * 60, true);
    generate_points(s, ci, 16.0);
    s.add_energy(ci, 8.0);
    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 0.0);
    ai.abil = "Spiritvessel Skull DMG (C4)";
    ai.add_tags = add_tag::NIGHTSOUL;
    ai.flat_dmg = 18.0 * s.stat(ci, st::EM);
    // NewCircleHitOnTarget(PrimaryTarget, nil, 3.5)
    ai.pattern = s.circle_on_target(3.5);
    let h = g::SPIRIT_VESSEL_SKULL_HITMARK;
    s.queue_attack(ai, 0, h, h);
}
