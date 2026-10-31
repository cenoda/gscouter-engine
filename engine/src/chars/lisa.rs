//! Lisa, from gcsim's internal/characters/lisa: normal and charged
//! attacks, the pressed and held skill with the enemy's Conductive stacks
//! (the held skill's multiplier by the stacks it consumes), the burst's
//! Lightning Rose with C4's discharges on enemy-camp gadgets, A1, A4 (the
//! enemy's DEF −15%) and every constellation. No plunges.

use std::cell::Cell;

use crate::content::{Character, Event};
use crate::gcsimdata::chars::lisa as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The enemy's Conductive stacks (gcsim's tag on it). A cell, since the
    /// held skill consumes them as it hits.
    conductive: Cell<i32>,
    /// The burst's ticks, snapshot a frame before its hitmark.
    burst: AttackEvent,
    previous_discharge_count: i32,
    c6_icd: i32,
    /// The last skill was held: a charge from it skips its windup.
    last_skill_hold: bool,
    /// C1: the held skill's hits that gave energy, this cast.
    c1_count: i32,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A4: ModKey = ModKey::new("lisa-a4");
const C2: ModKey = ModKey::new("lisa-c2");
const BURST_STATUS: ModKey = ModKey::new("lisaburst");

const CB_PRESS: u16 = 1 << 0;
const CB_A1: u16 = 1 << 1;
const CB_A4: u16 = 1 << 2;
const CB_PARTICLE: u16 = 1 << 3;
const CB_C1: u16 = 1 << 4;

const TASK_BURST_SNAP: u8 = 0;
// arg: the tick's frame after the burst
const TASK_BURST_TICK: u8 = 1;
const TASK_BURST_STATUS: u8 = 2;
const TASK_C1: u8 = 3;

const FRAMES_CHARGE: u8 = 0;

const BURST_FIRST_TICK: i32 = 119;

/// The enemy's Conductive stacks, for gcsl's .tags.<target>.lisa-conductive-stacks.
pub fn conductive(s: &Sim) -> i64 {
    match s.chars.iter().position(|c| c.kind == CharKind::Lisa) {
        Some(ci) => s.st::<State>(ci).conductive.get() as i64,
        None => 0,
    }
}

fn add_stack(s: &Sim, ci: usize) {
    let c = &s.st::<State>(ci).conductive;
    if c.get() < 3 {
        c.set(c.get() + 1);
    }
}

/// Whether the enemy is within an area (gcsim's EnemiesWithinArea).
fn enemy_within(s: &Sim, p: Pattern) -> bool {
    p.hits_circle(0.0, s.target_dist, s.target_radius) == Some(true)
}

impl Character for Char {
    const KEY: &'static str = "lisa";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            conductive: Cell::new(0),
            burst: AttackEvent::new(AttackInfo::new(0, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Electro, 25.0, 0.0), 0),
            previous_discharge_count: 0,
            c6_icd: 0,
            last_skill_hold: false,
            c1_count: 0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].cons >= 6 {
            s.subscribe(EventKind::Swap, Owner::Char(ci as u8));
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// gcsim's NextQueueItemIsValid: no charge right after a normal attack
    /// that ended the combo.
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        let active = &s.chars[s.player.active as usize];
        (a == Action::Charge && s.player.last.action == Action::Attack && active.normal_counter == 0).then(|| "need to use attack right before charge".into())
    }

    /// The held skill's multiplier, by the stacks it consumes (gcsim's
    /// OnEnemyHit subscriber).
    fn own_hit(s: &Sim, ci: usize, ev: &mut AttackEvent) {
        if ev.info.abil != g::SKILL_HOLD_ABIL {
            return;
        }
        let c = &s.st::<State>(ci).conductive;
        ev.info.mult = g::SKILL_HOLD[c.get() as usize][s.talent(ci, 1)];
        c.set(0);
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let hitmark = g::ATTACK_HITMARKS[n];
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_LISA_ELECTRO, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.circle_on_target(1.0);
        s.queue_attack(ai, 0, hitmark, hitmark);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hitmark, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.pattern = s.fan_on_player(0.0, 1.0, 10.0, 40.0);
        // no windup from a normal attack or the held skill
        let windup = match s.current_state() {
            AnimState::Normal => 14,
            AnimState::Skill if s.st::<State>(ci).last_skill_hold => 14,
            _ => 0,
        };
        let cb = if s.chars[ci].ascension >= 1 { CB_A1 } else { 0 };
        s.queue_attack(ai, cb, g::CHARGE_HITMARK - windup, g::CHARGE_HITMARK - windup);
        ActInfo::new(Frames::Char { id: FRAMES_CHARGE, arg: windup as i16 }, g::CHARGE_FRAMES[0] - windup, g::CHARGE_HITMARK - windup, AnimState::Charge)
    }

    fn frames(_: u8, windup: i16, next: Action) -> i32 {
        g::CHARGE_FRAMES[next as usize] - windup as i32
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold") == Some(1);
        s.st_mut::<State>(ci).last_skill_hold = hold;
        let lvl = s.talent(ci, 1);
        if !hold {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_LISA_ELECTRO, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_PRESS[lvl]);
            ai.abil = g::SKILL_PRESS_ABIL;
            ai.pattern = s.circle_on_target(1.0);
            s.queue_attack(ai, CB_PRESS, 0, g::SKILL_PRESS_HITMARK);
            s.set_cd_with_delay(ci, Action::Skill, 60, 17);
            let t = &g::SKILL_PRESS_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill);
        }
        if s.chars[ci].cons >= 2 {
            s.add_stat_mod(ci, C2, st::DEFP, 0.25, 126, false);
        }
        s.st_mut::<State>(ci).c1_count = 0;
        // the multiplier is set as it hits, by the enemy's stacks
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 50.0, 0.0);
        ai.abil = g::SKILL_HOLD_ABIL;
        if enemy_within(s, s.circle_on_player(0.0, 0.0, 10.0)) {
            ai.pattern = s.circle_on_target(0.2);
            let cb = if s.chars[ci].cons > 0 { CB_C1 } else { 0 } | CB_PARTICLE;
            s.queue_attack(ai, cb, 0, g::SKILL_HOLD_HITMARK);
        }
        s.set_cd_with_delay(ci, Action::Skill, 960, 114);
        let t = &g::SKILL_HOLD_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let c = ci as u8;
        // the first zap: no element, no DEF shred
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, 0.1);
        ai.abil = "Lightning Rose (Initial)";
        ai.pattern = s.circle_on_player(0.0, 0.0, 7.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.add_task(Task::Char { c, id: TASK_BURST_SNAP, arg: 0 }, g::BURST_HITMARK - 1);
        // 30 ticks, one every 0.5 s from frame 119
        for i in (0..15 * 60).step_by(30) {
            let progress = i + BURST_FIRST_TICK;
            s.add_task(Task::Char { c, id: TASK_BURST_TICK, arg: progress }, progress);
        }
        s.add_task(Task::Char { c, id: TASK_BURST_STATUS, arg: 0 }, g::BURST_HITMARK);
        s.consume_energy(ci, 63);
        s.set_cd_with_delay(ci, Action::Burst, 1200, 53);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_BURST_SNAP => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::BURST[lvl]);
                ai.abil = "Lightning Rose (Tick)";
                s.st_mut::<State>(ci).burst = s.snapshot_event(ai, 0);
            }
            TASK_BURST_TICK => burst_tick(s, ci, arg),
            TASK_BURST_STATUS => s.add_core_status(BURST_STATUS, BURST_FIRST_TICK + 900),
            TASK_C1 => s.add_energy(ci, 2.0),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        // the pressed skill's stacks: not while off the field
        if cbs & CB_PRESS != 0 && s.player.active as usize == ci {
            add_stack(s, ci);
        }
        if cbs & CB_A1 != 0 {
            add_stack(s, ci);
        }
        if cbs & CB_A4 != 0 {
            s.enemy_add_def(A4, -0.15, 600);
        }
        if cbs & CB_C1 != 0 {
            let st = s.st_mut::<State>(ci);
            if st.c1_count < 5 {
                st.c1_count += 1;
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C1, arg: 0 }, 24);
            }
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 12, true);
            s.queue_particle(5.0, Element::Electro, s.particle_delay[ci]);
        }
    }

    /// C6: swapping to her sets the enemy's stacks to 3, once every 5 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Swap { .. } = *e {
            let st = s.st::<State>(ci);
            if s.f < st.c6_icd && st.c6_icd != 0 {
                return;
            }
            if s.player.active as usize == ci {
                if enemy_within(s, s.circle_on_player(0.0, 0.0, 5.0)) {
                    s.st::<State>(ci).conductive.set(3);
                }
                let f = s.f;
                s.st_mut::<State>(ci).c6_icd = f + 300;
            }
        }
    }
}

/// A tick of Lightning Rose: below C4 one discharge on the enemy; from C4
/// up to three, on the enemy and then on enemy-camp gadgets in the area.
fn burst_tick(s: &mut Sim, ci: usize, progress: i32) {
    let area = s.circle_on_player(0.0, 0.0, 7.0);
    let cb = if s.chars[ci].ascension >= 4 { CB_A4 } else { 0 };
    let enemy = enemy_within(s, area);
    if s.chars[ci].cons < 4 {
        if !enemy {
            return;
        }
        // RandomEnemyWithinArea: the one enemy
        let _ = s.rng.intn(1);
        let p = s.circle_on_target(1.0);
        discharge(s, ci, p, cb);
        return;
    }
    let limit = 3;
    // RandomEnemiesWithinArea: a permutation of the one enemy
    let enemy_count = if enemy {
        let _ = s.rng.intn(1);
        1
    } else {
        0
    };
    // RandomGadgetsWithinArea: a random order of the gadgets in the area
    let mut gadgets = [(0.0, 0.0); 2];
    let mut gadget_count = 0;
    if enemy_count < limit {
        let mut found = [(0.0f64, 0.0f64); 16];
        let mut n = 0;
        for g in &s.gadgets {
            if g.enemy_camp && area.hits_circle(g.x, g.y, g.r) == Some(true) {
                if n == found.len() {
                    s.unsupported("lisa c4 with more than 16 gadgets");
                    return;
                }
                found[n] = (g.x, g.y);
                n += 1;
            }
        }
        if n > 0 {
            // Go's rand.Perm
            let mut perm = [0usize; 16];
            for i in 0..n {
                let j = s.rng.intn(i as i32 + 1) as usize;
                perm[i] = perm[j];
                perm[j] = i;
            }
            gadget_count = n.min((limit - enemy_count) as usize);
            for i in 0..gadget_count {
                gadgets[i] = found[perm[i]];
            }
        }
    }
    let total = enemy_count + gadget_count as i32;
    let mut count = match total {
        0 => 0,
        1 => 1,
        2 => {
            // the first: 60% one, 40% two; the rest: 15% one, 85% two
            let threshold = if progress == BURST_FIRST_TICK { 0.6 } else { 0.15 };
            if s.rng.float64() < threshold { 1 } else { 2 }
        }
        _ => {
            if progress == BURST_FIRST_TICK {
                // gcsim draws the first arc's count and returns without
                // discharging
                let c = if s.rng.float64() < 0.55 { 1 } else { 2 };
                s.st_mut::<State>(ci).previous_discharge_count = c;
                return;
            }
            let r = s.rng.float64();
            match s.st::<State>(ci).previous_discharge_count {
                1 => {
                    if r < 0.2 {
                        1
                    } else if r < 0.7 {
                        2
                    } else {
                        3
                    }
                }
                2 => {
                    if r < 0.25 {
                        1
                    } else if r < 0.75 {
                        2
                    } else {
                        3
                    }
                }
                3 => {
                    if r < 0.5 {
                        1
                    } else {
                        2
                    }
                }
                _ => 0,
            }
        }
    };
    s.st_mut::<State>(ci).previous_discharge_count = count;
    if count == 0 {
        return;
    }
    if enemy_count > 0 {
        let p = s.circle_on_target(1.0);
        discharge(s, ci, p, cb);
    }
    count -= enemy_count;
    for &(x, y) in gadgets.iter().take((count.max(0) as usize).min(gadget_count)) {
        discharge(s, ci, Pattern::Circle { x, y, r: 1.0 }, cb);
    }
}

/// A discharge with the burst's snapshot. The engine's attacks always hit
/// the enemy, so one whose circle does not reach it is an error.
fn discharge(s: &mut Sim, ci: usize, p: Pattern, cb: u16) {
    if !enemy_within(s, p) {
        s.unsupported("lisa burst discharge missing the enemy");
        return;
    }
    let mut ev = s.st::<State>(ci).burst;
    ev.info.pattern = p;
    s.queue_attack_with_snap(ev, cb, 0);
}
