//! Odette, from gcsim's internal/characters/odette: normal and charged
//! attacks, the skill (Adagio: Phantom Night Dancers) with its Solo Dance
//! Double and its recast (Coda at Dawn's Tolling), the burst (Presto:
//! Bluebird Finale) and Snow Swan's Dream, A1 (Marvelous Splendor), A4 and
//! C1 to C6, and her Stellar part: she turns superconduct into
//! Stellar-Conduct and cryo swirl into Stellar Swirl (`stellar.rs`), and
//! the Base DMG of every direct Stellar hit and Stellar Swirl contribution
//! gains 0.7% per 100 of her ATK (at most 14%). In Radiance:
//! Stellar-Conduct (inside a Polestar Field) or, failing that, Radiance:
//! Stellar Swirl, the recast's final hit, the upgraded Dance Double and
//! C1 and C4 deal direct Stellar DMG of that kind. No plunges (gcsim has
//! none).
//!
//! The recast's cooldown is gcsim's ActionSpecialSkill, kept in the
//! engine's third cooldown slot (`Action::LowPlunge`, which she has no
//! use for); while the recast is up, the skill's cooldown, charges, reset
//! and reduction read that slot (`Sim::cd_action`), as her overrides do.

use crate::content::{Character, Event, EventKind, Owner};
use crate::gcsimdata::chars::odette as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::stellar::{POLESTAR_FIELD, RADIANCE_SWIRL};
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// gcsim's danceDoubleSrc and danceDoubleNextAtk (0 Plume, 1 Wing).
    dance_src: i32,
    dance_next: i32,
    a1_src: i32,
    a1_self: i32,
    a1_others: i32,
    c2_src: i32,
    /// Snow Swan's Dream's bonus as it was given, on her and (C4) on the
    /// others.
    swan_self: f64,
    swan_others: f64,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const DANCE_DOUBLE: ModKey = ModKey::new(g::DANCE_DOUBLE_KEY);
const DANCE_UPGRADE: ModKey = ModKey::new(g::DANCE_DOUBLE_UPGRADE_KEY);
const SKILL_RECAST: ModKey = ModKey::new(g::SKILL_RECAST_KEY);
const SWANS_DREAM: ModKey = ModKey::new(g::SWANS_DREAM_KEY);
const A1_BUFF: ModKey = ModKey::new("odette-a1-buff");
const C2: ModKey = ModKey::new(g::C2_KEY);
const C2_CRYO: ModKey = ModKey::new("odette-c2-cryo");
const C2_ELECTRO: ModKey = ModKey::new("odette-c2-electro");
const C2_ANEMO: ModKey = ModKey::new("odette-c2-anemo");
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;

// arg: the source frame << 1 | the dance move
const TASK_DANCE_TICK: u8 = 0;
// arg: (the move << 1 | refresh) for the dance double to start
const TASK_START_DANCE: u8 = 1;
const TASK_RECAST_FINAL: u8 = 2;
const TASK_BURST_FINAL: u8 = 3;
// arg: the source frame << 2 | the quarters of a frame carried
const TASK_A1_TICK: u8 = 4;
// arg: the source frame
const TASK_C2_TICK: u8 = 5;

const NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];
const ATTACK_OFFSETS: [(f64, f64); 5] = [(0.0, 1.0), (0.0, 0.5), (0.0, 0.4), (-0.2, -0.6), (0.0, 0.4)];

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Radiance {
    None,
    Conduct,
    Swirl,
}

/// gcsim's getRadiance: Stellar-Conduct takes priority.
fn radiance(s: &Sim, ci: usize) -> Radiance {
    if s.mod_active(ci, POLESTAR_FIELD) {
        Radiance::Conduct
    } else if s.mod_active(ci, RADIANCE_SWIRL) {
        Radiance::Swirl
    } else {
        Radiance::None
    }
}

/// gcsim's useSpecialSkill: the skill is the recast.
pub(crate) fn use_special(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, DANCE_DOUBLE) && s.mod_active(ci, SKILL_RECAST)
}

fn dance(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, DANCE_DOUBLE)
}

fn stellar_ai(ci: usize, abil: &'static str, tag: AttackTag, mult: f64) -> AttackInfo {
    let mut ai = AttackInfo::new(ci, tag, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 0.0, mult);
    ai.abil = abil;
    ai.ignore_def = 1.0;
    ai
}

/// A4: Stellar Glimmer DMG +1.5% of the original per 100 ATK above 1000,
/// at most 30%.
fn a4_mult(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 4 {
        return 1.0;
    }
    let scaling = (s.total_atk(ci) - 1000.0).max(0.0);
    1.0 + (scaling / 100.0 * 0.015).min(0.3)
}

fn a1_max(s: &Sim, ci: usize) -> i32 {
    4 + if s.chars[ci].cons >= 1 { 2 } else { 0 }
}

/// gcsim's startDanceDouble.
fn start_dance(s: &mut Sim, ci: usize, first: i32, delay: i32, refresh: bool) {
    let src = s.f;
    st_mut(s, ci).dance_src = src;
    s.add_status(ci, DANCE_DOUBLE, 1262, false);
    s.add_task(Task::Char { c: ci as u8, id: TASK_DANCE_TICK, arg: src << 1 | first }, delay);
    if !refresh && s.chars[ci].ascension >= 1 {
        // a1OnDanceSummon
        let max = a1_max(s, ci);
        let state = st_mut(s, ci);
        state.a1_self = max;
        state.a1_others = 0;
        state.a1_src = src;
        a1_tick(s, ci, src, 0);
    }
    if s.chars[ci].cons >= 2 && s.chars[ci].ascension >= 1 {
        st_mut(s, ci).c2_src = src;
        c2_tick(s, ci, src);
    }
}

/// gcsim's danceDoubleTicker.
fn dance_tick(s: &mut Sim, ci: usize, src: i32, move_: i32) {
    if st(s, ci).dance_src != src || !dance(s, ci) {
        return;
    }
    let lvl = s.talent(ci, 1);
    let plume = move_ == g::DANCE_DOUBLE_PLUME;
    let (abil, mult) = if plume { ("\"Plume\" Dance Move", g::PLUME[lvl]) } else { ("\"Wing\" Dance Move", g::WING[lvl]) };
    let ap = if plume { s.circle_on_target(4.0) } else { s.circle_on_enemy(0.0, 1.0, 4.0) };
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, mult);
    ai.abil = abil;
    ai.pattern = ap;
    s.queue_attack(ai, 0, 0, 0);
    let next = (move_ + 1) % 2;
    st_mut(s, ci).dance_next = next;
    s.add_task(Task::Char { c: ci as u8, id: TASK_DANCE_TICK, arg: src << 1 | next }, g::SKILL_TICK_DELAY[move_ as usize]);
    if !s.mod_active(ci, DANCE_UPGRADE) {
        return;
    }
    let rad = radiance(s, ci);
    let a4 = a4_mult(s, ci);
    let mut ai = match (rad, plume) {
        (Radiance::None, _) => return,
        (Radiance::Conduct, true) => stellar_ai(ci, "\"Plume\" Dance Move (Stellar-Conduct)", AttackTag::DirectStellarConduct, g::PLUME_SSC[lvl] * a4),
        (Radiance::Conduct, false) => stellar_ai(ci, "\"Wing\" Dance Move (Stellar-Conduct)", AttackTag::DirectStellarConduct, g::WING_SSC[lvl] * a4),
        (Radiance::Swirl, true) => stellar_ai(ci, "\"Plume\" Dance Move (Stellar Swirl)", AttackTag::DirectStellarSwirl, g::PLUME_S_SW[lvl] * a4),
        (Radiance::Swirl, false) => stellar_ai(ci, "\"Wing\" Dance Move (Stellar Swirl)", AttackTag::DirectStellarSwirl, g::WING_S_SW[lvl] * a4),
    };
    ai.pattern = ap;
    s.queue_attack(ai, 0, 0, 0);
}

/// gcsim's a1Ticker, every 59.25 frames on average (`quarters` carries
/// the fraction).
fn a1_tick(s: &mut Sim, ci: usize, src: i32, quarters: i32) {
    let max = a1_max(s, ci);
    let state = st(s, ci);
    if state.a1_src != src || state.a1_self == 0 || state.a1_others == max || !dance(s, ci) {
        return;
    }
    let q = quarters + 1;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1_TICK, arg: src << 2 | q % 4 }, 59 + q / 4);
    if s.player.active as usize == ci {
        return;
    }
    let cons = s.chars[ci].cons;
    let remove = if cons >= 1 { 2 } else { 1 };
    let reduce = if cons >= 6 { 0 } else { 1 };
    let state = st_mut(s, ci);
    let stacks = remove.min(state.a1_self);
    state.a1_self -= stacks * reduce;
    state.a1_others = (state.a1_others + stacks).min(max);
}

/// gcsim's c2Ticker: every second while the Dance Double is on, in
/// Radiance, Cryo and Electro or Anemo RES -20% for 2 s.
fn c2_tick(s: &mut Sim, ci: usize, src: i32) {
    if !dance(s, ci) || st(s, ci).c2_src != src {
        return;
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C2_TICK, arg: src }, 60);
    let (key, ele) = match radiance(s, ci) {
        Radiance::Conduct => (C2_ELECTRO, Element::Electro),
        Radiance::Swirl => (C2_ANEMO, Element::Anemo),
        Radiance::None => return,
    };
    let ap = s.circle_on_player(0.0, 0.0, 10.0);
    if s.enemy_alive && s.enemy_within(ap) {
        s.enemy_add_resist(C2_CRYO, Element::Cryo, -0.2, 120);
        s.enemy_add_resist(key, ele, -0.2, 120);
    }
}

/// The recast: the duet's three hits, its final hit, and the Dance Double
/// again after it.
fn recast(s: &mut Sim, ci: usize) -> ActInfo {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ODETTE_DANCE_DUO, icd::GROUP_ODETTE_DANCE_DUO, Element::Cryo, 25.0, g::CODA_DO_T[lvl]);
    ai.abil = "Coda at Dawn's Tolling";
    ai.pattern = s.circle_on_player(0.0, 0.5, 4.2);
    for &d in g::SKILL_RECAST_DO_T_HITMARKS {
        s.queue_attack(ai, CB_PARTICLE, d, d);
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_RECAST_FINAL, arg: 0 }, g::SKILL_RECAST_FINAL_HITMARK);
    // the tickers stop for the recast; the Dance Double starts again after
    let src = s.f;
    st_mut(s, ci).dance_src = src;
    s.add_task(Task::Char { c: ci as u8, id: TASK_DANCE_TICK, arg: src << 1 | g::DANCE_DOUBLE_PLUME }, g::SKILL_RECAST_FINAL_HITMARK + g::SKILL_RECAST_FIRST_TICK_DELAY);
    s.set_cd(ci, Action::LowPlunge, 15 * 60);
    s.delete_mod(ci, SKILL_RECAST);
    let t = &g::SKILL_RECAST_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
}

fn recast_final(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 1);
    let a4 = a4_mult(s, ci);
    let mut ai = if radiance(s, ci) == Radiance::Swirl {
        stellar_ai(ci, "Coda at Dawn's Tolling (Stellar Swirl)", AttackTag::DirectStellarSwirl, g::CODA_S_SW[lvl] * a4)
    } else {
        stellar_ai(ci, "Coda at Dawn's Tolling (Stellar-Conduct)", AttackTag::DirectStellarConduct, g::CODA_SSC[lvl] * a4)
    };
    ai.hitlag_halt = 0.04 * 60.0;
    ai.hitlag_factor = 0.01;
    ai.pattern = s.circle_on_player(0.0, 0.0, 4.6);
    let tag = ai.tag;
    s.queue_attack(ai, CB_PARTICLE, 0, 0);
    s.add_status(ci, DANCE_UPGRADE, 20 * 60, false);
    // C1: Daybreak Finale, of the final hit's kind
    if s.chars[ci].cons >= 1 {
        let a4 = a4_mult(s, ci);
        let mut ai = if tag == AttackTag::DirectStellarSwirl {
            stellar_ai(ci, "Daybreak Finale (C1) (Stellar Swirl)", tag, 4.5 * a4)
        } else {
            stellar_ai(ci, "Daybreak Finale (C1) (Stellar-Conduct)", tag, 3.0 * a4)
        };
        ai.pattern = s.circle_on_player(0.0, 0.0, 4.5);
        s.queue_attack(ai, CB_PARTICLE, 0, 0);
    }
}

fn burst_ai(ci: usize, s: &Sim) -> AttackInfo {
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST[lvl]);
    ai.abil = "Presto: Bluebird Finale (Slash)";
    ai
}

/// stellarInit's bonus: 0.7% per 100 of her ATK, at most 14%.
fn stellar_bonus(s: &Sim, ci: usize) -> f64 {
    (s.total_atk(ci) / 100.0 * 0.007).min(0.14)
}

/// C6's elevation of a direct Stellar hit by character `on` (her
/// OnApplyAttack subscription), in thousandths.
pub(crate) fn c6_elevation(s: &Sim, ci: usize, on: usize) -> u16 {
    if !dance(s, ci) {
        return 0;
    }
    let state = st(s, ci);
    if on == ci {
        if state.a1_self > 0 { 450 } else { 0 }
    } else if state.a1_others > 0 {
        250
    } else {
        0
    }
}

impl Character for Char {
    const KEY: &'static str = "odette";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // stellarInit
        s.subscribe(EventKind::EnemyHit, o);
        s.subscribe(EventKind::SpecialReactionAttack, o);
        // A1: Stellar Glimmer DMG by Marvelous Splendor
        if asc >= 1 {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(A1_BUFF, ModKind::ReactBonus, o, st::NONE, false, false, -1));
            }
        }
        // C2: ATK% by Marvelous Splendor
        if cons >= 2 && asc >= 1 {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(C2, ModKind::Stat, o, st::ATKP, false, false, -1));
            }
        }
        if cons >= 4 {
            s.subscribe(EventKind::EnemyDamage, o);
        }
        if cons >= 6 {
            s.stellar.odette_c6 = Some(ci as u8);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        if a != Action::Skill || !use_special(s, ci) {
            return None;
        }
        if s.chars[ci].cd[crate::sim::Char::cd_index(Action::LowPlunge)].available > 0 {
            Some(None)
        } else {
            Some(Some(Failure::SkillCd))
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let (ox, oy) = ATTACK_OFFSETS[n];
        let hb = g::ATTACK_HITBOXES[n];
        let ap = if n == 3 { s.box_on_player(ox, oy, hb[0], hb[1]) } else { s.circle_on_player(ox, oy, hb[0]) };
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            ai.pattern = ap;
            s.queue_attack_at(ci, ai, 0, g::ATTACK_HITMARKS[n][i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let hm = g::ATTACK_HITMARKS[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hm[hm.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge";
        // NewBoxHit(Player, PrimaryTarget, {X: -0.2, Y: -0.6}, 3.3, 4.8)
        ai.pattern = s.box_on_target(-0.2, -0.6, 3.3, 4.8);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if use_special(s, ci) {
            return recast(s, ci);
        }
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL[lvl]);
        ai.abil = "Adagio: Phantom Night Dancers";
        ai.pattern = s.circle_on_player(0.0, 0.0, 4.0);
        s.queue_attack(ai, CB_PARTICLE, g::SKILL_HITMARK, g::SKILL_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_START_DANCE, arg: g::SKILL_FIRST_TICK_DELAY << 2 }, 23);
        s.add_status(ci, SKILL_RECAST, 394, true);
        s.set_cd_with_delay(ci, Action::Skill, 15 * 60, 14);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mut ai = burst_ai(ci, s);
        ai.pattern = s.circle_on_player(0.0, 0.0, 7.0);
        for (i, &d) in g::BURST_HITMARKS.iter().enumerate() {
            let mut a = ai;
            // only the first hit is blunt
            if i == 0 {
                a.blunt = true;
                a.poise = 30.0;
            }
            s.queue_attack_at(ci, a, 0, d);
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_FINAL, arg: 0 }, g::FINAL_HITMARK);
        // a Dance Double on the field keeps its next move
        let (next, refresh) = if dance(s, ci) { (st(s, ci).dance_next, 1) } else { (g::DANCE_DOUBLE_PLUME, 0) };
        let arg = g::BURST_FIRST_TICK_DELAY << 2 | next << 1 | refresh;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_START_DANCE, arg }, g::BURST_SUMMON_FRAME);
        s.add_status(ci, SKILL_RECAST, 6 * 60 + g::BURST_SUMMON_FRAME, false);
        s.consume_energy(ci, 7);
        s.set_cd_with_delay(ci, Action::Burst, 15 * 60, 0);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_DANCE_TICK => dance_tick(s, ci, arg >> 1, arg & 1),
            TASK_START_DANCE => start_dance(s, ci, arg >> 1 & 1, arg >> 2, arg & 1 != 0),
            TASK_RECAST_FINAL => recast_final(s, ci),
            TASK_BURST_FINAL => {
                let lvl = s.talent(ci, 2);
                let mut ai = burst_ai(ci, s);
                ai.abil = "Presto: Bluebird Finale (Final)";
                ai.mult = g::BURST_FINAL[lvl];
                ai.pattern = s.circle_on_player(0.0, 0.0, 7.0);
                s.queue_attack(ai, 0, 0, 0);
                // Snow Swan's Dream, and C4's half of it on the others
                let o = Owner::Char(ci as u8);
                let buff = g::BURST_BUFF[lvl];
                st_mut(s, ci).swan_self = buff;
                s.add_mod(ci, Mod::owned(SWANS_DREAM, ModKind::ReactBonus, o, st::NONE, false, true, -1), 20 * 60);
                if s.chars[ci].cons >= 4 {
                    st_mut(s, ci).swan_others = buff * 0.5;
                    for i in 0..s.chars.len() {
                        if i != ci {
                            s.add_mod(i, Mod::owned(SWANS_DREAM, ModKind::ReactBonus, o, st::NONE, false, true, -1), 20 * 60);
                        }
                    }
                }
            }
            TASK_A1_TICK => a1_tick(s, ci, arg >> 2, arg & 3),
            TASK_C2_TICK => c2_tick(s, ci, arg),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 12 * 60, true);
            s.queue_particle(5.0, Element::Cryo, s.particle_delay[ci]);
        }
    }

    /// C2's ATK% by Marvelous Splendor.
    fn mod_amount(s: &Sim, ci: usize, on: usize, _: &Mod, out: &mut Stats) -> bool {
        if !dance(s, ci) {
            return false;
        }
        let state = st(s, ci);
        let stacks = if on == ci { state.a1_self } else { state.a1_others };
        out[st::ATKP] += stacks as f64 * 0.07;
        true
    }

    /// A1's 15% per stack of Marvelous Splendor and Snow Swan's Dream, on
    /// Stellar Glimmer DMG.
    fn react_bonus(s: &Sim, ci: usize, on: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if !info.tag.is_stellar() {
            return 0.0;
        }
        let state = st(s, ci);
        if m.key == SWANS_DREAM {
            return if on == ci { state.swan_self } else { state.swan_others };
        }
        if !dance(s, ci) {
            return 0.0;
        }
        let stacks = if on == ci { state.a1_self } else { state.a1_others };
        stacks as f64 * 0.15
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyHit(ev) if ev.info.tag.is_stellar_direct() => {
                s.hit_base_bonus += stellar_bonus(s, ci);
            }
            Event::SpecialReactionAttack(ev) if ev.info.tag == AttackTag::ReactionStellarSwirl => {
                s.special_base_bonus += stellar_bonus(s, ci);
                // C6, on each contribution
                if s.chars[ci].cons >= 6 {
                    let el = c6_elevation(s, ci, ev.info.actor as usize);
                    s.special_atk.elevation += el;
                }
            }
            // C4: Stellar Glimmer DMG on the enemy brings a coordinated
            // attack, once every 3.5 s
            Event::EnemyDamage(ev) if ev.info.tag.is_stellar() => {
                if s.mod_active(ci, C4_ICD) {
                    return;
                }
                s.add_status(ci, C4_ICD, 210, true);
                let mut ai = if radiance(s, ci) == Radiance::Swirl {
                    stellar_ai(ci, "Odette C4 (Stellar Swirl)", AttackTag::DirectStellarSwirl, 0.99)
                } else {
                    stellar_ai(ci, "Odette C4 (Stellar-Conduct)", AttackTag::DirectStellarConduct, 0.66)
                };
                ai.hitlag_halt = 0.04 * 60.0;
                ai.hitlag_factor = 0.01;
                ai.deployable = true;
                ai.pattern = s.circle_on_target(3.0);
                s.queue_attack(ai, 0, 5, 5);
            }
            _ => {}
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let state = st(s, ci);
        let v = match fields[0].as_str() {
            "a1-stacks-self" => Num::Int(if dance(s, ci) { state.a1_self as i64 } else { 0 }),
            "a1-stacks-other" => Num::Int(if dance(s, ci) { state.a1_others as i64 } else { 0 }),
            _ => return None,
        };
        Some(Ok(v))
    }
}
