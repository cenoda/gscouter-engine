//! Illuga, from gcsim's internal/characters/illuga: normal and charged
//! attacks, Dawnbearing Songbird (tap and hold), Shadowless Reflection
//! (the burst and its Nightingale's Song: flat DMG on the active
//! character's Geo and direct Lunar-Crystallize hits), A1, A4 and every
//! constellation.
//!
//! gcsim's OnConstructSpawned hook of the burst never adds songs (it reads
//! its argument as a Construct, which the event hands as a pointer to
//! one), so only the constructs present as the burst lands count.

use crate::content::{Character, Event};
use crate::gamedata::chars::illuga as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    songs: i32,
    songs_extra_construct: i32,
    c2_quills: i32,
    c4_src: i32,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const A1_CRIT: ModKey = ModKey::new("illuga-a1-crit");
const A1_EM: ModKey = ModKey::new("illuga-a1-em");
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C4: ModKey = ModKey::new("illuga-c4");

const CB_PARTICLE: u16 = 1 << 0;

/// The skill's hit: arg 1 for hold.
const TASK_SKILL: u8 = 0;
/// A1's buffs.
const TASK_A1: u8 = 1;
/// The burst's hit and song.
const TASK_BURST: u8 = 2;
/// C4's ticker, with its src.
const TASK_C4: u8 = 3;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];

impl Character for Char {
    const KEY: &'static str = "illuga";
    const MOONSIGN: i32 = 1;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { songs: 0, songs_extra_construct: 0, c2_quills: 0, c4_src: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // burstBuffInit (its OnConstructSpawned hook does nothing)
        s.subscribe(EventKind::EnemyHit, o);
        // a1Init
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::SpecialReactionAttack, o);
        }
        // c1Init: crystallize cryo, electro, hydro and Lunar-Crystallize
        if s.chars[ci].cons >= 1 {
            s.subscribe(EventKind::Reaction, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let allowed: &[&str] = match a {
            Action::Attack | Action::Charge | Action::Burst => &[],
            Action::Skill => &["hold"],
            _ => return Some(name.into()),
        };
        p.0.iter().find(|(k, _)| !allowed.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let marks = g::ATTACK_HITMARKS[n];
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let hb = g::ATTACK_HITBOXES[n][i];
            let off = g::ATTACK_OFFSETS[n][i];
            ai.pattern = if n == 1 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
            s.queue_attack_at(ci, ai, 0, marks[i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *marks.last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.pattern = s.circle_on_target(0.8);
        let h = g::CHARGE_HITMARK;
        s.queue_attack(ai, 0, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Charge)
    }

    /// Dawnbearing Songbird: tap (on her queue) or hold (on the fight's),
    /// and A1's buffs.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if p.get("hold") == Some(1) {
            s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL, arg: 1 }, g::SKILL_HOLD_HITMARK);
            s.add_task(Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, 33);
            s.set_cd_with_delay(ci, Action::Skill, 15 * 60, 33);
            let t = &g::SKILL_HOLD_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill);
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL, arg: 0 }, g::SKILL_TAP_HITMARK);
        s.add_task(Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, 24);
        s.set_cd_with_delay(ci, Action::Skill, 15 * 60, 24);
        let t = &g::SKILL_TAP_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST, arg: 0 }, g::BURST_HITMARK);
        a1(s, ci);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 6);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL => {
                let lvl = s.talent(ci, 1);
                let hold = arg == 1;
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, 0.0);
                ai.abil = if hold { "Dawnbearing Songbird (Hold)" } else { "Dawnbearing Songbird (Tap)" };
                ai.blunt = true;
                ai.poise = if hold { 100.0 } else { 75.0 };
                ai.pattern = s.box_on_enemy(0.0, -0.3, 2.0, if hold { 35.0 } else { 12.0 });
                let mut ev = s.snapshot_event(ai, CB_PARTICLE);
                let (em, def) = if hold { (g::SKILL_HOLD_EM[lvl], g::SKILL_HOLD_DEF[lvl]) } else { (g::SKILL_TAP_EM[lvl], g::SKILL_TAP_DEF[lvl]) };
                ev.info.flat_dmg = em * ev.stats[st::EM] + def * total_def(&ev.stats);
                s.queue_attack_with_snap(ev, CB_PARTICLE, 0);
            }
            TASK_A1 => a1(s, ci),
            TASK_BURST => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, 0.0);
                ai.abil = "Shadowless Reflection";
                ai.blunt = true;
                ai.poise = 150.0;
                ai.pattern = s.circle_on_player(0.0, 0.0, 6.5);
                let mut ev = s.snapshot_event(ai, 0);
                ev.info.flat_dmg += g::BURST_EM[lvl] * ev.stats[st::EM];
                ev.info.flat_dmg += g::BURST_DEF[lvl] * total_def(&ev.stats);
                s.add_status(ci, BURST, 20 * 60, true);
                let st = s.st_mut::<State>(ci);
                st.c2_quills = 0;
                st.songs = 21;
                st.songs_extra_construct = 15;
                // 5 more songs for each construct within 30 of the player,
                // up to 3
                let (px, py) = s.player_pos;
                for k in 0..s.constructs.len() {
                    if s.st::<State>(ci).songs_extra_construct <= 0 {
                        break;
                    }
                    let Some(p) = s.constructs[k].pos else {
                        s.unsupported("illuga's burst with a construct whose position the engine does not know");
                        return;
                    };
                    if ((p[0] - px).powi(2) + (p[1] - py).powi(2)).sqrt() > 30.0 {
                        continue;
                    }
                    let st = s.st_mut::<State>(ci);
                    st.songs_extra_construct -= 5;
                    st.songs += 5;
                }
                s.queue_attack_with_snap(ev, 0, 0);
                // C4: DEF +200 for the active character every second
                if s.chars[ci].cons >= 4 {
                    let f = s.f;
                    s.st_mut::<State>(ci).c4_src = f;
                    c4_tick(s, ci, f);
                }
            }
            TASK_C4 => c4_tick(s, ci, arg),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 30, true);
            let n = if s.rng.float64() < 0.5 { 5.0 } else { 4.0 };
            s.queue_particle(n, Element::Geo, s.particle_delay[ci]);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == A1_EM {
            out[st::EM] += g::A1_EM as f64 + c6(s, ci, 0.0, 0.0, (g::C6_EM - g::A1_EM) as f64).2;
            true
        } else if m.key == C4 {
            out[st::DEF] += 200.0;
            true
        } else {
            false
        }
    }

    /// A1: Geo CRIT Rate and DMG of the other members.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A1_CRIT && ev.info.element == Element::Geo {
            let (cr, cd, _) = c6(s, ci, g::C6_CR - g::A1_CR, g::C6_CD - g::A1_CD, 0.0);
            ev.stats[st::CR] += g::A1_CR + cr;
            ev.stats[st::CD] += g::A1_CD + cd;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // Nightingale's Song: flat DMG on the active character's Geo
            // hits
            Event::EnemyHit(ev) => {
                if ev.info.element != Element::Geo || ev.info.actor != s.player.active || !s.mod_active(ci, BURST) || s.st::<State>(ci).songs <= 0 {
                    return;
                }
                let lvl = s.talent(ci, 2);
                let amt = match ev.info.tag {
                    AttackTag::ElementalBurst | AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge => (g::BURST_BUFF_GEO[lvl] + g::A4_GEO_BUFF[a4_count(s, ci)]) * s.stat(ci, st::EM),
                    AttackTag::DirectLunarCrystallize => (g::BURST_BUFF_LCR[lvl] + g::A4_LCR_BUFF[a4_count(s, ci)]) * s.stat(ci, st::EM),
                    _ => return,
                };
                s.st_mut::<State>(ci).songs -= 1;
                s.hit_flat += amt;
                c2_increment(s, ci);
                if s.st::<State>(ci).songs <= 0 {
                    s.delete_mod(ci, BURST);
                }
            }
            // A1: CRIT on the others' Lunar-Crystallize contributions
            Event::SpecialReactionAttack(ev) => {
                if ev.info.tag != AttackTag::ReactionLunarCrystallize || ev.info.actor as usize == ci {
                    return;
                }
                let (cr, cd, _) = c6(s, ci, g::C6_CR - g::A1_CR, g::C6_CD - g::A1_CD, 0.0);
                s.special_atk.stats[st::CR] += g::A1_CR + cr;
                s.special_atk.stats[st::CD] += g::A1_CD + cd;
            }
            // C1: 12 energy on her own crystallize while she is active
            Event::Reaction(r, ev) => {
                if !matches!(r, Reaction::CrystallizeCryo | Reaction::CrystallizeElectro | Reaction::CrystallizeHydro | Reaction::LunarCrystallize) {
                    return;
                }
                let active = s.player.active as usize;
                if ci != active || s.mod_active(ci, C1_ICD) || ev.info.actor as usize != ci {
                    return;
                }
                s.add_status(ci, C1_ICD, 15 * 60, true);
                s.add_energy(ci, 12.0);
            }
            _ => {}
        }
    }
}

/// C6's raises over A1, as (CR, CD, EM).
fn c6(s: &Sim, ci: usize, cr: f64, cd: f64, em: f64) -> (f64, f64, f64) {
    if s.chars[ci].cons < 6 {
        (0.0, 0.0, 0.0)
    } else {
        (cr, cd, em)
    }
}

fn moonsign_level(s: &Sim) -> i32 {
    s.chars.iter().map(|c| c.moonsign).sum()
}

/// A1: the other members' Geo CRIT for 20 s, and with Ascendant Gleam
/// their EM.
fn a1(s: &mut Sim, ci: usize) {
    if s.chars[ci].ascension < 1 {
        return;
    }
    let o = Owner::Char(ci as u8);
    let gleam = moonsign_level(s) >= 2;
    for i in 0..s.chars.len() {
        if i == ci {
            continue;
        }
        s.add_mod(i, Mod::owned(A1_CRIT, ModKind::Attack, o, st::NONE, false, true, -1), 20 * 60);
        if gleam {
            s.add_mod(i, Mod::owned(A1_EM, ModKind::Stat, o, st::NONE, false, true, -1), 20 * 60);
        }
    }
}

/// A4: the geo and hydro members, up to 3.
fn a4_count(s: &Sim, ci: usize) -> usize {
    if s.chars[ci].ascension < 4 {
        return 0;
    }
    s.chars.iter().filter(|c| matches!(c.element, Element::Geo | Element::Hydro)).count().min(3)
}

/// C2: every 7 songs used, a hit of 400% EM and 200% DEF.
fn c2_increment(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 2 {
        return;
    }
    let st = s.st_mut::<State>(ci);
    st.c2_quills += 1;
    if st.c2_quills < g::C2_QUILL_THRESHOLD {
        return;
    }
    st.c2_quills -= g::C2_QUILL_THRESHOLD;
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, 0.0);
    ai.abil = "Aedon C2 Hit";
    ai.blunt = true;
    ai.poise = 75.0;
    ai.flat_dmg += s.stat(ci, st::EM) * 4.0;
    ai.flat_dmg += total_def(&s.stats(ci)) * 2.0;
    ai.pattern = Pattern::Single;
    let h = g::C2_HITMARK;
    s.queue_attack(ai, 0, h, h);
}

fn c4_tick(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).c4_src != src || !s.mod_active(ci, BURST) {
        return;
    }
    let active = s.player.active as usize;
    s.add_mod(active, Mod::owned(C4, ModKind::Stat, Owner::Char(ci as u8), st::NONE, false, true, -1), 66);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C4, arg: src }, 60);
}
