//! Mona, from gcsim's internal/characters/mona: normal and charged attacks,
//! Mirror Reflection of Doom (tap and hold), Stellaris Phantasm (the
//! bubble, its break and Omen), Illusory Torrent (her own dash: no dash
//! cooldown, a hydro hit, A1's phantoms), A1, A4, every constellation and
//! her Hexerei bonuses.
//!
//! gcsim makes her Hexerei unless her param hexerei=0. Some Hexerei parts
//! need a second Hexerei member (Astral Glow, the Omen refresh), others
//! only her own (C1's 160% off field, C2's post-burst charged attack and
//! EM, C4's CRIT DMG, C6's doubled charged attack and its Omen ticker), as
//! gcsim checks them.
//!
//! The bubble is a status on the enemy: any damage on it breaks it (into
//! Omen and the explosion), unless the hit has no impulse (a frozen enemy,
//! Klee's burst); the break attack 8 s after the cast always does.

use std::cell::Cell;

use crate::content::{Character, Event};
use crate::gamedata::chars::mona as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// A4's HYDRO DMG, refreshed every second (gcsim's a4Stats).
    a4_hydro: f64,
    /// gcsim's c6Src (0 until first set, which keeps the first dash's
    /// ticker from starting), c6Stacks and c6NearbyOmen.
    c6_src: i32,
    c6_stacks: i32,
    c6_nearby_omen: bool,
    /// Astral Glow's stacks; gcsim clears them inside the reaction bonus
    /// that spends them.
    astral_glow: Cell<i32>,
    astral_glow_src: i32,
    /// The Omen refreshes left (gcsim's tag "omen-debuff" on the enemy).
    omen_refresh: i32,
}

const BUBBLE: ModKey = ModKey::new(g::BUBBLE_KEY);
const OMEN: ModKey = ModKey::new(g::OMEN_KEY);
const OMEN_REFRESH_ICD: ModKey = ModKey::new(g::OMEN_REFRESH_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const ASTRAL_GLOW: ModKey = ModKey::new(g::ASTRAL_GLOW_KEY);
const ASTRAL_GLOW_ICD: ModKey = ModKey::new(g::ASTRAL_GLOW_ICD_KEY);
const OMEN_DMG: ModKey = ModKey::new("mona-omen");
const A4: ModKey = ModKey::new("mona-a4");
const HEX_VAPE: ModKey = ModKey::new("mona-hexerei-astral-glow-vaporize");
const C1: ModKey = ModKey::new("mona-c1");
const C2_ICD: ModKey = ModKey::new(g::C2ICDKEY);
const C2_HEX_POST_BURST: ModKey = ModKey::new(g::C2_HEXEREI_POST_BURST_CA_KEY);
const C2_HEX_EM: ModKey = ModKey::new("mona-hexerei-c2-em");
const C4: ModKey = ModKey::new(g::C4KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);

const CB_ASTRAL_GLOW: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;
const CB_BUBBLE: u16 = 1 << 2;
const CB_C6_RESET: u16 = 1 << 3;
const CB_C2_HEX: u16 = 1 << 4;

/// A4's refresh (her queue).
const TASK_A4: u8 = 0;
/// A1's check while she dashes (the fight's queue).
const TASK_A1: u8 = 1;
/// A1's phantom exploding (the fight's queue).
const TASK_A1_EXPLODE: u8 = 2;
/// Astral Glow's end, with its src.
const TASK_ASTRAL_GLOW_END: u8 = 3;
/// C1's reaction bonus on a member, a frame after the hit (the fight's
/// queue): arg is the member.
const TASK_C1: u8 = 4;
/// C6's ticker, with its src.
const TASK_C6_TICK: u8 = 5;
/// C6's Hexerei ticker for Omen nearby.
const TASK_C6_OMEN: u8 = 6;

/// Frames of the charged attack (arg: its windup) and the dash (arg: f).
const FR_CHARGE: u8 = 0;
const FR_DASH: u8 = 1;

const REMOVED_DASH: u8 = 0;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];

impl Character for Char {
    const KEY: &'static str = "mona";
    const HEXEREI: bool = true;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { a4_hydro: 0.0, c6_src: 0, c6_stacks: 0, c6_nearby_omen: false, astral_glow: Cell::new(0), astral_glow_src: 0, omen_refresh: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons, hex) = (s.chars[ci].ascension, s.chars[ci].cons, s.chars[ci].hexerei);
        let hex2 = hex && s.hexerei_count() >= 2;
        let n = s.chars.len();
        // burstHook, and C1's and C2's checks after it
        s.subscribe(EventKind::EnemyDamage, o);
        // burstDamageBonus: Omen's DMG bonus for everyone
        for i in 0..n {
            s.chars[i].mods.push(Mod::owned(OMEN_DMG, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        // a4: HYDRO DMG from 20% of her ER without extra mods, every second
        if asc >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Stat, o, st::HYDROP, true, false, -1));
            a4_update(s, ci);
        }
        // hexInit: Astral Glow spent on the others' vaporize
        if hex2 {
            for i in 0..n {
                if i != ci {
                    s.chars[i].mods.push(Mod::owned(HEX_VAPE, ModKind::ReactBonus, o, st::NONE, false, false, -1));
                }
            }
        }
        if cons >= 4 {
            for i in 0..n {
                s.chars[i].mods.push(Mod::owned(C4, ModKind::Attack, o, st::NONE, false, false, -1));
            }
            s.subscribe(EventKind::SpecialReactionAttack, o);
        }
        if cons >= 6 && hex {
            s.subscribe(EventKind::EnemyHit, o);
            c6_omen_ticker(s, ci);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            Action::Skill => p.0.iter().find(|(k, _)| k != "hold").map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    /// No charged attack right after the normal attack that ended the
    /// combo; then the template's.
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::Charge && s.player.last.action == Action::Attack && s.chars[s.player.active as usize].normal_counter == 0 {
            return Some("mona: need to use attack right before charge".into());
        }
        crate::content::template_invalid(s, a)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let st = s.st::<State>(ci);
        match fields[0].as_str() {
            "astral-glow" => Some(Ok(Num::Int(st.astral_glow.get() as i64))),
            "c6-stacks" => Some(Ok(Num::Int(if s.mod_active(ci, C6) { st.c6_stacks as i64 } else { 0 }))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let h = g::ATTACK_HITMARKS[n];
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_MONA_WATER_DAMAGE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.circle_on_target(g::ATTACK_RADIUS[n]);
        s.queue_attack(ai, CB_ASTRAL_GLOW, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    /// The charged attack, 14 frames sooner from idle or a swap.
    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let windup = if matches!(s.current_state(), AnimState::Idle | AnimState::Swap) { 0 } else { 14 };
        do_ca(s, ci, g::CHARGE_HITMARK - windup);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: FR_CHARGE, arg: windup as i16 }, t[0] - windup, t[Action::Swap as usize] - windup, AnimState::Charge)
    }

    /// Mirror Reflection of Doom: the phantom's four ticks (snapshot now)
    /// and its explosion.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = (p.get("hold").unwrap_or(0) != 0) as usize;
        let lvl = s.talent(ci, 1);
        let ap = s.circle_on_player(0.0, 0.0, 5.0);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::SKILL_DOT[lvl]);
        ai.abil = "Mirror Reflection of Doom (Tick)";
        ai.pattern = ap;
        let ev = s.snapshot_event(ai, 0);
        let mut i = g::SKILL_HITMARKS[hold];
        while i < 300 {
            s.queue_attack_with_snap(ev, 0, i);
            i += 59;
        }
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::SKILL[lvl]);
        ai.abil = "Mirror Reflection of Doom (Explode)";
        ai.pattern = ap;
        s.queue_attack(ai, CB_PARTICLE, 0, g::SKILL_HITMARKS[hold] + 243);
        s.set_cd_with_delay(ci, Action::Skill, 12 * 60, 24);
        let t = &g::SKILL_FRAMES[hold];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Burst as usize], AnimState::Skill)
    }

    /// Stellaris Phantasm: the bubble on the enemy, and its break 8 s later.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
        ai.abil = "Illusory Bubble (Initial)";
        ai.pattern = s.circle_on_player(0.0, 0.0, 10.0);
        // no snapshot (gcsim's snapshot delay -1)
        s.queue_attack_with_snap(AttackEvent::new(ai, CB_BUBBLE), CB_BUBBLE, g::BURST_HITMARK);
        if s.chars[ci].hexerei {
            s.add_status(ci, C2_HEX_POST_BURST, 5 * 60, false);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::MonaBubbleBreak, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 0.0, 0.0);
        ai.abil = "Illusory Bubble (Break)";
        ai.pattern = s.circle_on_player(0.0, 0.0, 10.0);
        s.queue_attack_with_snap(AttackEvent::new(ai, 0), 0, g::BURST_HITMARK + 480);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 5);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Burst)
    }

    /// Illusory Torrent: no dash cooldown; a hydro hit at its end, A1's
    /// check in 2 s, C6's ticker, and the stamina at the template's time.
    fn own_dash(s: &mut Sim, ci: usize, p: &Params) -> Option<ActInfo> {
        let f = p.get("f").unwrap_or(0) as i32;
        let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_DASH, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
        ai.abil = "Dash";
        ai.pattern = s.circle_on_player(0.0, 0.1, 2.0);
        let h = g::DASH_HITMARK + f;
        s.queue_attack(ai, 0, h, h);
        s.add_task(Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, 120);
        if s.chars[ci].cons >= 6 {
            c6_start_ticker(s, ci);
        }
        // QueueDashStaminaConsumption
        let len = s.chars[ci].dash_len;
        s.add_task(Task::DashStam { c: ci as u8 }, len - 1);
        let t = &g::DASH_FRAMES;
        let mut info = ActInfo::new(Frames::Char { id: FR_DASH, arg: f as i16 }, t[0] + f, h, AnimState::Dash);
        info.on_removed = OnRemoved::Char(REMOVED_DASH);
        Some(info)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_CHARGE => g::CHARGE_FRAMES[next as usize] - arg as i32,
            FR_DASH => g::DASH_FRAMES[next as usize] + arg as i32,
            _ => unreachable!(),
        }
    }

    /// c6OnDashEnd: the ticker stops without Omen nearby.
    fn on_removed(s: &mut Sim, ci: usize, id: u8, _: AnimState) {
        if id == REMOVED_DASH && s.chars[ci].cons >= 6 && !s.st::<State>(ci).c6_nearby_omen {
            s.st_mut::<State>(ci).c6_src = -1;
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_A4 => a4_update(s, ci),
            TASK_A1 => a1(s, ci),
            TASK_A1_EXPLODE => {
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.5 * g::SKILL[lvl]);
                ai.abil = "Mirror Reflection of Doom (A1 Explode)";
                // gcsim centers it on the player, where it is now
                ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_ASTRAL_GLOW_END => {
                let st = s.st::<State>(ci);
                if st.astral_glow_src == arg {
                    st.astral_glow.set(0);
                }
            }
            TASK_C1 => {
                let on = arg as usize;
                s.add_mod(on, Mod::owned(C1, ModKind::ReactBonus, Owner::Char(ci as u8), st::NONE, false, true, -1), 8 * 60);
            }
            TASK_C6_TICK => c6_tick(s, ci, arg),
            TASK_C6_OMEN => c6_omen_ticker(s, ci),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_BUBBLE != 0 {
            // lasts 8 s if not broken, a frame more so that the break
            // attack finds it
            s.enemy_add_status(BUBBLE, 481, true);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 60, false);
            let count = if s.rng.float64() < 0.33 { 4.0 } else { 3.0 };
            s.queue_particle(count, Element::Hydro, s.particle_delay[ci]);
        }
        // makeC6CAResetCB
        if cbs & CB_C6_RESET != 0 && s.mod_active(ci, C6) {
            s.delete_mod(ci, C6);
            s.st_mut::<State>(ci).c6_stacks = 0;
        }
        if cbs & CB_ASTRAL_GLOW != 0 {
            astral_glow_gain(s, ci);
        }
        // c2HexereiCB: EM +80 for everyone
        if cbs & CB_C2_HEX != 0 {
            for i in 0..s.chars.len() {
                s.add_stat_mod(i, C2_HEX_EM, st::EM, 80.0, 12 * 60, true);
            }
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        // A4
        out[st::HYDROP] += s.st::<State>(ci).a4_hydro;
        true
    }

    fn attack_mod(s: &Sim, ci: usize, on: usize, m: &Mod, ev: &mut AttackEvent) {
        let marked = s.enemy_status_active(BUBBLE) || s.enemy_status_active(OMEN);
        if m.key == OMEN_DMG {
            if marked {
                ev.stats[st::DMGP] += g::DMG_BONUS[s.talent(ci, 2)];
            }
        } else if m.key == C4 {
            if marked {
                ev.stats[st::CR] += 0.15;
                if s.chars[ci].hexerei && s.chars[on].hexerei {
                    ev.stats[st::CD] += 0.15;
                }
            }
        } else if m.key == C6 {
            let stacks = s.st::<State>(ci).c6_stacks;
            if ev.info.tag == AttackTag::Extra && stacks != 0 {
                ev.stats[st::DMGP] += 0.60 * stacks as f64;
            }
        }
    }

    fn react_bonus(s: &Sim, ci: usize, on: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if m.key == HEX_VAPE {
            let glow = &s.st::<State>(ci).astral_glow;
            let v = 0.05 * glow.get() as f64;
            if is_vaporize(info) {
                glow.set(0);
                return v;
            }
            return 0.0;
        }
        // C1
        let mut v = 0.15;
        if s.chars[ci].hexerei && s.player.active as usize != on {
            v *= 1.6;
        }
        match info.tag {
            AttackTag::SwirlHydro | AttackTag::ECDamage | AttackTag::ReactionLunarCharge | AttackTag::DirectLunarCharged | AttackTag::ReactionLunarCrystallize | AttackTag::DirectLunarCrystallize => v,
            _ if is_vaporize(info) => v,
            _ => 0.0,
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyDamage(ev) => {
                // burstHook: her normal and charged attacks refresh Omen
                // (Hexerei), then the bubble breaks
                if ev.info.actor as usize == ci && matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
                    omen_refresh(s, ci);
                }
                if s.enemy_status_active(BUBBLE) && (ev.info.tag == AttackTag::MonaBubbleBreak || !ev.info.no_impulse) {
                    bubble_burst(s, ci);
                }
                // C1: the hitter's hydro reactions, from the next frame
                if s.chars[ci].cons >= 1 && s.enemy_status_active(OMEN) {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_C1, arg: ev.info.actor as i32 }, 1);
                }
                // C2: a normal attack's hit is followed by a charged attack,
                // 20% of the time (always after a Hexerei burst), once in 5 s
                if s.chars[ci].cons >= 2 && ev.info.actor as usize == ci && ev.info.tag == AttackTag::Normal {
                    if !s.mod_active(ci, C2_HEX_POST_BURST) && s.rng.float64() > 0.2 {
                        return;
                    }
                    if s.mod_active(ci, C2_ICD) {
                        return;
                    }
                    s.delete_mod(ci, C2_HEX_POST_BURST);
                    s.add_status(ci, C2_ICD, 5 * 60, true);
                    do_ca(s, ci, 53);
                }
            }
            // C4 on lunar reactions
            Event::SpecialReactionAttack(ev) => {
                if !s.enemy_status_active(BUBBLE) && !s.enemy_status_active(OMEN) {
                    return;
                }
                s.special_atk.stats[st::CR] += 0.15;
                if s.chars[ci].hexerei && s.chars[ev.info.actor as usize].hexerei {
                    s.special_atk.stats[st::CD] += 0.15;
                }
            }
            // Hexerei C6: her charged attack's Mult doubled on a marked enemy
            Event::EnemyHit(ev) => {
                if ev.info.actor as usize == ci && ev.info.tag == AttackTag::Extra && (s.enemy_status_active(OMEN) || s.enemy_status_active(BUBBLE)) {
                    s.hit_mult *= 2.0;
                }
            }
            _ => {}
        }
    }
}

/// Vaporize (gcsim's Amped with AmpType vaporize): hydro on pyro (x2), or
/// pyro on hydro (x1.5).
fn is_vaporize(info: &AttackInfo) -> bool {
    info.amped && (info.element == Element::Hydro || (info.element == Element::Pyro && info.amp_mult == 1.5))
}

fn do_ca(s: &mut Sim, ci: usize, delay: i32) {
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::CHARGE[lvl]);
    ai.abil = "Charge Attack";
    ai.pattern = s.circle_on_target(3.0);
    let cons = s.chars[ci].cons;
    let mut cbs = CB_ASTRAL_GLOW;
    if cons >= 6 {
        cbs |= CB_C6_RESET;
    }
    if cons >= 2 && s.chars[ci].hexerei {
        cbs |= CB_C2_HEX;
    }
    s.queue_attack(ai, cbs, delay, delay);
}

fn a4_update(s: &mut Sim, ci: usize) {
    let er = s.stat_ex(ci, st::ER, true);
    s.st_mut::<State>(ci).a4_hydro = 0.2 * er;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, 60);
}

/// A1: every 2 s while she dashes, a phantom that explodes 2 s later with
/// an enemy within 15.
fn a1(s: &mut Sim, ci: usize) {
    if s.chars[ci].ascension < 1 || s.player.active as usize != ci || s.current_state() != AnimState::Dash {
        return;
    }
    if s.enemy_alive && s.enemy_within(s.circle_on_player(0.0, 0.0, 15.0)) {
        s.add_task(Task::Char { c: ci as u8, id: TASK_A1_EXPLODE, arg: 0 }, 120);
    }
    s.add_task(Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, 120);
}

/// The bubble breaks: Omen, and the explosion a frame later.
fn bubble_burst(s: &mut Sim, ci: usize) {
    s.enemy.mods.retain(|m| m.key != BUBBLE);
    let lvl = s.talent(ci, 2);
    let dur = (g::OMEN_DURATION[lvl] * 60.0) as i32;
    s.enemy_add_status(OMEN, dur, true);
    if s.chars[ci].hexerei {
        s.st_mut::<State>(ci).omen_refresh = g::OMEN_REFRESH_CAP;
    }
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Hydro, 50.0, g::EXPLOSION[lvl]);
    ai.abil = "Illusory Bubble (Explosion)";
    ai.pattern = Pattern::Single;
    s.queue_attack(ai, 0, 1, 1);
}

/// Hexerei: her normal or charged hit on Omen adds 2 s to it, four times,
/// once in 0.5 s.
fn omen_refresh(s: &mut Sim, ci: usize) {
    if !s.chars[ci].hexerei || s.hexerei_count() < 2 || !s.enemy_status_active(OMEN) {
        return;
    }
    let left = s.st::<State>(ci).omen_refresh;
    if left <= 0 || s.enemy_status_active(OMEN_REFRESH_ICD) {
        return;
    }
    s.st_mut::<State>(ci).omen_refresh = left - 1;
    s.enemy_add_status(OMEN_REFRESH_ICD, 30, true);
    let exp = s.enemy.mods.iter().find(|m| m.key == OMEN).map_or(0, |m| m.expiry());
    let dur = exp - s.f + 2 * 60;
    s.enemy_add_status(OMEN, dur, true);
}

/// Hexerei with a second Hexerei member: a hit gives a stack of Astral
/// Glow (3 at most) for 8 s, once in 0.1 s.
fn astral_glow_gain(s: &mut Sim, ci: usize) {
    if !s.chars[ci].hexerei || s.hexerei_count() < 2 || s.mod_active(ci, ASTRAL_GLOW_ICD) {
        return;
    }
    s.add_status(ci, ASTRAL_GLOW_ICD, 6, false);
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    if st.astral_glow.get() < 3 {
        st.astral_glow.set(st.astral_glow.get() + 1);
    }
    st.astral_glow_src = f;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ASTRAL_GLOW_END, arg: f }, 8 * 60);
    s.add_status(ci, ASTRAL_GLOW, 8 * 60, true);
}

fn c6_start_ticker(s: &mut Sim, ci: usize) {
    if s.st::<State>(ci).c6_src >= 0 {
        return;
    }
    let f = s.f;
    s.st_mut::<State>(ci).c6_src = f;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6_TICK, arg: f }, 59);
}

/// C6: each second of her dash (or, Hexerei, with Omen nearby) a stack of
/// +60% charged attack DMG, 3 at most, for 8 s.
fn c6_tick(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).c6_src != src {
        return;
    }
    let dashing = s.player.active as usize == ci && s.current_state() == AnimState::Dash;
    if !dashing && !s.st::<State>(ci).c6_nearby_omen {
        s.st_mut::<State>(ci).c6_src = -1;
        return;
    }
    if !s.mod_active(ci, C6) {
        s.st_mut::<State>(ci).c6_stacks = 0;
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6_TICK, arg: src }, 60);
    if s.player.active as usize != ci {
        return;
    }
    let state = s.st_mut::<State>(ci);
    state.c6_stacks = (state.c6_stacks + 1).min(3);
    s.add_mod(ci, Mod::owned(C6, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 8 * 60);
}

/// Hexerei C6: every 0.3 s, Omen or the bubble on an enemy within 15
/// keeps the ticker going.
fn c6_omen_ticker(s: &mut Sim, ci: usize) {
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6_OMEN, arg: 0 }, 18);
    let nearby = s.enemy_alive && (s.enemy_status_active(OMEN) || s.enemy_status_active(BUBBLE)) && s.enemy_within(s.circle_on_player(0.0, 0.0, 15.0));
    if nearby {
        s.st_mut::<State>(ci).c6_nearby_omen = true;
        c6_start_ticker(s, ci);
        return;
    }
    s.st_mut::<State>(ci).c6_nearby_omen = false;
    if s.player.active as usize != ci || s.current_state() != AnimState::Dash {
        s.st_mut::<State>(ci).c6_src = -1;
    }
}
