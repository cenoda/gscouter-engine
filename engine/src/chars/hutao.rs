//! Hu Tao, from gcsim's internal/characters/hutao.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::hutao as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    apply_a1: bool,
    heal_count: i32,
    heal_src: f64,
    /// The frame the current Blood Blossom started.
    bb_src: i32,
}

const PARAMITA: ModKey = ModKey::new(g::PARAMITA_BUFF);
const PARAMITA_ICD: ModKey = ModKey::new(g::PARAMITA_ENERGY_ICD);
const A1: ModKey = ModKey::new(g::A1_BUFF_KEY);
const A4: ModKey = ModKey::new("hutao-a4");
const BLOOD_BLOSSOM: ModKey = ModKey::new(g::BB_DEBUFF);
const C6: ModKey = ModKey::new("hutao-c6");
const C6_ICD: ModKey = ModKey::new("hutao-c6-icd");

// attack callbacks
const CB_PARTICLE: u16 = 1 << 0;
const CB_BB: u16 = 1 << 1;
const CB_HEAL: u16 = 1 << 2;

// tasks
const TASK_A1: u8 = 0;
const TASK_BB_TICK: u8 = 1;
// arg: 1 for the check every 2 s, which needs HP under 2
const TASK_C6_CHECK: u8 = 2;

// frames and on_removed ids
const PP_CHARGE: u8 = 0;

fn pp_charge_frames(prev: i16, next: Action) -> i32 {
    if prev == -1 || !matches!(next, Action::Dash | Action::Jump) {
        return g::PP_CHARGE_FRAMES[next as usize];
    }
    match prev {
        0 => {
            if next == Action::Dash {
                1
            } else {
                2
            }
        }
        1 => {
            if next == Action::Dash {
                4
            } else {
                5
            }
        }
        2 => 2,
        3 => 3,
        4 => 3,
        _ => 500,
    }
}

impl Character for Char {
    const KEY: &'static str = "hutao";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { bb_src: -1, ..Default::default() }
    }

    fn init(s: &mut Sim, ci: usize) {
        // onExitField
        s.subscribe(EventKind::Swap, Owner::Char(ci as u8));
        if s.chars[ci].ascension >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Stat, Owner::Char(ci as u8), st::PYROP, false, false, -1));
        }
        // C4 needs an enemy with Blood Blossom to die, which ends the fight
        if s.chars[ci].cons >= 6 {
            s.subscribe(EventKind::Drain, Owner::Char(ci as u8));
            // every 2 s from the start, once HP is set
            s.add_task(Task::Char { c: ci as u8, id: TASK_C6_CHECK, arg: 1 }, 1);
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _on: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == A4 && s.chars[ci].hp_ratio <= 0.5 {
            out[st::PYROP] += 0.33;
            return true;
        }
        false
    }

    fn snapshot(s: &Sim, ci: usize, info: &mut AttackInfo) {
        if s.mod_active(ci, PARAMITA) && matches!(info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            info.element = Element::Pyro;
        }
    }

    fn charge_stam(s: &Sim, ci: usize) -> Option<f64> {
        (s.chars[ci].cons >= 1 && s.mod_active(ci, PARAMITA)).then_some(0.0)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let pp = s.mod_active(ci, PARAMITA);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let hitmarks = if pp { g::PP_ATTACK_HITMARKS[n] } else { g::ATTACK_HITMARKS[n] };
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = (if pp { g::PP_ATTACK_HITLAG_HALT_FRAME[n][i] } else { g::ATTACK_HITLAG_HALT_FRAME[n][i] }) * 60.0;
            ai.def_halt = if pp { g::PP_ATTACK_DEF_HALT[n][i] } else { g::ATTACK_DEF_HALT[n][i] };
            let cb = if pp { CB_PARTICLE } else { 0 };
            s.queue_attack_at(ci, ai, cb, hitmarks[i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = if pp { &g::PP_ATTACK_FRAMES[n] } else { &g::ATTACK_FRAMES[n] };
        let mut info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *hitmarks.last().unwrap(), AnimState::Normal);
        if pp && n == 1 {
            info.norm_for = Some(Action::Charge);
        }
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        if !s.mod_active(ci, PARAMITA) {
            s.queue_attack(ai, 0, 0, g::CHARGE_HITMARK);
            return ActInfo::new(Frames::Table(&g::CHARGE_FRAMES), g::CHARGE_FRAMES[0], g::CHARGE_HITMARK, AnimState::Charge);
        }
        s.extend_mod(ci, PARAMITA, 108);
        s.queue_attack(ai, CB_PARTICLE | CB_BB, 0, g::PP_CHARGE_HITMARK);
        let mut prev: i16 = -1;
        let last = &s.player.last;
        if last.char as usize == ci && last.action == Action::Attack {
            let c = &s.chars[ci];
            prev = (c.normal_counter - 1) as i16;
            if prev < 0 {
                prev = (c.normal_hit_num - 1) as i16;
            }
        }
        let mut info = ActInfo::new(Frames::Char { id: PP_CHARGE, arg: prev }, g::PP_CHARGE_FRAMES[0], 1, AnimState::Charge);
        info.on_removed = OnRemoved::Char(PP_CHARGE);
        info
    }

    fn frames(_: u8, prev: i16, next: Action) -> i32 {
        pp_charge_frames(prev, next)
    }

    fn on_removed(s: &mut Sim, ci: usize, _: u8, next: AnimState) {
        if next != AnimState::Burst {
            s.extend_mod(ci, PARAMITA, -108);
        }
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut bonus = g::PPATK[lvl] * s.max_hp(ci);
        let max_bonus = s.stat(ci, st::BASEATK) * 4.0;
        if bonus > max_bonus {
            bonus = max_bonus;
        }
        s.add_mod(ci, Mod::stat(PARAMITA, st::ATK, bonus, true, true, -1), 540 + g::SKILL_START);
        s.st_mut::<State>(ci).apply_a1 = true;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, 540 + g::SKILL_START);
        let drain = 0.30 * s.current_hp(ci);
        s.drain(ci, drain);
        // the 0 dmg attack at skillStart changes nothing here
        s.set_cd_with_delay(ci, Action::Skill, 960, 14);
        ActInfo::new(Frames::Table(&g::SKILL_FRAMES), g::SKILL_FRAMES[0], g::SKILL_FRAMES[Action::Burst as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let low = s.chars[ci].hp_ratio <= 0.5;
        let lvl = s.talent(ci, 2);
        let (mult, regen) = if low { (g::BURST_LOW[lvl], g::REGEN_LOW[lvl]) } else { (g::BURST[lvl], g::REGEN[lvl]) };
        let heal_src = s.max_hp(ci) * regen;
        let st = s.st_mut::<State>(ci);
        st.heal_count = 0;
        st.heal_src = heal_src;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 50.0, mult);
        ai.abil = if low { "Spirit Soother (Low HP)" } else { "Spirit Soother" };
        let cb = CB_HEAL | if s.chars[ci].cons >= 2 { CB_BB } else { 0 };
        s.queue_attack(ai, cb, 0, g::BURST_HITMARK);
        s.consume_energy(ci, 68);
        s.set_cd_with_delay(ci, Action::Burst, 900, 62);
        ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_FRAMES[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_A1 => a1(s, ci),
            TASK_BB_TICK => bb_tick(s, ci, arg),
            TASK_C6_CHECK => c6_check(s, ci, arg == 1),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 {
            particle_cb(s, ci);
        }
        if cbs & CB_BB != 0 {
            apply_bb(s, ci);
        }
        if cbs & CB_HEAL != 0 {
            burst_heal_cb(s, ci);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { .. } => {
                if s.mod_active(ci, PARAMITA) {
                    a1(s, ci);
                    s.delete_mod(ci, PARAMITA);
                }
            }
            Event::Drain { amount, .. } if amount > 0.0 => c6_check(s, ci, false),
            _ => {}
        }
    }
}

/// C6: under 25% HP (under 2 HP for the check every 2 s), CRIT Rate 100%
/// for 10 s, once a minute. Every check queues the next one.
fn c6_check(s: &mut Sim, ci: usize, every_2s: bool) {
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6_CHECK, arg: 1 }, 120);
    if s.mod_active(ci, C6_ICD) || s.chars[ci].hp_ratio > 0.25 {
        return;
    }
    if every_2s && s.current_hp(ci) >= 2.0 {
        return;
    }
    if s.chars[ci].hp_ratio <= 0.0 {
        s.set_hp(ci, 1.0);
    }
    s.add_stat_mod(ci, C6, st::CR, 1.0, 600, true);
    s.add_status(ci, C6_ICD, 3600, false);
}

fn a1(s: &mut Sim, ci: usize) {
    if s.chars[ci].ascension < 1 || !s.st::<State>(ci).apply_a1 {
        return;
    }
    s.st_mut::<State>(ci).apply_a1 = false;
    for i in 0..s.chars.len() {
        if i != ci {
            s.add_mod(i, Mod::stat(A1, st::CR, 0.12, false, true, -1), 480);
        }
    }
}

fn particle_cb(s: &mut Sim, ci: usize) {
    if !s.mod_active(ci, PARAMITA) || s.mod_active(ci, PARAMITA_ICD) {
        return;
    }
    s.add_status(ci, PARAMITA_ICD, 300, true);
    let count = if s.rng.float64() < 0.5 { 3.0 } else { 2.0 };
    s.queue_particle(count, Element::Pyro, s.particle_delay[ci]);
}

fn apply_bb(s: &mut Sim, ci: usize) {
    if !s.enemy_status_active(BLOOD_BLOSSOM) {
        let f = s.f;
        s.queue_enemy_task(Task::Char { c: ci as u8, id: TASK_BB_TICK, arg: f }, 240);
        s.st_mut::<State>(ci).bb_src = f;
    }
    s.enemy_add_status(BLOOD_BLOSSOM, 570, true);
}

fn bb_tick(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).bb_src != src || !s.enemy_status_active(BLOOD_BLOSSOM) {
        return;
    }
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BB[lvl]);
    ai.abil = "Blood Blossom";
    if s.chars[ci].cons >= 2 {
        ai.flat_dmg += s.max_hp(ci) * 0.1;
    }
    s.queue_attack(ai, 0, 0, 0);
    s.queue_enemy_task(Task::Char { c: ci as u8, id: TASK_BB_TICK, arg: src }, 240);
}

fn burst_heal_cb(s: &mut Sim, ci: usize) {
    let st = s.st_mut::<State>(ci);
    if st.heal_count == 5 {
        return;
    }
    st.heal_count += 1;
    let src = st.heal_src;
    let bonus = s.stat(ci, st::HEAL);
    s.heal(ci, ci, src, bonus);
}
