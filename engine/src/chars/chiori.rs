//! Chiori, from gcsim's internal/characters/chiori: normal and charged
//! attacks, the skill pressed or held with its Tamato and the second doll
//! a construct brings, its recast (A1's Tapestry, which swaps to the next
//! character) and A1's Tailoring, the burst, A4. No plunges or
//! constellations.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::chiori as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

/// One of gcsim's tickers: alive until killed or its life runs out. A new
/// one always kills the old first, so a task of an older generation is a
/// task of a dead ticker.
#[derive(Default, Clone, Copy)]
struct Ticker {
    alive: bool,
    generation: i32,
    /// The frame a doll was made (its src).
    src: i32,
}

#[derive(Default)]
pub struct State {
    doll: Ticker,
    rock_doll: Ticker,
    checker: Ticker,
    a1_triggered: bool,
    a1_attack_count: i32,
}

const A1_WINDOW: ModKey = ModKey::new(g::A1_WINDOW_KEY);
const SEIZE: ModKey = ModKey::new(g::A1_SEIZE_THE_MOMENT_KEY);
const SEIZE_ICD: ModKey = ModKey::new(g::A1_SEIZE_THE_MOMENT_ICD_KEY);
const TAILORING: ModKey = ModKey::new(g::A1_GEO_INFUSION_KEY);
const A4: ModKey = ModKey::new(g::A4_BUFF_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;

// tickers, in a task's arg as which * 2^24 + generation
const DOLL: i32 = 0;
const ROCK_DOLL: i32 = 1;
const CHECKER: i32 = 2;

const TASK_SWEEP: u8 = 0;
const TASK_DOLLS: u8 = 1;
const TASK_KILL: u8 = 2;
const TASK_TICK: u8 = 3;
// arg: the doll (DOLL or ROCK_DOLL)
const TASK_DOLL_ATTACK: u8 = 4;
const TASK_A1_WINDOW: u8 = 5;
const TASK_A1_WINDOW_END: u8 = 6;
const TASK_SWAP: u8 = 7;
const TASK_BURST: u8 = 8;

impl Character for Char {
    const KEY: &'static str = "chiori";
    const MAX_CONS: i32 = 0;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::EnemyDamage, o);
        }
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::ConstructSpawned, o);
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// The recast is ready while A1's window is open.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && s.mod_active(ci, A1_WINDOW)).then_some(None)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = g::ATTACK_HITLAG_FACTOR[n][i];
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let off = g::ATTACK_OFFSETS[n];
            let hb = g::ATTACK_HITBOXES[n];
            ai.pattern = if n == 2 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
            // snapshot at the hitmark
            let h = g::ATTACK_HITMARKS[n][i];
            s.queue_attack(ai, 0, h, h);
        }
        // A1's Tailoring, from a normal attack in its window
        if s.chars[ci].ascension >= 1 && s.mod_active(ci, A1_WINDOW) {
            a1_tailoring(s, ci);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::CHARGE.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = ["Charge 0", "Charge 1"][i];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::CHARGE_HITLAG_HALT_FRAME[i] * 60.0;
            ai.def_halt = g::CHARGE_DEF_HALT[i];
            ai.pattern = s.circle_on_player(0.0, g::CHARGE_OFFSETS[i], g::CHARGE_RADIUS[i]);
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], *g::CHARGE_HITMARKS.last().unwrap(), AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(0).clamp(0, 1) as usize;
        if s.mod_active(ci, A1_WINDOW) {
            // the recast: A1's Tapestry, then a swap to the next character
            a1_trigger(s, ci);
            s.add_status(ci, SEIZE, g::A1_SEIZE_THE_MOMENT_DURATION, true);
            s.st_mut::<State>(ci).a1_attack_count = 0;
            s.add_task(Task::Char { c: ci as u8, id: TASK_SWAP, arg: 0 }, 1);
            let d = s.swap_delay;
            return ActInfo::new(Frames::Const(d), d, d, AnimState::Skill);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_SWEEP, arg: 0 }, g::SKILL_HITMARKS[hold]);
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD, g::SKILL_CD_STARTS[hold]);
        if s.chars[ci].ascension >= 1 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1_WINDOW, arg: g::SKILL_A1_WINDOW_DURATIONS[hold] }, g::SKILL_A1_WINDOW_STARTS[hold]);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_DOLLS, arg: 0 }, g::SKILL_CD_STARTS[hold]);
        let t = &g::SKILL_FRAMES[hold];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Skill as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST, arg: 0 }, g::BURST_SNAPSHOT_TIMING);
        s.consume_energy(ci, g::BURST_ENERGY_FRAME);
        s.set_cd(ci, Action::Burst, (g::BURST_CD * 60.0) as i32);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SWEEP => {
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_CHIORI_SKILL, icd::GROUP_CHIORI_SKILL, Element::Geo, 25.0, g::THRUST_ATK_SCALING[lvl]);
                ai.abil = "Fluttering Hasode (Upward Sweep)";
                ai.blunt = true;
                ai.poise = 50.0;
                ai.pattern = s.box_on_player(0.0, 0.0, 5.5, 3.5);
                let mut ev = s.snapshot_event(ai, 0);
                ev.info.flat_dmg = total_def(&ev.stats) * g::THRUST_DEF_SCALING[lvl];
                s.queue_attack_with_snap(ev, 0, 0);
            }
            // the doll, and the rock doll as soon as there is a construct
            TASK_DOLLS => {
                make_ticker(s, ci, DOLL, g::SKILL_DOLL_DURATION);
                s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg: arg_of(DOLL, s.st::<State>(ci).doll.generation) }, g::SKILL_DOLL_START_DELAY);
                make_ticker(s, ci, CHECKER, g::SKILL_DOLL_DURATION);
                tick(s, ci, CHECKER);
            }
            TASK_KILL => {
                let (which, generation) = (arg >> 24, arg & 0xFF_FFFF);
                let t = ticker(s, ci, which);
                if t.generation == generation && t.alive {
                    ticker_mut(s, ci, which).alive = false;
                }
            }
            TASK_TICK => {
                let (which, generation) = (arg >> 24, arg & 0xFF_FFFF);
                if ticker(s, ci, which).generation == generation {
                    tick(s, ci, which);
                }
            }
            TASK_DOLL_ATTACK => doll_attack(s, ci, arg),
            TASK_A1_WINDOW => {
                s.add_status(ci, A1_WINDOW, arg, true);
                s.st_mut::<State>(ci).a1_triggered = false;
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1_WINDOW_END, arg: 0 }, arg);
            }
            TASK_A1_WINDOW_END => {
                if !s.st::<State>(ci).a1_triggered {
                    a1_tailoring(s, ci);
                }
            }
            // gcsim's Player.Exec of a swap to the next character
            TASK_SWAP => {
                let next = (ci + 1) % s.chars.len();
                s.force_swap(next);
            }
            TASK_BURST => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 50.0, g::BURST_ATK_SCALING[lvl]);
                ai.abil = "Hiyoku: Twin Blades";
                ai.blunt = true;
                ai.poise = 200.0;
                ai.pattern = s.circle_on_player(0.0, 0.0, 12.0);
                let mut ev = s.snapshot_event(ai, 0);
                ev.info.flat_dmg = total_def(&ev.stats) * g::BURST_DEF_SCALING[lvl];
                s.queue_attack_with_snap(ev, 0, g::BURST_HITMARK - g::BURST_SNAPSHOT_TIMING);
            }
            _ => unreachable!(),
        }
    }

    fn mod_amount(_: &Sim, _: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        // A4
        out[st::GEOP] += 0.20;
        true
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE == 0 || s.mod_active(ci, PARTICLE_ICD) {
            return;
        }
        s.add_status(ci, PARTICLE_ICD, 3 * 60, true);
        let count = if s.rng.float64() < 0.2 { 2.0 } else { 1.0 };
        s.queue_particle(count, Element::Geo, s.particle_delay[ci]);
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // A4: a construct appearing
            Event::ConstructSpawned => apply_a4(s, ci),
            // A1's Seize the Moment: the active character's normal,
            // charged or plunging attack hits
            Event::EnemyDamage(ev) => {
                if !s.mod_active(ci, SEIZE) || s.mod_active(ci, SEIZE_ICD) {
                    return;
                }
                if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) || ev.info.actor != s.player.active {
                    return;
                }
                s.add_status(ci, SEIZE_ICD, g::A1_SEIZE_THE_MOMENT_ICD, true);
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_CHIORI_SKILL, icd::GROUP_CHIORI_SKILL, Element::Geo, 25.0, g::THRUST_ATK_SCALING[lvl]);
                ai.abil = "Fluttering Hasode (Seize the Moment)";
                ai.pattern = s.circle_on_target(2.5);
                let mut atk = s.snapshot_event(ai, 0);
                atk.info.flat_dmg = total_def(&atk.stats) * g::THRUST_DEF_SCALING[lvl];
                s.queue_attack_with_snap(atk, 0, 1);
                let st = s.st_mut::<State>(ci);
                st.a1_attack_count += 1;
                if st.a1_attack_count == g::A1_SEIZE_THE_MOMENT_ATTACK_LIMIT {
                    s.delete_mod(ci, SEIZE);
                }
            }
            _ => {}
        }
    }
}

fn arg_of(which: i32, generation: i32) -> i32 {
    which << 24 | generation
}

fn ticker(s: &Sim, ci: usize, which: i32) -> Ticker {
    let st = s.st::<State>(ci);
    match which {
        DOLL => st.doll,
        ROCK_DOLL => st.rock_doll,
        _ => st.checker,
    }
}

fn ticker_mut(s: &mut Sim, ci: usize, which: i32) -> &mut Ticker {
    let st = s.st_mut::<State>(ci);
    match which {
        DOLL => &mut st.doll,
        ROCK_DOLL => &mut st.rock_doll,
        _ => &mut st.checker,
    }
}

/// newTicker, after killing the old one: alive for `life` frames.
fn make_ticker(s: &mut Sim, ci: usize, which: i32, life: i32) {
    let f = s.f;
    let t = ticker_mut(s, ci, which);
    t.alive = true;
    t.generation = (t.generation + 1) & 0xFF_FFFF;
    t.src = f;
    let generation = t.generation;
    s.add_task(Task::Char { c: ci as u8, id: TASK_KILL, arg: arg_of(which, generation) }, life);
}

/// ticker.tick: the doll's attack 5 frames later, or the construct check;
/// then the next tick.
fn tick(s: &mut Sim, ci: usize, which: i32) {
    let t = ticker(s, ci, which);
    if !t.alive {
        return;
    }
    let interval = if which == CHECKER {
        construct_check(s, ci);
        // the check kills itself once it made the rock doll
        if !ticker(s, ci, CHECKER).alive {
            return;
        }
        g::SKILL_DOLL_CONSTRUCT_CHECK_INTERVAL
    } else {
        s.add_task(Task::Char { c: ci as u8, id: TASK_DOLL_ATTACK, arg: which }, g::SKILL_DOLL_ATTACK_DELAY);
        g::SKILL_DOLL_ATTACK_INTERVAL
    };
    s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg: arg_of(which, t.generation) }, interval);
}

/// skillDollConstructCheck: a construct brings the rock doll and A4.
fn construct_check(s: &mut Sim, ci: usize) {
    if ticker(s, ci, ROCK_DOLL).alive || s.constructs.is_empty() {
        return;
    }
    make_ticker(s, ci, ROCK_DOLL, g::SKILL_DOLL_DURATION);
    let generation = ticker(s, ci, ROCK_DOLL).generation;
    s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg: arg_of(ROCK_DOLL, generation) }, g::SKILL_ROCK_DOLL_START_DELAY);
    ticker_mut(s, ci, CHECKER).alive = false;
}

/// A doll's attack: a circle of 1.2 on the enemy, which is always within
/// its search range of 12.
fn doll_attack(s: &mut Sim, ci: usize, which: i32) {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_CHIORI_SKILL, icd::GROUP_CHIORI_SKILL, Element::Geo, 25.0, g::TURRET_ATK_SCALING[lvl]);
    ai.abil = if which == DOLL { "Fluttering Hasode (Tamato)" } else { "Fluttering Hasode (Tamato - Construct)" };
    ai.blunt = true;
    ai.pattern = s.circle_on_target(g::SKILL_DOLL_AO_E);
    let mut ev = s.snapshot_event(ai, CB_PARTICLE);
    ev.info.flat_dmg = total_def(&ev.stats) * g::TURRET_DEF_SCALING[lvl];
    s.queue_attack_with_snap(ev, CB_PARTICLE, 0);
}

/// commonA1Trigger (its C4 and C6 parts are not ported).
fn a1_trigger(s: &mut Sim, ci: usize) {
    s.st_mut::<State>(ci).a1_triggered = true;
    s.delete_mod(ci, A1_WINDOW);
}

/// A1's Tailoring: geo on Chiori's normal, charged and plunging attacks
/// for 5 s.
fn a1_tailoring(s: &mut Sim, ci: usize) {
    a1_trigger(s, ci);
    s.add_infusion(ci, TAILORING, Element::Geo, g::A1_GEO_INFUSION_DURATION, true, &[AttackTag::Normal, AttackTag::Extra, AttackTag::Plunge]);
}

fn apply_a4(s: &mut Sim, ci: usize) {
    s.add_mod(ci, Mod::owned(A4, ModKind::Stat, Owner::Char(ci as u8), st::GEOP, false, true, -1), g::A4_DURATION);
}
