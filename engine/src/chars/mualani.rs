//! Mualani (gcsim's internal/characters/mualani).
//!
//! Her Nightsoul's Blessing (60 points, one less every 6 frames) turns her
//! normal attack into Sharky's Bite, on a cooldown of its own (one charge,
//! 1.8 s). While she surfs (dashes, jumps or walks in the blessing) gcsim's
//! OnTick hits the enemy with a box ahead of her once per 0.7 s, which marks
//! it as prey and gives a Momentum stack; three make the Surging Bite.

use crate::content::{Character, Event};
use crate::gamedata::chars::mualani as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    nightsoul_src: i32,
    momentum: i32,
    a4_stacks: i32,
    c1_done: bool,
    a1_count: i32,
    burst: Option<AttackEvent>,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const MOMENTUM_ICD: ModKey = ModKey::new(g::MOMENTUM_ICD_KEY);
const SURFING: ModKey = ModKey::new(g::SURFING_KEY);
const MARKED_AS_PREY: ModKey = ModKey::new(g::MARKED_AS_PREY_KEY);
const C4: ModKey = ModKey::new(g::C4KEY);

// attack callbacks
const CB_PARTICLE: u16 = 1 << 0;
const CB_REMOVE_MARK: u16 = 1 << 1;
const CB_A1: u16 = 1 << 2;
const CB_SURFING: u16 = 1 << 3;

// character tasks
/// The skill's blessing, skillDelay after the press.
const TASK_ENTER: u8 = 0;
/// Surfing starts and points begin to drain (arg: nightsoulSrc).
const TASK_SURF: u8 = 1;
/// One point every 6 frames (arg: nightsoulSrc).
const TASK_REDUCE: u8 = 2;
/// Sharky's Bite lands (arg: the momentum stacks it was used with).
const TASK_BITE: u8 = 3;
/// A1's points, 20 frames after the bite.
const TASK_A1: u8 = 4;
/// C2's point every 10 frames after the second A1.
const TASK_C2_POINT: u8 = 5;
/// The burst's bullet is made: its snapshot (arg: A4 stacks).
const TASK_BURST_SNAP: u8 = 6;
/// The burst lands, `travel` later (fight queue; arg: A4 stacks).
const TASK_BURST_HIT: u8 = 7;

// frames
const FR_SKILL: u8 = 0;
const FR_CHARGE: u8 = 1;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];
const BITE_NAMES: [&str; 3] = ["Sharky's Bite (0 momentum)", "Sharky's Bite (1 momentum)", "Sharky's Bite (2 momentum)"];

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn char_task(s: &mut Sim, ci: usize, id: u8, arg: i32, delay: i32) {
    s.queue_char_task(ci, Task::Char { c: ci as u8, id, arg }, delay);
}

impl Character for Char {
    const KEY: &'static str = "mualani";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { nightsoul_src: -1, momentum: 0, a4_stacks: 0, c1_done: false, a1_count: 0, burst: None }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.chars[ci].ns.max_points = 60.0;
        let o = Owner::Char(ci as u8);
        // A4
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::NightsoulBurst, o);
        }
        // C4: burst DMG +75%
        if s.chars[ci].cons >= 4 {
            s.chars[ci].mods.push(Mod::owned(C4, ModKind::Attack, o, st::NONE, false, true, -1));
        }
        // onExitField
        s.subscribe(EventKind::Swap, o);
        // surfingTick
        s.tick_subs.push(ci as u8);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let allowed: &[&str] = match a {
            Action::Attack | Action::Burst => &["travel"],
            Action::Charge | Action::Skill => &[],
            _ => return Some(name.into()),
        };
        p.0.iter().find(|(k, _)| !allowed.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
    }

    /// No charged attack in the blessing (gcsim's ErrInvalidChargeAction).
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::Charge && s.ns_has_blessing(ci) {
            return Some("need to use attack right before charge".into());
        }
        crate::content::template_invalid(s, a)
    }

    /// Sharky's Bite waits for its charge, as character_deceased.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        if a == Action::Attack && s.ns_has_blessing(ci) && s.chars[ci].cd[crate::sim::Char::cd_index(Action::Attack)].available <= 0 {
            return Some(Some(Failure::CharacterDeceased));
        }
        None
    }

    fn action_stam(s: &Sim, ci: usize, _: Action) -> Option<f64> {
        s.ns_has_blessing(ci).then_some(0.0)
    }

    /// AnimationStartDelay: Yelan's (1) and Xingqiu's (2) waves.
    fn start_delay(s: &Sim, ci: usize, key: usize) -> Option<i32> {
        Some(if s.ns_has_blessing(ci) {
            match (st(s, ci).momentum >= 3, key == 2) {
                (true, true) => 44,
                (true, false) => 37,
                (false, true) => 11,
                (false, false) => 9,
            }
        } else {
            11
        })
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.ns_has_blessing(ci) {
            return shark_bite(s, ci, p);
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.circle_on_target(0.7);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], g::ATTACK_CQA[n], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let windup = match s.current_state() {
            AnimState::Idle | AnimState::Dash | AnimState::Jump | AnimState::Walk | AnimState::Swap => 13,
            _ => 0,
        };
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.pattern = s.circle_on_player(0.0, 1.0, 3.5);
        let h = g::SHORT_CHARGE_HITMARK + windup;
        s.queue_attack(ai, 0, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: FR_CHARGE, arg: windup as i16 }, windup + t[0], windup + t[Action::Swap as usize], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.ns_has_blessing(ci) {
            cancel_nightsoul(s, ci);
            let t = &g::SKILL_CANCEL_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Burst as usize], AnimState::Skill);
        }
        char_task(s, ci, TASK_ENTER, 0, g::SKILL_DELAY);
        // pressed "while" walking
        let walking = s.current_state() == AnimState::Walk;
        let t = &g::SKILL_FRAMES;
        let cqa = if walking { g::SKILL_DELAY } else { t[Action::Attack as usize] };
        ActInfo::new(Frames::Char { id: FR_SKILL, arg: walking as i16 }, t[0], cqa, AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(70, |t| t as i32);
        let stacks = st(s, ci).a4_stacks;
        st_mut(s, ci).a4_stacks = 0;
        // the area is the enemy's as the burst is used; the enemy does not
        // move, so it is made with the attack
        char_task(s, ci, TASK_BURST_SNAP, stacks * 1024 + travel, g::BURST_HITMARKS);
        s.set_cd_with_delay(ci, Action::Burst, 15 * 60, 0);
        s.consume_energy(ci, 11);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    /// The dash in the blessing uses 10 points and replaces the template's.
    fn own_dash(s: &mut Sim, ci: usize, _: &Params) -> Option<ActInfo> {
        if !s.ns_has_blessing(ci) {
            return None;
        }
        reduce_points(s, ci, 10.0);
        let t = &g::SKILL_DASH_FRAMES;
        Some(ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Dash))
    }

    /// The jump in the blessing uses 2 points, 14 out of a dash.
    fn own_jump(s: &mut Sim, ci: usize, _: &Params) -> Option<ActInfo> {
        if !s.ns_has_blessing(ci) {
            return None;
        }
        let v = if s.player.last.action == Action::Dash { 14.0 } else { 2.0 };
        reduce_points(s, ci, v);
        let t = &g::SKILL_JUMP_FRAMES;
        Some(ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Jump))
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_SKILL => {
                if next == Action::Walk && arg != 0 {
                    g::SKILL_DELAY
                } else {
                    g::SKILL_FRAMES[next as usize]
                }
            }
            _ => arg as i32 + g::CHARGE_FRAMES[next as usize],
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_ENTER => {
                s.ns_enter(ci, 60.0);
                s.delete_mod(ci, PARTICLE_ICD);
                let st = st_mut(s, ci);
                st.a1_count = 0;
                st.c1_done = false;
                // C2: two Momentum stacks
                if s.chars[ci].cons >= 2 {
                    st_mut(s, ci).momentum = 2;
                }
                let f = s.f;
                st_mut(s, ci).nightsoul_src = f;
                char_task(s, ci, TASK_SURF, f, 6);
            }
            TASK_SURF => {
                s.add_status(ci, SURFING, -1, false);
                reduce_tick(s, ci, arg);
            }
            TASK_REDUCE => reduce_tick(s, ci, arg),
            TASK_BITE => bite(s, ci, arg),
            TASK_A1 => {
                if s.ns_has_blessing(ci) {
                    s.ns_generate(ci, 20.0);
                    c2_puffer(s, ci);
                    // C4: 8 energy
                    if s.chars[ci].cons >= 4 && s.chars[ci].ascension >= 1 {
                        s.add_energy(ci, 8.0);
                    }
                }
            }
            TASK_C2_POINT => s.ns_generate(ci, 1.0),
            TASK_BURST_SNAP => {
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
                ai.abil = "Boomsharka-laka";
                ai.add_tags = add_tag::NIGHTSOUL;
                ai.pattern = s.circle_on_target(5.0);
                let ev = s.snapshot_event(ai, 0);
                st_mut(s, ci).burst = Some(ev);
                let (stacks, travel) = (arg / 1024, arg % 1024);
                s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_HIT, arg: stacks }, travel);
            }
            TASK_BURST_HIT => {
                let Some(mut ev) = st_mut(s, ci).burst.take() else { return };
                // the max HP as it lands, not the snapshot's
                let hp = s.max_hp(ci);
                ev.info.flat_dmg = g::BURST[s.talent(ci, 2)] * hp;
                if s.chars[ci].ascension >= 4 {
                    ev.info.flat_dmg += 0.15 * arg as f64 * hp;
                }
                s.queue_attack_with_snap(ev, 0, 0);
            }
            _ => {}
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_SURFING != 0 {
            if s.enemy_status_active(MOMENTUM_ICD) {
                return;
            }
            s.enemy_add_status(MARKED_AS_PREY, g::MARKED_AS_PREY_DUR, true);
            s.enemy_add_status(MOMENTUM_ICD, g::MOMENTUM_ICD as i32, false);
            let st = st_mut(s, ci);
            st.momentum = (st.momentum + 1).min(3);
            return;
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD, true);
            let count = if s.rng.float64() < 0.5 { 5.0 } else { 4.0 };
            s.queue_particle(count, Element::Hydro, s.particle_delay[ci]);
        }
        if cbs & CB_REMOVE_MARK != 0 {
            s.enemy_delete_status(MARKED_AS_PREY);
        }
        if cbs & CB_A1 != 0 && st(s, ci).a1_count < 2 {
            st_mut(s, ci).a1_count += 1;
            char_task(s, ci, TASK_A1, 0, g::A1_DELAY);
        }
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C4 && ev.info.tag == AttackTag::ElementalBurst {
            ev.stats[st::DMGP] += 0.75;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // onExitField: on any swap
            Event::Swap { .. } => {
                if s.ns_has_blessing(ci) {
                    cancel_nightsoul(s, ci);
                }
            }
            Event::NightsoulBurst(_) => {
                let st = st_mut(s, ci);
                st.a4_stacks = (st.a4_stacks + 1).min(3);
            }
            _ => {}
        }
    }

    /// surfingTick: the Surfing Hit, while she dashes, jumps or walks on
    /// the field in the blessing, on an enemy its box reaches that has no
    /// Momentum ICD.
    fn on_tick(s: &mut Sim, ci: usize) {
        if s.player.active as usize != ci || !s.ns_has_blessing(ci) || !s.mod_active(ci, SURFING) {
            return;
        }
        if !matches!(s.current_state(), AnimState::Dash | AnimState::Jump | AnimState::Walk) {
            return;
        }
        let p = s.box_on_player(0.0, 0.9, 0.0, 1.0);
        let lands = match s.hits_enemy(p) {
            Some(l) => l,
            None => {
                if s.error.is_none() {
                    s.error = Some("unsupported: hit pattern (mualani surfing hit)".into());
                }
                return;
            }
        };
        if !lands || s.enemy_status_active(MOMENTUM_ICD) {
            return;
        }
        let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, 0.0);
        ai.abil = "Surfing Hit";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = p;
        s.queue_attack(ai, CB_SURFING, 0, 0);
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "nightsoul" => Some(s.ns_condition(ci, fields)),
            "momentum" => Some(Ok(Num::Int(st(s, ci).momentum as i64))),
            _ => None,
        }
    }
}

/// nightsoulPointReduceFunc: a point every 6 frames while the blessing of
/// that skill lasts.
fn reduce_tick(s: &mut Sim, ci: usize, src: i32) {
    if st(s, ci).nightsoul_src != src || !s.ns_has_blessing(ci) {
        return;
    }
    reduce_points(s, ci, 1.0);
    char_task(s, ci, TASK_REDUCE, src, 6);
}

/// reduceNightsoulPoints.
fn reduce_points(s: &mut Sim, ci: usize, v: f64) {
    s.ns_consume(ci, v);
    if s.ns_points(ci) <= 0.00001 {
        cancel_nightsoul(s, ci);
    }
}

/// cancelNightsoul.
fn cancel_nightsoul(s: &mut Sim, ci: usize) {
    s.ns_exit(ci);
    s.set_cd(ci, Action::Skill, 6 * 60);
    s.reset_cd(ci, Action::Attack);
    s.delete_mod(ci, SURFING);
    let st = st_mut(s, ci);
    st.momentum = 0;
    st.nightsoul_src = -1;
}

/// sharkBite: the bite lands at its hitmark with the stacks it was used
/// with.
fn shark_bite(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
    s.chars[ci].normal_counter = 0;
    let m = st(s, ci).momentum.min(3) as usize;
    char_task(s, ci, TASK_BITE, m as i32, g::SHARK_BITE_HITMARKS[m]);
    let t = &g::SHARK_BITE_FRAMES[m];
    let min = if m >= 3 { Action::Dash } else { Action::Walk };
    ActInfo::new(Frames::Table(t), t[0], t[min as usize], AnimState::Normal)
}

fn bite(s: &mut Sim, ci: usize, stacks: i32) {
    st_mut(s, ci).momentum = 0;
    let lvl = s.talent(ci, 1);
    let mut mult = g::BITE[lvl] + g::MOMENTUM_BONUS[lvl] * stacks as f64 + c1(s, ci);
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
    ai.add_tags = add_tag::NIGHTSOUL;
    if stacks >= 3 {
        ai.abil = "Sharky's Surging Bite";
        mult += g::SURGING_BITE[lvl];
    } else {
        ai.abil = BITE_NAMES[stacks as usize];
    }
    // the Shark Missiles go to other marked enemies within 5 of the
    // primary target; the engine's fight has one enemy, so there are none
    // and the bite keeps its whole multiplier
    ai.flat_dmg = mult * s.max_hp(ci);
    ai.pattern = Pattern::Single;
    let mut cbs = CB_PARTICLE | CB_REMOVE_MARK;
    if s.chars[ci].ascension >= 1 {
        cbs |= CB_A1;
    }
    s.queue_attack(ai, cbs, 0, 0);
    s.set_cd_with_delay(ci, Action::Attack, 108, 0);
}

/// C1: the first bite of each blessing +66% of max HP (every bite at C6).
fn c1(s: &mut Sim, ci: usize) -> f64 {
    if s.chars[ci].cons < 1 || st(s, ci).c1_done {
        return 0.0;
    }
    st_mut(s, ci).c1_done = s.chars[ci].cons < 6;
    0.66
}

/// c2puffer: a Momentum stack, and after the second A1 a point every 10
/// frames for 12 times.
fn c2_puffer(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 2 || s.chars[ci].ascension < 1 {
        return;
    }
    let st = st_mut(s, ci);
    st.momentum = (st.momentum + 1).min(3);
    if st.a1_count == 2 {
        for i in 0..12 {
            char_task(s, ci, TASK_C2_POINT, 0, i * 10 + 10);
        }
    }
}
