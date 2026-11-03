//! Durin, from gcsim's internal/characters/durin: normal and charged
//! attacks, Essential Transformation (the skill's window, recast as
//! Confirmation of Purity with the skill or Denial of Darkness with an
//! attack), the burst in its white form (As the Light Shifts and the
//! Dragon of White Flame) or, after Denial of Darkness, its black form (As
//! the Stars Smolder and the Dragon of Dark Decay), A1, A4, his Hexerei
//! bonus (A1 ×1.75 with two Hexerei members) and every constellation.
//!
//! gcsim's quirks, kept: Denial of Darkness's three hits queue closures
//! over one AttackInfo, so all three deal the third hit's multiplier; the
//! charged attack is tagged a normal attack; A4 spends a stack and reads
//! his total ATK as each burst tick fires. C4's chance to keep a C1 stack
//! draws a Float64 from the fight's generator at each consumption, where
//! gcsim draws it, so the draws of everyone after keep gcsim's order.
//!
//! The fight has one enemy: A1's resistance shred and C6's DEF shred land
//! on it, and the burst's ticks hit it.

use crate::content::{Character, Event, EventKind, Owner};
use crate::gcsimdata::chars::durin as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The frame of the last burst (gcsim's burstSrc).
    burst_src: i32,
    a4_stacks: i32,
    /// gcsim's "durin-c1" tag on each member.
    c1_tags: [i32; 4],
}

const SKILL_WINDOW: ModKey = ModKey::new(g::SKILL_WINDOW_KEY);
const WHITE: ModKey = ModKey::new(g::WHITE_KEY);
const BLACK: ModKey = ModKey::new(g::BLACK_KEY);
const ENERGY_ICD: ModKey = ModKey::new(g::ENERGY_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST_WHITE: ModKey = ModKey::new(g::BURST_KEY_WHITE);
const BURST_BLACK: ModKey = ModKey::new(g::BURST_KEY_BLACK);
const A1_BLACK: ModKey = ModKey::new(g::A1_BLACK_KEY);
const C4: ModKey = ModKey::new("durin-c4");
const C6: ModKey = ModKey::new("durin-c6");

const CB_PARTICLE: u16 = 1 << 0;
const CB_C6: u16 = 1 << 1;

/// Confirmation of Purity's hit, at its hitmark.
const TASK_WHITE_HIT: u8 = 0;
/// A hit of Denial of Darkness, at its hitmark.
const TASK_BLACK_HIT: u8 = 1;
/// A white tick, with the burst's src (the fight's queue).
const TASK_TICK_WHITE: u8 = 2;
/// A black tick: src * 64 + travel (the fight's queue).
const TASK_TICK_BLACK: u8 = 3;
/// The burst's first hitmark: A1, A4, C1 and the burst's status; arg 1
/// for white.
const TASK_BURST_STATE: u8 = 4;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];
/// gcsim's attackOffsets (not in the generated tables, which carry no
/// points).
const ATTACK_OFFSETS: [&[(f64, f64)]; 4] = [&[(0.0, 0.3)], &[(0.0, 0.6)], &[(-0.2, 0.4), (-0.2, 0.4)], &[(-0.2, -0.6)]];

/// gcsim's a1ReactToElements: the reactions and the elements whose
/// resistance A1 shreds; c2ReactToElements adds vaporize and melt.
fn react_elements(r: Reaction) -> &'static [Element] {
    match r {
        Reaction::Overload => &[Element::Electro, Element::Pyro],
        Reaction::SwirlPyro => &[Element::Anemo, Element::Pyro],
        Reaction::CrystallizePyro => &[Element::Geo, Element::Pyro],
        Reaction::Burning => &[Element::Dendro, Element::Pyro],
        Reaction::Vaporize => &[Element::Hydro, Element::Pyro],
        Reaction::Melt => &[Element::Cryo, Element::Pyro],
        Reaction::ElectroCharged
        | Reaction::LunarCharged
        | Reaction::Superconduct
        | Reaction::StellarConduct
        | Reaction::Frozen
        | Reaction::Shatter
        | Reaction::CrystallizeElectro
        | Reaction::CrystallizeHydro
        | Reaction::CrystallizeCryo
        | Reaction::SwirlHydro
        | Reaction::SwirlElectro
        | Reaction::SwirlCryo
        | Reaction::StellarSwirl
        | Reaction::Quicken
        | Reaction::Aggravate
        | Reaction::Spread
        | Reaction::Bloom
        | Reaction::Hyperbloom
        | Reaction::Burgeon
        | Reaction::LunarBloom
        | Reaction::LunarCrystallize => &[],
    }
}

/// gcsim's "durin-a1-<element>" resist mods.
fn a1_key(e: Element) -> ModKey {
    match e {
        Element::Electro => ModKey::new("durin-a1-electro"),
        Element::Pyro => ModKey::new("durin-a1-pyro"),
        Element::Anemo => ModKey::new("durin-a1-anemo"),
        Element::Geo => ModKey::new("durin-a1-geo"),
        Element::Dendro => ModKey::new("durin-a1-dendro"),
        _ => unreachable!(),
    }
}

/// gcsim's "durin-c2-<element>" stat mods.
fn c2_key(e: Element) -> ModKey {
    match e {
        Element::Electro => ModKey::new("durin-c2-electro"),
        Element::Pyro => ModKey::new("durin-c2-pyro"),
        Element::Anemo => ModKey::new("durin-c2-anemo"),
        Element::Geo => ModKey::new("durin-c2-geo"),
        Element::Dendro => ModKey::new("durin-c2-dendro"),
        Element::Hydro => ModKey::new("durin-c2-hydro"),
        Element::Cryo => ModKey::new("durin-c2-cryo"),
        _ => unreachable!(),
    }
}

impl Character for Char {
    const KEY: &'static str = "durin";
    const HEXEREI: bool = true;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { burst_src: 0, a4_stacks: 0, c1_tags: [0; 4] }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // a1Init: the reactions and OnEnemyDamage; c1Init: two OnEnemyHit
        // hooks; c2Init: the reactions and OnEnemyDamage (all handled in
        // order in one subscription of each kind)
        if asc >= 1 || cons >= 2 {
            s.subscribe(EventKind::Reaction, o);
            s.subscribe(EventKind::EnemyDamage, o);
        }
        if cons >= 1 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        // c4Init: burst DMG +40%
        if cons >= 4 {
            s.chars[ci].mods.push(Mod::owned(C4, ModKind::Attack, o, st::NONE, false, true, -1));
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "a4-stacks" => Some(Ok(Num::Int(s.st::<State>(ci).a4_stacks as i64))),
            _ => None,
        }
    }

    /// The skill's window makes the skill ready (the recast).
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && s.mod_active(ci, SKILL_WINDOW)).then_some(None)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, SKILL_WINDOW) {
            return skill_recast_black(s, ci);
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            let (ox, oy) = ATTACK_OFFSETS[n][i];
            let hb = g::ATTACK_HITBOXES[n][i];
            ai.pattern = if n == 3 { s.box_on_player(ox, oy, hb[0], hb[1]) } else { s.circle_on_player(ox, oy, hb[0]) };
            s.queue_attack_at(ci, ai, 0, g::ATTACK_HITMARKS[n][i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let hm = g::ATTACK_HITMARKS[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hm[hm.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    /// gcsim tags it a normal attack.
    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[s.talent(ci, 0)]);
        ai.abil = "Charge";
        ai.pattern = s.circle_on_player(0.0, 0.8, 2.5);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    /// Essential Transformation: the window for a recast, or, in it,
    /// Confirmation of Purity.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, SKILL_WINDOW) {
            return skill_recast_white(s, ci);
        }
        s.add_status(ci, SKILL_WINDOW, g::SKILL_WINDOW_DUR, true);
        s.set_cd(ci, Action::Skill, 12 * 60);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Burst as usize], AnimState::Skill)
    }

    /// The black burst after Denial of Darkness, the white one otherwise.
    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let white = !s.mod_active(ci, BLACK);
        let travel = if white { 0 } else { p.get("travel").map_or(9, |t| t as i32) };
        let lvl = s.talent(ci, 2);
        let (abil, initial) = if white { ("As the Light Shifts", g::BURST_WHITE_INITIAL) } else { ("As the Stars Smolder", g::BURST_BLACK_INITIAL) };
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_DURIN_BURST_TORNADO, icd::GROUP_DEFAULT, Element::Pyro, 25.0, 0.0);
        ai.abil = abil;
        ai.ignore_def = c6_def_ignore(s, ci, white);
        ai.pattern = s.circle_on_target(6.5);
        let c6 = if white && s.chars[ci].cons >= 6 { CB_C6 } else { 0 };
        for (i, mult) in initial.iter().enumerate() {
            ai.mult = mult[lvl];
            let hm = g::BURST_INIT_HITMARK[i];
            s.queue_attack(ai, c6, hm, hm);
        }
        let f = s.f;
        s.st_mut::<State>(ci).burst_src = f;
        if white {
            s.delete_mod(ci, BURST_BLACK);
            for i in 0..g::BURST_TICKS_WHITE {
                let delay = (g::BURST_FIRST_TICK_DELAY_WHITE + g::BURST_INTERVAL_WHITE * i as f64).ceil() as i32;
                s.add_task(Task::Char { c: ci as u8, id: TASK_TICK_WHITE, arg: f }, delay);
            }
        } else {
            s.delete_mod(ci, BURST_WHITE);
            for i in 0..g::BURST_TICKS_BLACK {
                let delay = (g::BURST_FIRST_TICK_DELAY_BLACK + g::BURST_INTERVAL_BLACK * i as f64).ceil() as i32;
                s.add_task(Task::Char { c: ci as u8, id: TASK_TICK_BLACK, arg: f << 6 | travel.clamp(0, 63) }, delay);
            }
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_STATE, arg: white as i32 }, g::BURST_INIT_HITMARK[0]);
        s.set_cd(ci, Action::Burst, g::BURST_CD);
        s.consume_energy(ci, 10);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        let lvl = s.talent(ci, 1);
        match id {
            TASK_WHITE_HIT => {
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_WHITE[lvl]);
                ai.abil = "Confirmation of Purity";
                ai.pattern = s.circle_on_player(0.0, 1.2, 4.5);
                s.queue_attack(ai, CB_PARTICLE, 0, 0);
            }
            TASK_BLACK_HIT => {
                // every closure reads the shared AttackInfo after the loop
                // set its multiplier to the last hit's
                let last = g::SKILL_BLACK.len() - 1;
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DURIN_SKILL, Element::Pyro, 25.0, g::SKILL_BLACK[last][lvl]);
                ai.abil = "Denial of Darkness";
                ai.pattern = s.box_on_player(0.0, -3.0, 3.0, 7.0);
                s.queue_attack(ai, CB_PARTICLE, 0, 0);
            }
            TASK_TICK_WHITE => {
                if arg != s.st::<State>(ci).burst_src {
                    return;
                }
                let mult = g::BURST_WHITE_DO_T[s.talent(ci, 2)] * a4_dmg(s, ci);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_DURIN_BURST_WHITE, icd::GROUP_DURIN_BURST_WHITE, Element::Pyro, 25.0, mult);
                ai.abil = "Dragon of White Flame";
                ai.ignore_def = c6_def_ignore(s, ci, true);
                ai.pattern = s.circle_on_target(3.5);
                let c6 = if s.chars[ci].cons >= 6 { CB_C6 } else { 0 };
                s.queue_attack(ai, c6, 0, 0);
            }
            TASK_TICK_BLACK => {
                let (src, travel) = (arg >> 6, arg & 63);
                if src != s.st::<State>(ci).burst_src {
                    return;
                }
                let mult = g::BURST_BLACK_DO_T[s.talent(ci, 2)] * a4_dmg(s, ci);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_DURIN_BURST_BLACK, icd::GROUP_DURIN_BURST_BLACK, Element::Pyro, 25.0, mult);
                ai.abil = "Dragon of Dark Decay";
                ai.ignore_def = c6_def_ignore(s, ci, false);
                ai.pattern = s.circle_on_target(1.5);
                s.queue_attack(ai, 0, 0, travel);
            }
            TASK_BURST_STATE => {
                let white = arg == 1;
                a1_on_burst(s, ci, white);
                // a4OnBurst
                if s.chars[ci].ascension >= 4 {
                    s.st_mut::<State>(ci).a4_stacks = 10;
                }
                c1_on_burst(s, ci, white);
                let key = if white { BURST_WHITE } else { BURST_BLACK };
                s.add_status(ci, key, g::BURST_DURATION as i32, false);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        // particleCB
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, true);
            s.queue_particle(4.0, Element::Pyro, s.particle_delay[ci]);
        }
        // c6WhiteMakeCB
        if cbs & CB_C6 != 0 {
            s.enemy_add_def(C6, -0.3, 6 * 60);
        }
    }

    /// A1's black form: amplifying reactions DMG +40% (×1.75 Hexerei).
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if info.amped {
            0.40 * hexerei_a1_bonus(s, ci)
        } else {
            0.0
        }
    }

    /// C4: burst DMG +40%.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::ElementalBurst {
            ev.stats[st::DMGP] += 0.4;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        match *e {
            Event::Reaction(r, ev) => {
                if !ev.on_enemy() {
                    return;
                }
                let elems = react_elements(r);
                if elems.is_empty() {
                    return;
                }
                // a1MakeResShred (not vaporize or melt)
                if asc >= 1 && !matches!(r, Reaction::Vaporize | Reaction::Melt) && s.mod_active(ci, BURST_WHITE) {
                    let v = -0.20 * hexerei_a1_bonus(s, ci);
                    for &el in elems {
                        s.enemy_add_resist(a1_key(el), el, v, 6 * 60);
                    }
                }
                // c2Init's makeBuff
                if cons >= 2 && (s.mod_active(ci, BURST_WHITE) || s.mod_active(ci, BURST_BLACK)) {
                    c2_make_buff(s, elems);
                }
            }
            Event::EnemyDamage(ev) => {
                if !s.enemy_burning() || !matches!(ev.info.element, Element::Dendro | Element::Pyro) {
                    return;
                }
                // a1's durin-a1-hook-on-dmg
                if asc >= 1 && s.mod_active(ci, BURST_WHITE) {
                    let v = -0.20 * hexerei_a1_bonus(s, ci);
                    s.enemy_add_resist(a1_key(Element::Dendro), Element::Dendro, v, 6 * 60);
                    s.enemy_add_resist(a1_key(Element::Pyro), Element::Pyro, v, 6 * 60);
                }
                // c2's durin-c2-hook-on-dmg
                if cons >= 2 && (s.mod_active(ci, BURST_WHITE) || s.mod_active(ci, BURST_BLACK)) {
                    c2_make_buff(s, &[Element::Pyro, Element::Dendro]);
                }
            }
            Event::EnemyHit(ev) => c1_hit(s, ci, ev),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- skill recasts

/// Confirmation of Purity: a pyro hit, the white form for 30 s.
fn skill_recast_white(s: &mut Sim, ci: usize) -> ActInfo {
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_WHITE_HIT, arg: 0 }, g::SKILL_WHITE_HITMARKS);
    s.delete_mod(ci, SKILL_WINDOW);
    s.delete_mod(ci, BLACK);
    s.add_status(ci, WHITE, 30 * 60, true);
    skill_energy(s, ci);
    let t = &g::SKILL_RECAST_WHITE_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
}

/// Denial of Darkness: three pyro hits, the black form for 30 s.
fn skill_recast_black(s: &mut Sim, ci: usize) -> ActInfo {
    for &hm in g::SKILL_BLACK_HITMARKS {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BLACK_HIT, arg: 0 }, hm);
    }
    s.delete_mod(ci, SKILL_WINDOW);
    s.delete_mod(ci, WHITE);
    s.add_status(ci, BLACK, 30 * 60, true);
    skill_energy(s, ci);
    let t = &g::SKILL_RECAST_BLACK_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
}

/// The recast's energy, once every 6 s.
fn skill_energy(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, ENERGY_ICD) {
        return;
    }
    let lvl = s.talent(ci, 1);
    s.add_energy(ci, g::SKILL_ENERGY[lvl]);
    s.add_status(ci, ENERGY_ICD, 6 * 60, true);
}

// ---------------------------------------------------------------- passives

fn hexerei_a1_bonus(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].hexerei && s.hexerei_count() >= 2 {
        1.75
    } else {
        1.0
    }
}

/// a1OnBurst: the white burst drops the black form's amplifying bonus, the
/// black one adds it for the burst's duration.
fn a1_on_burst(s: &mut Sim, ci: usize, white: bool) {
    if s.chars[ci].ascension < 1 {
        return;
    }
    if white {
        s.delete_mod(ci, A1_BLACK);
        return;
    }
    let m = Mod::owned(A1_BLACK, ModKind::ReactBonus, Owner::Char(ci as u8), st::NONE, false, true, -1);
    s.add_mod(ci, m, g::BURST_DURATION as i32);
}

/// gcsim's a4Dmg: while a burst lasts and stacks remain, a tick spends
/// one for +3% per 100 ATK, up to 75%.
fn a4_dmg(s: &mut Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 4 || s.st::<State>(ci).a4_stacks <= 0 {
        return 1.0;
    }
    if !s.mod_active(ci, BURST_WHITE) && !s.mod_active(ci, BURST_BLACK) {
        return 1.0;
    }
    s.st_mut::<State>(ci).a4_stacks -= 1;
    1.0 + (s.total_atk(ci) / 100.0 * 0.03).min(0.75)
}

// ---------------------------------------------------------------- constellations

/// c1OnBurst: the white burst gives the others 20 stacks, the black one
/// gives them to Durin.
fn c1_on_burst(s: &mut Sim, ci: usize, white: bool) {
    if s.chars[ci].cons < 1 {
        return;
    }
    let n = s.chars.len();
    let st = s.st_mut::<State>(ci);
    for i in 0..n {
        let own = i == ci;
        st.c1_tags[i] = if own != white { 20 } else { 0 };
    }
}

/// gcsim's two OnEnemyHit hooks: the white form's +60% of Durin's ATK on
/// the active member's hits, the black form's +150% on his burst hits.
fn c1_hit(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    let actor = ev.info.actor as usize;
    // durin-c1-white-hook
    if matches!(ev.info.tag, AttackTag::ElementalBurst | AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge)
        && actor != ci
        && s.st::<State>(ci).c1_tags[actor] > 0
        && s.mod_active(ci, BURST_WHITE)
        && actor == s.player.active as usize
    {
        let consume = c4_c1_consume_mult(s, ci);
        s.st_mut::<State>(ci).c1_tags[actor] -= consume;
        s.hit_flat += 0.6 * s.total_atk(ci);
    }
    // durin-c1-black-hook
    if ev.info.tag == AttackTag::ElementalBurst && actor == ci && s.st::<State>(ci).c1_tags[ci] != 0 && s.mod_active(ci, BURST_BLACK) {
        let consume = 2 * c4_c1_consume_mult(s, ci);
        s.st_mut::<State>(ci).c1_tags[ci] -= consume;
        s.hit_flat += 1.5 * s.total_atk(ci);
    }
}

/// C4: a C1 consumption keeps its stacks 30% of the time (a Float64 from
/// the fight's generator, drawn where gcsim draws it).
fn c4_c1_consume_mult(s: &mut Sim, ci: usize) -> i32 {
    if s.chars[ci].cons < 4 {
        return 1;
    }
    if s.rng.float64() < 0.3 {
        0
    } else {
        1
    }
}

/// C2: +50% DMG of each element for every member, 6 s.
fn c2_make_buff(s: &mut Sim, elems: &[Element]) {
    for &el in elems {
        for i in 0..s.chars.len() {
            s.add_stat_mod(i, c2_key(el), el.dmg_stat(), 0.5, 6 * 60, true);
        }
    }
}

/// C6: the burst ignores 30% (white) or 70% (black) of DEF.
fn c6_def_ignore(s: &Sim, ci: usize, white: bool) -> f64 {
    if s.chars[ci].cons < 6 {
        0.0
    } else if white {
        0.3
    } else {
        0.7
    }
}
