//! Vodyanitsa, from gcsim's internal/characters/vodyanitsa: normal and
//! charged attacks, the skill (Rechitativ: Sonorous Dawn) and its Song of
//! Ages Past (the Horns of Spring's Call, the heals, the Hydro and Cryo RES
//! shred and the party buff), the burst (Koda: Sink With Thee), A1 (the
//! Wandering Vortex: Stellar Vortices made while the party buff is on
//! shred Anemo RES, and the AlterMark), A4 (Lead Vocal and Chorus) and C1
//! to C6. She does not enable Stellar Swirl herself; her A1 hooks into the
//! Stellar Vortex of `stellar.rs` (gcsim's WanderingVortexKey). No plunges
//! (gcsim has none).
//!
//! C2's status and its attack mod share gcsim's key
//! ("two-voices-in-black-and-white") in one list of mods: until the first
//! horn the attack mod (expiry -1) reads as the status, so C2 applies from
//! the start; the first horn's status then replaces her own attack mod,
//! and her own hits no longer get C2. The engine keeps both under one key,
//! so it does the same.

use crate::content::{Character, Event, EventKind, Owner};
use crate::gcsimdata::chars::vodyanitsa as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::stellar::RADIANCE_SWIRL;
use crate::types::*;

pub struct Char;

pub struct State {
    /// gcsim's songSrc.
    song_src: i32,
    lead_vocal: i32,
    chorus: i32,
    /// The id of the Stellar Vortex A1 follows (0 for none), and whether it
    /// became a Wandering Vortex.
    vortex: u32,
    wandering: bool,
    /// The expiry frames of C4's stacks.
    c4_stacks: [i32; 3],
    c4_len: usize,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_KEY);
const SONG: ModKey = ModKey::new(g::SONG_KEY);
const SONG_BUFF: ModKey = ModKey::new(g::SONG_BUFF_KEY);
const HYDRO_SHRED: ModKey = ModKey::new(g::HYDRO_SHRED_KEY);
const CRYO_SHRED: ModKey = ModKey::new(g::CRYO_SHRED_KEY);
const A1_SHRED: ModKey = ModKey::new(g::A1_SHRED_KEY);
const ALTER_MARK: ModKey = ModKey::new(g::ALTER_MARK_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const C1: ModKey = ModKey::new(g::C1_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C4: ModKey = ModKey::new(g::C4_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);

const CB_SHRED: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;

const TASK_START_SONG: u8 = 0;
const TASK_A4_GRANT: u8 = 1;
// arg: the song's source frame
const TASK_SONG_BUFF: u8 = 2;
const TASK_HORN: u8 = 3;
const TASK_HEAL: u8 = 4;
const TASK_C2_APPLY: u8 = 5;
const TASK_BURST: u8 = 6;
// arg: the vortex id
const TASK_A1_CHECK: u8 = 7;

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn song_duration(s: &Sim, ci: usize) -> i32 {
    let mut dur = g::SKILL_DUR * 60;
    if s.chars[ci].cons >= 2 {
        dur += g::C2_SONG_EXTENSION;
    }
    dur
}

/// The closure `active` of gcsim's startSong.
fn song_active(s: &Sim, ci: usize, src: i32) -> bool {
    st(s, ci).song_src == src && s.f - src < song_duration(s, ci)
}

/// gcsim's startSong: the skill restarts Song of Ages Past.
fn start_song(s: &mut Sim, ci: usize) {
    let src = s.f;
    st_mut(s, ci).song_src = src;
    let dur = song_duration(s, ci);
    s.add_status(ci, SONG, dur, false);
    song_buff(s, ci, src);
    s.add_task(Task::Char { c: ci as u8, id: TASK_HORN, arg: src }, g::HORN_INTERVAL * 60);
    let heal_every = (g::HEAL_INTERVAL * 60.0) as i32;
    s.add_task(Task::Char { c: ci as u8, id: TASK_HEAL, arg: src }, g::HEAL_FIRST + heal_every);
}

fn song_buff(s: &mut Sim, ci: usize, src: i32) {
    if !song_active(s, ci, src) {
        return;
    }
    s.add_status(ci, SONG_BUFF, g::SONG_BUFF_DUR, false);
    s.add_task(Task::Char { c: ci as u8, id: TASK_SONG_BUFF, arg: src }, g::SONG_BUFF_TICK);
}

fn horn(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::HORN[lvl]);
    ai.abil = "Horn of Spring's Call";
    ai.use_hp = true;
    ai.pattern = s.circle_on_target(3.0);
    s.queue_attack(ai, CB_SHRED | CB_PARTICLE, g::HORN_DELAY, g::HORN_DELAY);
    // C2: the bullet applies Two Voices in Black and White when it attacks
    s.add_task(Task::Char { c: ci as u8, id: TASK_C2_APPLY, arg: 0 }, g::HORN_DELAY);
}

fn heal(s: &mut Sim, ci: usize) {
    let active = s.player.active as usize;
    let lvl = s.talent(ci, 1);
    let mut amount = g::HEAL_FLAT[lvl] + g::HEAL_PCT[lvl] * s.max_hp(ci);
    if s.chars[ci].cons >= 4 {
        if s.chars[active].hp_ratio >= g::C4_HP_THRESHOLD {
            c4_stack(s, ci);
        } else {
            amount *= 1.0 + g::C4_HEAL_BONUS;
        }
    }
    let bonus = s.stat(ci, st::HEAL);
    s.heal(ci, active, amount, bonus);
    c1_apply(s, ci);
}

/// C1: each heal raises the party's ATK by 0.8% of her Max HP for 5 s.
fn c1_apply(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 1 {
        return;
    }
    let v = g::C1_RATIO * s.max_hp(ci);
    for i in 0..s.chars.len() {
        s.add_mod(i, Mod::stat(C1, st::ATK, v, true, false, -1), g::C1_DUR);
    }
}

/// C4: a stack for 6 s, each with its own expiry: an expired one is
/// replaced, else one is added, else the oldest is replaced.
fn c4_stack(s: &mut Sim, ci: usize) {
    let f = s.f;
    let exp = f + g::C4_DUR;
    let state = st_mut(s, ci);
    let mut oldest: Option<usize> = None;
    for i in 0..state.c4_len {
        let e = state.c4_stacks[i];
        if e <= f {
            state.c4_stacks[i] = exp;
            return;
        }
        if oldest.is_none_or(|o| e < state.c4_stacks[o]) {
            oldest = Some(i);
        }
    }
    if state.c4_len < g::C4_MAX_STACKS as usize {
        state.c4_stacks[state.c4_len] = exp;
        state.c4_len += 1;
        return;
    }
    state.c4_stacks[oldest.unwrap()] = exp;
}

/// C2's status, at each horn.
fn c2_apply(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 2 {
        return;
    }
    s.add_status(ci, C2, g::C2_DUR, false);
}

fn c2_applies(s: &Sim, ci: usize, on: usize) -> bool {
    if !s.mod_active(ci, C2) {
        return false;
    }
    s.chars[ci].cons >= 6 || on == s.player.active as usize
}

/// A1's Mark: on the party while the song's party buff is on.
fn mark_active(s: &Sim, ci: usize) -> bool {
    s.chars[ci].ascension >= 1 && s.mod_active(ci, SONG_BUFF)
}

/// A1's shred on the enemy if its center is in the area.
fn a1_shred(s: &mut Sim, ap: Pattern) {
    if s.enemy_alive && s.enemy_within(ap) {
        s.enemy_add_resist(A1_SHRED, Element::Anemo, -g::A1_SHRED, g::A1_SHRED_DUR);
    }
}

fn a1_check(s: &mut Sim, ci: usize) {
    if !mark_active(s, ci) {
        return;
    }
    if !st(s, ci).wandering {
        st_mut(s, ci).wandering = true;
        let (x, y) = s.stellar.vortex_pos().expect("a Wandering Vortex without a vortex");
        a1_shred(s, Pattern::Circle { x, y, r: g::A1_SHRED_RADIUS as f64 });
    }
    s.add_status(ci, ALTER_MARK, g::ALTER_MARK_DUR, false);
}

/// gcsim's OnStellarSwirlVortex subscription of A1: after each Stellar
/// Swirl, with the vortex it made or stacked.
pub(crate) fn on_vortex(s: &mut Sim, ci: usize, id: u32) {
    if mark_active(s, ci) {
        for i in 0..s.chars.len() {
            s.add_status(i, RADIANCE_SWIRL, g::A1_RADIANCE_DUR, false);
        }
    }
    if st(s, ci).vortex == id {
        return;
    }
    let state = st_mut(s, ci);
    state.vortex = id;
    state.wandering = false;
    a1_tick(s, ci, id);
}

fn a1_tick(s: &mut Sim, ci: usize, id: u32) {
    if st(s, ci).vortex != id {
        return;
    }
    a1_check(s, ci);
    s.add_task(Task::Char { c: ci as u8, id: TASK_A1_CHECK, arg: id as i32 }, g::A1_CHECK_EVERY);
}

/// gcsim's OnStellarVortexDetonate subscription of A1, with the
/// detonation's area.
pub(crate) fn on_detonate(s: &mut Sim, ci: usize, ap: Pattern) {
    let state = st_mut(s, ci);
    if state.vortex == 0 {
        return;
    }
    let wandering = state.wandering;
    state.vortex = 0;
    state.wandering = false;
    if wandering {
        a1_shred(s, ap);
    }
}

/// A4's flat damage on a hit or contribution, by one stack.
fn a4_consume(s: &mut Sim, ci: usize, actor: usize, per_kilo: f64, max_dmg: f64) -> f64 {
    if !s.mod_active(ci, A4) {
        return 0.0;
    }
    let active = s.player.active as usize == actor;
    let state = st_mut(s, ci);
    if active {
        if state.lead_vocal <= 0 {
            return 0.0;
        }
        state.lead_vocal -= 1;
    } else {
        if state.chorus <= 0 {
            return 0.0;
        }
        state.chorus -= 1;
    }
    ((s.max_hp(ci) - g::A4_MIN_HP as f64).max(0.0) * per_kilo / 1000.0).min(max_dmg)
}

impl Character for Char {
    const KEY: &'static str = "vodyanitsa";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { song_src: 0, lead_vocal: 0, chorus: 0, vortex: 0, wandering: false, c4_stacks: [0; 3], c4_len: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // A1: the Stellar Vortex hooks
        if asc >= 1 {
            s.stellar.wandering = Some(ci as u8);
        }
        if asc >= 4 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        if asc >= 4 || cons >= 2 {
            s.subscribe(EventKind::SpecialReactionAttack, o);
        }
        if cons >= 2 {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(C2, ModKind::Attack, o, st::NONE, false, false, -1));
            }
        }
        if cons >= 4 {
            s.chars[ci].mods.push(Mod::owned(C4, ModKind::Stat, o, st::HPP, false, false, -1));
        }
        if cons >= 6 {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(C6, ModKind::Attack, o, st::NONE, false, false, -1));
            }
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge => p.0.iter().find(|(k, _)| k != "travel").map(|(k, _)| format!("{name} param {k}")),
            Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.circle_on_target(g::ATTACK_RADIUS[n]);
        let h = g::ATTACK_HITMARKS[n];
        // N4 is an AoE at the target, not a bullet
        let dmg = if n == 3 { h } else { h + travel };
        s.queue_attack(ai, 0, h, dmg);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.pattern = s.circle_on_target(2.0);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK + travel);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::SKILL[lvl]);
        ai.abil = "Rechitativ: Sonorous Dawn";
        ai.use_hp = true;
        ai.pattern = s.circle_on_player(0.0, 0.0, 5.5);
        s.queue_attack(ai, CB_SHRED, g::SKILL_HITMARK, g::SKILL_HITMARK);
        s.add_task(Task::Char { c: ci as u8, id: TASK_START_SONG, arg: 0 }, g::SKILL_HITMARK);
        s.add_task(Task::Char { c: ci as u8, id: TASK_A4_GRANT, arg: 0 }, g::LEAD_VOCAL_DELAY);
        s.set_cd(ci, Action::Skill, g::SKILL_CD);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST, arg: 0 }, g::BURST_HITMARK);
        s.set_cd(ci, Action::Burst, g::BURST_CD);
        s.consume_energy(ci, 5);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_START_SONG => start_song(s, ci),
            TASK_A4_GRANT => {
                if s.chars[ci].ascension < 4 {
                    return;
                }
                let state = st_mut(s, ci);
                state.lead_vocal = g::A4_LEAD_VOCAL;
                state.chorus = g::A4_CHORUS;
                s.add_status(ci, A4, g::A4_STACK_DUR, false);
            }
            TASK_SONG_BUFF => song_buff(s, ci, arg),
            TASK_HORN => {
                if !song_active(s, ci, arg) {
                    return;
                }
                horn(s, ci);
                s.add_task(Task::Char { c: ci as u8, id: TASK_HORN, arg }, g::HORN_INTERVAL * 60);
            }
            TASK_HEAL => {
                if !song_active(s, ci, arg) {
                    return;
                }
                heal(s, ci);
                s.add_task(Task::Char { c: ci as u8, id: TASK_HEAL, arg }, (g::HEAL_INTERVAL * 60.0) as i32);
            }
            TASK_C2_APPLY => c2_apply(s, ci),
            TASK_BURST => {
                let lvl = s.talent(ci, 2);
                let mut mult = g::BURST[lvl];
                let mut abil = "Koda: Sink With Thee";
                if s.mod_active(ci, SONG) {
                    mult *= 1.0 + g::BURST_BONUS[lvl];
                    abil = "Koda: Sink With Thee (Song of Ages Past)";
                }
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 50.0, mult);
                ai.abil = abil;
                ai.use_hp = true;
                ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_A1_CHECK => a1_tick(s, ci, arg as u32),
            _ => unreachable!(),
        }
    }

    /// The skill's hits shred Hydro and Cryo RES; each horn hit makes a
    /// particle, once every 0.2 s.
    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_SHRED != 0 {
            let shred = -g::SKILL_SHRED[s.talent(ci, 1)];
            let dur = g::SKILL_SHRED_DUR * 60;
            s.enemy_add_resist(HYDRO_SHRED, Element::Hydro, shred, dur);
            s.enemy_add_resist(CRYO_SHRED, Element::Cryo, shred, dur);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD, false);
            s.queue_particle(1.0, Element::Hydro, s.particle_delay[ci]);
        }
    }

    /// C4's Max HP by its stacks.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let state = st(s, ci);
        let n = state.c4_stacks[..state.c4_len].iter().filter(|&&e| e > s.f).count();
        if n == 0 {
            return false;
        }
        out[st::HPP] += g::C4_HP_RATIO * n as f64;
        true
    }

    /// C2's CRIT DMG and C6's DMG% on Hydro and Cryo hits.
    fn attack_mod(s: &Sim, ci: usize, on: usize, m: &Mod, ev: &mut AttackEvent) {
        let info = &ev.info;
        if info.tag.is_reaction() || info.tag.is_direct() || !matches!(info.element, Element::Hydro | Element::Cryo) {
            return;
        }
        if m.key == C2 {
            if s.mod_active(ci, ALTER_MARK) || !c2_applies(s, ci, on) {
                return;
            }
            ev.stats[st::CD] += g::C2_HYDRO_CRYO_CD;
        } else if m.key == C6 && s.mod_active(ci, SONG_BUFF) {
            ev.stats[st::DMGP] += g::C6_HYDRO_CRYO_DMG;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // A4: a stack on a Hydro or Cryo hit, or on a direct Stellar
            // Swirl hit while the AlterMark is on
            Event::EnemyHit(ev) => {
                let info = &ev.info;
                let alter = s.mod_active(ci, ALTER_MARK);
                if matches!(info.element, Element::Hydro | Element::Cryo)
                    && matches!(info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge | AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst)
                    && !alter
                {
                    s.hit_flat += a4_consume(s, ci, info.actor as usize, g::A4_PER_KILO_HC as f64, g::A4_MAX_HC as f64);
                }
                if info.tag == AttackTag::DirectStellarSwirl && alter {
                    s.hit_flat += a4_consume(s, ci, info.actor as usize, g::A4_PER_KILO_S_SW as f64, g::A4_MAX_S_SW as f64);
                }
            }
            // A4, C2 and C6 on each Stellar Swirl contribution
            Event::SpecialReactionAttack(ev) => {
                if ev.info.tag != AttackTag::ReactionStellarSwirl {
                    return;
                }
                let actor = ev.info.actor as usize;
                let alter = s.mod_active(ci, ALTER_MARK);
                if s.chars[ci].ascension >= 4 && alter {
                    let dmg = a4_consume(s, ci, actor, g::A4_PER_KILO_S_SW as f64, g::A4_MAX_S_SW as f64);
                    s.special_atk.info.flat_dmg += dmg;
                }
                let cons = s.chars[ci].cons;
                if cons >= 2 && alter && c2_applies(s, ci, actor) {
                    s.special_atk.stats[st::CD] += g::C2_STELLAR_CD;
                }
                if cons >= 6 && s.mod_active(ci, SONG_BUFF) {
                    s.special_atk.elevation += (g::C6_ELEVATION * 1000.0).round() as u16;
                }
            }
            _ => {}
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let state = st(s, ci);
        let v = match fields[0].as_str() {
            "song" => Num::from_bool(s.mod_active(ci, SONG)),
            "lead-vocal" => Num::Int(state.lead_vocal as i64),
            "chorus" => Num::Int(state.chorus as i64),
            "wandering" => Num::from_bool(s.mod_active(ci, ALTER_MARK)),
            _ => return None,
        };
        Some(Ok(v))
    }
}
