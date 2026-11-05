//! Dahlia, from gcsim's internal/characters/dahlia: normal attacks, the
//! charged attack, Immersive Ordinance (tap and `short_hold`), Radiant
//! Psalter with its Favonian Favor, Benison stacks and Shield of Sacred
//! Favor, A1, A4 and every constellation.
//!
//! The normal attacks' hits are his own tasks (gcsim's QueueCharTask, so
//! hitlag on him delays them), each snapshot when it lands. His charged
//! attack is tagged as a normal attack, as gcsim's is, so its hits count
//! for Benison stacks.
//!
//! A4's ATK SPD is a mod on the members swapped in during Favonian Favor
//! (not on the one active when it starts, as in gcsim): it applies only
//! in the normal attack state, so a normal attack's frames, which read
//! ATK SPD as the action starts, see it after another normal attack and
//! not out of idle or a skill, and Skirk's, read live, always. Its value
//! is recomputed from his max HP every 0.5 s on the active member's
//! queue, from the burst and from each swap, as gcsim's chain of tasks
//! does.
//!
//! C6: the first drain under the Favor starts the 15 minute cooldown,
//! whether or not the active member fell (and is revived to full HP), as
//! gcsim's does.

use crate::content::{Character, Event};
use crate::gamedata::chars::dahlia as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// Benison stacks he may still gain this burst.
    gen_limit: i32,
    /// Benison stacks held.
    stacks: i32,
    /// Normal attack hits counted towards the next stack.
    na_count: i32,
    /// A4's ATK SPD (gcsim's attackSpeedBuff).
    atkspd: f64,
    /// gcsim's c.shield: the shield he made last, until removeShield.
    shield: bool,
}

const FAVOR: ModKey = ModKey::new(g::BURST_FAVONIAN_FAVOR);
const NA_ICD: ModKey = ModKey::new(g::NORMAL_ATTACK_STACK_ICD);
const A1_ICD: ModKey = ModKey::new(g::BENISON_STACK_GENERATION_ICD);
const ATK_SPD: ModKey = ModKey::new(g::ATTACK_SPEED_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD);
const SHIELD: ModKey = ModKey::new("Radiant Psalter (Shield)");

const CB_PARTICLE: u16 = 1 << 0;

/// A normal attack's hit: arg is normal counter * 4 + hit.
const TASK_NORMAL: u8 = 0;
/// The Favor starts, 31 frames after the burst.
const TASK_FAVOR: u8 = 1;
/// The Favor's end removes the shield (his queue).
const TASK_REMOVE_SHIELD: u8 = 2;
/// A4's update on the queue of the member in slot arg.
const TASK_SPEED: u8 = 3;
/// A shield from a Benison stack.
const TASK_REGEN: u8 = 4;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];
const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];

impl Character for Char {
    const KEY: &'static str = "dahlia";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // setupBurst, a1, a4, c6, in gcsim's order
        s.subscribe(EventKind::EnemyHit, o);
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::Reaction, o);
        }
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::Swap, o);
        }
        if s.chars[ci].cons >= 6 {
            s.subscribe(EventKind::Drain, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Burst => None,
            Action::Skill => p.0.iter().find(|(k, _)| k != "short_hold").map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let marks = g::ATTACK_HITMARKS[n];
        for (i, &h) in marks.iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: (n * 4 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *marks.last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        for (i, &h) in g::CHARGE_HITMARKS.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[i][lvl]);
            ai.abil = CHARGE_NAMES[i];
            ai.pattern = s.circle_on_player(0.0, 0.0, 2.2);
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], *g::CHARGE_HITMARKS.last().unwrap(), AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = if p.get("short_hold").unwrap_or(0) != 0 { 1 } else { 0 };
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::SKILL[lvl]);
        ai.abil = "Immersive Ordinance";
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
        let h = g::SKILL_HITMARKS[hold];
        s.queue_attack(ai, CB_PARTICLE, h, h);
        s.set_cd_with_delay(ci, Action::Skill, 9 * 60, g::SKILL_CD_STARTS[hold]);
        let t = &g::SKILL_FRAMES[hold];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let st = s.st_mut::<State>(ci);
        st.gen_limit = g::BENISON_MAX_GENERATE;
        st.stacks = 0;
        st.na_count = 0;
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 50.0, g::BURST[lvl]);
        ai.abil = "Radiant Psalter";
        ai.pattern = s.circle_on_player(0.0, 0.8, 5.5);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.set_cd_with_delay(ci, Action::Burst, 15 * 60, g::BURST_CD_START);
        s.consume_energy(ci, g::BURST_ENERGY_DRAIN);
        s.add_task(Task::Char { c: ci as u8, id: TASK_FAVOR, arg: 0 }, g::BURST_SHIELD_INITIAL);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = ((arg / 4) as usize, (arg % 4) as usize);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.def_halt = true;
                ai.pattern = s.circle_on_player(0.0, g::ATTACK_OFFSETS[n][i], g::ATTACK_HITBOXES[n][i]);
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_FAVOR => {
                if s.chars[ci].ascension < 4 {
                    // gcsim writes A4's buff, made only at A4, and panics
                    s.unsupported("dahlia's burst below ascension 4");
                    return;
                }
                let dur = 12 * 60 + if s.chars[ci].cons >= 4 { 3 * 60 } else { 0 };
                s.add_status(ci, FAVOR, dur, true);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_REMOVE_SHIELD, arg: 0 }, dur);
                gen_shield(s, ci);
                let active = s.player.active as usize;
                update_speed(s, ci, active);
            }
            TASK_REMOVE_SHIELD => {
                if has_shield(s) && s.st::<State>(ci).shield {
                    let f = s.f;
                    s.set_shield_expiry(ShieldType::DahliaBurst, f + 1);
                    s.st_mut::<State>(ci).shield = false;
                }
            }
            TASK_SPEED => update_speed(s, ci, arg as usize),
            TASK_REGEN => {
                s.st_mut::<State>(ci).stacks -= 1;
                gen_shield(s, ci);
                if s.chars[ci].cons >= 2 {
                    add_c2(s, ci);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && ev.on_enemy() && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 30, true);
            s.queue_particle(3.0, Element::Hydro, s.particle_delay[ci]);
        }
    }

    /// A4's ATK SPD, under the Favor and in the normal attack state.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if !s.mod_active(ci, FAVOR) || s.current_state() != AnimState::Normal {
            return false;
        }
        out[st::ATKSPD] += s.st::<State>(ci).atkspd;
        true
    }

    /// C2: +25% shield strength while his shield is up.
    fn shield_bonus(s: &Sim, _: usize, _: ModKey) -> (f64, bool) {
        (if has_shield(s) { 0.25 } else { 0.0 }, false)
    }

    /// His shield broke: another from a Benison stack.
    fn shield_damaged(s: &mut Sim, ci: usize, _: ModKey, _: f64, broken: bool) {
        if broken {
            try_regen(s, ci, g::BURST_SHIELD_REGENERATED);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyHit(ev) => {
                if !s.mod_active(ci, FAVOR) || ev.info.actor != s.player.active || ev.info.tag != AttackTag::Normal {
                    return;
                }
                if s.mod_active(ci, NA_ICD) || s.st::<State>(ci).gen_limit == 0 {
                    return;
                }
                let st = s.st_mut::<State>(ci);
                st.na_count += 1;
                if st.na_count == 4 {
                    add_benison(s, ci, 1);
                    s.st_mut::<State>(ci).na_count = 0;
                }
                s.add_status(ci, NA_ICD, 3, true);
            }
            Event::Reaction(Reaction::Frozen, ev) => {
                if !s.mod_active(ci, FAVOR) || !ev.on_enemy() {
                    return;
                }
                if !s.mod_active(ci, A1_ICD) {
                    s.add_status(ci, A1_ICD, 8 * 60, true);
                    add_benison(s, ci, 2);
                }
            }
            Event::Swap { prev, next } => {
                if !s.mod_active(ci, FAVOR) {
                    return;
                }
                for k in 0..s.chars.len() {
                    if k == prev && s.mod_active(k, ATK_SPD) {
                        s.delete_mod(k, ATK_SPD);
                    }
                    if k == next && !s.mod_active(k, ATK_SPD) {
                        s.add_mod(k, Mod::owned(ATK_SPD, ModKind::Stat, Owner::Char(ci as u8), st::ATKSPD, false, false, -1), -1);
                    }
                }
                update_speed(s, ci, next);
            }
            Event::Drain { amount, .. } => {
                if !s.mod_active(ci, FAVOR) || amount <= 0.0 || s.mod_active(ci, C6_ICD) {
                    return;
                }
                let active = s.player.active as usize;
                if s.chars[active].hp_ratio <= 0.0 {
                    let hp = s.max_hp(active);
                    s.set_hp(active, hp);
                }
                s.add_status(ci, C6_ICD, 15 * 60 * 60, true);
            }
            _ => {}
        }
    }
}

/// updateSpeedBuff: while the member in slot `src` is active, A4's value
/// from his max HP now (and C6's 10%), again in 0.5 s on its queue.
fn update_speed(s: &mut Sim, ci: usize, src: usize) {
    if s.player.active as usize != src {
        return;
    }
    let c6 = if s.chars[ci].cons >= 6 { 0.1 } else { 0.0 };
    let v = (s.max_hp(ci) * 0.001 * 0.005).min(0.2) + c6;
    s.st_mut::<State>(ci).atkspd = v;
    s.queue_char_task(src, Task::Char { c: ci as u8, id: TASK_SPEED, arg: src as i32 }, 30);
}

fn has_shield(s: &Sim) -> bool {
    s.shields.iter().any(|x| x.typ == ShieldType::DahliaBurst)
}

fn gen_shield(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 2);
    let hp = g::BURST_SHIELD_PP[lvl] * s.max_hp(ci) + g::BURST_SHIELD_FLAT[lvl];
    s.st_mut::<State>(ci).shield = true;
    s.add_shield(Shield { owner: ci as u8, key: SHIELD, typ: ShieldType::DahliaBurst, target: -1, ele: Element::Hydro, hp, expiry: -1, em_bonus: 0.0 });
}

/// tryRegenShield: with a stack and the Favor lasting `delay` more, a
/// shield `delay` frames later.
fn try_regen(s: &mut Sim, ci: usize, delay: i32) {
    if s.st::<State>(ci).stacks <= 0 {
        return;
    }
    if s.mod_duration(ci, g::BURST_FAVONIAN_FAVOR) < delay {
        return;
    }
    s.add_task(Task::Char { c: ci as u8, id: TASK_REGEN, arg: 0 }, delay);
}

/// addBenisonStack, with C1's energy.
fn add_benison(s: &mut Sim, ci: usize, stacks: i32) {
    let st = s.st_mut::<State>(ci);
    if st.gen_limit <= 0 {
        return;
    }
    let stacks = stacks.min(st.gen_limit);
    st.gen_limit -= stacks;
    st.stacks += stacks;
    if s.chars[ci].cons >= 1 {
        s.add_energy(ci, 2.5 * stacks as f64);
    }
    if !has_shield(s) {
        try_regen(s, ci, g::BURST_SHIELD_AFTER_BENISON_STACK);
    }
}

/// C2's shield bonus mod for 12 s (AddShieldBonusMod: replaces its key).
fn add_c2(s: &mut Sim, ci: usize) {
    let expiry = s.f + 12 * 60;
    let m = ShieldBonusMod { key: C2, owner: Owner::Char(ci as u8), expiry };
    match s.shield_bonus_mods.iter().position(|m| m.key == C2) {
        Some(i) => s.shield_bonus_mods[i] = m,
        None => s.shield_bonus_mods.push(m),
    }
}
