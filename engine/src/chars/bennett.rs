//! Bennett, from gcsim's internal/characters/bennett: normal and charged
//! attacks, the skill pressed and held (levels 1 and 2, and the C4 variant) and the
//! burst's field, which also infuses the player with pyro.

use crate::content::Character;
use crate::gcsimdata::chars::bennett as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// Bennett's healing bonus when he cast the burst.
    heal_bonus: f64,
}

const FIELD: ModKey = ModKey::new(g::BURST_FIELD_KEY);
const BURST_STATUS: ModKey = ModKey::new(g::BURST_KEY);
const PRESS_PARTICLE_ICD: ModKey = ModKey::new(g::PRESS_PARTICLE_ICD_KEY);
const HOLD_PARTICLE_ICD: ModKey = ModKey::new(g::HOLD_PARTICLE_ICD_KEY);
const C2: ModKey = ModKey::new("bennett-c2");
const C6_INFUSION: ModKey = ModKey::new("bennett-fire-weapon");

const CB_PRESS_PARTICLE: u16 = 1 << 0;
const CB_HOLD_PARTICLE: u16 = 1 << 1;

// tasks: the field's ticks (arg 1 for the first, which does not heal) and
// the held skill's hits (arg: level * 2 + hit)
const TASK_FIELD: u8 = 0;
const TASK_HOLD_HIT: u8 = 1;

impl Character for Char {
    const KEY: &'static str = "bennett";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        // a special default for Bennett, which set_particle_delay overrides
        s.particle_delay[ci] = 80;
        if s.chars[ci].cons >= 2 {
            s.chars[ci].mods.push(Mod::owned(C2, ModKind::Stat, Owner::Char(ci as u8), st::ER, false, false, -1));
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _on: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C2 {
            if s.chars[ci].hp_ratio < 0.7 {
                out[st::ER] += 0.3;
            }
            return true;
        }
        // the field's buff: ATK from Bennett's base ATK at the tick
        out[st::ATK] += m.value;
        if s.chars[ci].cons >= 6 {
            out[st::PYROP] += 0.15;
        }
        true
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAMES[n] * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        let off = g::ATTACK_OFFSETS[n];
        ai.pattern = if n == 3 {
            s.box_on_player(0.0, off, g::ATTACK_HITBOXES[n][0], g::ATTACK_HITBOXES[n][1])
        } else {
            s.fan_on_player(0.0, off, g::ATTACK_HITBOXES[n][0], g::ATTACK_FAN_ANGLES[n])
        };
        let hitmark = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, hitmark, hitmark);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hitmark, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    /// gcsim's ChargeAttack: two physical hits, each a circle of radius 2.2
    /// on the player, without hitlag. Stamina and the need for a normal
    /// attack right before are the template's (sword).
    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::CHARGE.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = ["Charge 0", "Charge 1"][i];
            ai.pattern = s.circle_on_player(0.0, 0.0, 2.2);
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], *g::CHARGE_HITMARKS.last().unwrap(), AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let mut level = p.get("hold").unwrap_or(0);
        if !(0..=2).contains(&level) {
            level = 0;
        }
        let mut c4 = false;
        if p.get("hold_c4") == Some(1) && s.chars[ci].cons >= 4 {
            level = 1;
            c4 = true;
        }
        if level == 0 { skill_press(s, ci) } else { skill_hold(s, ci, level as usize, c4) }
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_core_status(BURST_STATUS, 720 + g::BURST_START_FRAME);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 50.0, g::BURST[lvl]);
        ai.abil = "Fantastic Voyage";
        ai.pattern = s.circle_on_player(0.0, 0.5, 6.0);
        s.queue_attack(ai, 0, 37, 37);
        // the field ticks from 0 s to 12 s; the player is always in it
        s.st_mut::<State>(ci).heal_bonus = s.stats(ci)[st::HEAL];
        s.add_task(Task::Char { c: ci as u8, id: TASK_FIELD, arg: 1 }, g::BURST_START_FRAME);
        for i in (60..=12 * 60).step_by(60) {
            s.add_task(Task::Char { c: ci as u8, id: TASK_FIELD, arg: 0 }, i + g::BURST_START_FRAME);
        }
        s.consume_energy(ci, 36);
        s.set_cd_with_delay(ci, Action::Burst, 900, g::BURST_START_FRAME);
        ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_FRAMES[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_FIELD => field_tick(s, ci, arg == 1),
            TASK_HOLD_HIT => {
                let (level, i) = ((arg / 2) as usize, (arg % 2) as usize);
                let lvl = s.talent(ci, 1);
                let mut ai = hold_info(ci, level);
                ai.mult = g::SKILL_HOLD[level - 1][i][lvl];
                ai.hitlag_halt = 0.09 * 60.0;
                let (off, hb) = (g::SKILL_HOLD_OFFSETS[i], g::SKILL_HOLD_HITBOXES[i]);
                ai.pattern = if i == 1 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
                s.queue_attack(ai, CB_HOLD_PARTICLE, 0, 0);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PRESS_PARTICLE != 0 && !s.mod_active(ci, PRESS_PARTICLE_ICD) {
            s.add_status(ci, PRESS_PARTICLE_ICD, 18, true);
            let count = if s.rng.float64() < 0.25 { 3.0 } else { 2.0 };
            s.queue_particle(count, Element::Pyro, s.particle_delay[ci]);
        }
        if cbs & CB_HOLD_PARTICLE != 0 && !s.mod_active(ci, HOLD_PARTICLE_ICD) {
            s.add_status(ci, HOLD_PARTICLE_ICD, 90, true);
            s.queue_particle(3.0, Element::Pyro, s.particle_delay[ci]);
        }
    }
}

/// A1: the skill's cooldown is 20% shorter.
fn a1(s: &Sim, ci: usize, cd: i32) -> i32 {
    if s.chars[ci].ascension < 1 {
        return cd;
    }
    (cd as f64 * 0.8) as i32
}

/// A4: in the field, half as long again.
fn a4_cd(s: &Sim, ci: usize, cd: i32) -> i32 {
    if s.chars[ci].ascension < 4 || !s.mod_active(ci, FIELD) {
        return cd;
    }
    (cd as f64 * 0.5) as i32
}

fn skill_press(s: &mut Sim, ci: usize) -> ActInfo {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 50.0, g::SKILL[lvl]);
    ai.abil = "Passion Overload (Press)";
    ai.hitlag_halt = 0.09 * 60.0;
    ai.hitlag_factor = 0.01;
    ai.def_halt = true;
    ai.pattern = s.fan_on_player(0.0, 0.8, 2.5, 270.0);
    s.queue_attack(ai, CB_PRESS_PARTICLE, g::SKILL_PRESS_HITMARK, g::SKILL_PRESS_HITMARK);
    let cd = a4_cd(s, ci, a1(s, ci, 5 * 60));
    s.set_cd_with_delay(ci, Action::Skill, cd, 14);
    let t = &g::SKILL_FRAMES[0];
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
}

fn hold_info(ci: usize, level: usize) -> AttackInfo {
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, 0.0);
    ai.abil = ["", "Passion Overload (Level 1)", "Passion Overload (Level 2)"][level];
    ai.hitlag_factor = 0.01;
    ai.def_halt = true;
    ai
}

fn skill_hold(s: &mut Sim, ci: usize, level: usize, c4: bool) -> ActInfo {
    let lvl = s.talent(ci, 1);
    for (i, &hitmark) in g::SKILL_HOLD_HITMARKS[level - 1].iter().enumerate() {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_HOLD_HIT, arg: (level * 2 + i) as i32 }, hitmark);
    }
    if level == 2 {
        let mut ai = hold_info(ci, level);
        ai.mult = g::EXPLOSION[lvl];
        ai.pattern = s.circle_on_player(0.0, 1.0, 3.5);
        s.queue_attack(ai, CB_HOLD_PARTICLE, 166, 166);
    }
    // the C4 variant adds an attack of 135% of the second hit
    if level == 1 && c4 {
        let mut ai = hold_info(ci, level);
        ai.mult = g::SKILL_HOLD[0][1][lvl] * 1.35;
        ai.abil = "Passion Overload (C4)";
        ai.hitlag_halt = 0.12 * 60.0;
        ai.pattern = s.box_on_player(0.0, -1.0, 3.0, 4.0);
        s.queue_attack(ai, CB_HOLD_PARTICLE, 94, 94);
    }
    let (idx, cd, delay) = match level {
        1 => (if c4 { 2 } else { 1 }, 450, 43),
        _ => (if s.chars[ci].ascension >= 4 && s.mod_active(ci, FIELD) { 4 } else { 3 }, 600, 110),
    };
    let cd = a4_cd(s, ci, a1(s, ci, cd));
    s.set_cd_with_delay(ci, Action::Skill, cd, delay);
    let t = &g::SKILL_FRAMES[idx];
    ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
}

/// applyBennettField: heals the active character under 70% HP (not on the
/// first tick), and buffs its ATK when over the threshold (any HP from C1),
/// infusing its weapon with pyro from C6.
fn field_tick(s: &mut Sim, ci: usize, first: bool) {
    s.apply_self_infusion(AURA_PYRO, 25.0, g::BURST_BUFF_DURATION);
    let lvl = s.talent(ci, 2);
    let heal = g::BURSTHP[lvl] + g::BURSTHPP[lvl] * s.max_hp(ci);
    let mut pc = g::BURSTATK[lvl];
    if s.chars[ci].cons >= 1 {
        pc += 0.2;
    }
    let atk = pc * s.stat(ci, st::BASEATK);
    let active = s.player.active as usize;
    if !first && s.chars[active].hp_ratio < 0.7 {
        let bonus = s.st::<State>(ci).heal_bonus;
        s.heal(ci, active, heal, bonus);
    }
    let threshold = if s.chars[ci].cons >= 1 { 0.0 } else { 0.7 };
    if s.chars[active].hp_ratio > threshold {
        if s.chars[ci].cons >= 6 && matches!(s.chars[active].class, WeaponClass::Claymore | WeaponClass::Spear | WeaponClass::Sword) {
            s.add_infusion(active, C6_INFUSION, Element::Pyro, g::BURST_BUFF_DURATION, true, &[AttackTag::Normal, AttackTag::Extra, AttackTag::Plunge]);
        }
        let mut m = Mod::owned(FIELD, ModKind::Stat, Owner::Char(ci as u8), st::NONE, true, true, -1);
        m.value = atk;
        s.add_mod(active, m, g::BURST_BUFF_DURATION);
    }
}
