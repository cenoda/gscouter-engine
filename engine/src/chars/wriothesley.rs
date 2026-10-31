//! Wriothesley, from gcsim's internal/characters/wriothesley, at C0:
//! normal attacks (with Chilling Penalty), charged attacks (A1's Rebuke),
//! the skill, the burst with its Surging Blade, A1 and A4. No plunges or
//! constellations. Radiance (stellar.go) needs a Polestar Field, which
//! nothing the engine has makes, so it never applies; port it with the
//! first content that makes one.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::wriothesley as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    saved_normal: i32,
    a4_stacks: i32,
    burst: Option<AttackEvent>,
}

const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const LOSE_HP_ICD: ModKey = ModKey::new(g::SKILL_LOSE_HPICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new(g::A1_STATUS);
const A1_ICD: ModKey = ModKey::new(g::A1_ICD_KEY);
const A4: ModKey = ModKey::new("wriothesley-a4");
const OUSIA_ICD: ModKey = ModKey::new(g::BURST_OUSIA_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_PENALTY: u16 = 1 << 1;
const CB_REBUKE: u16 = 1 << 2;

// arg: the normal's index * 8 + the hit's
const TASK_NORMAL: u8 = 0;
const TASK_OUSIA: u8 = 1;

const NAMES: [&str; 5] = ["Normal 1", "Normal 2", "Normal 3", "Normal 4", "Normal 5"];
const ENHANCED: [&str; 5] = ["Normal 1 (Enhanced)", "Normal 2 (Enhanced)", "Normal 3 (Enhanced)", "Normal 4 (Enhanced)", "Normal 5 (Enhanced)"];

/// Chilling Penalty's buff: the skill's status, above half HP.
fn skill_buff(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, SKILL) && s.chars[ci].hp_ratio > 0.5
}

/// A1's Gracious Rebuke: below 60% HP, off its cooldown.
fn rebuke_ready(s: &Sim, ci: usize) -> bool {
    s.chars[ci].ascension >= 1 && s.chars[ci].hp_ratio < 0.6 && !s.mod_active(ci, A1_ICD)
}

/// Dash and skill keep the normal attack combo only after a normal attack
/// or the skill.
fn keep_combo(s: &mut Sim, ci: usize) {
    if !matches!(s.player.last.action, Action::Attack | Action::Skill) {
        s.st_mut::<State>(ci).saved_normal = 0;
    }
}

impl Character for Char {
    const KEY: &'static str = "wriothesley";
    // C1 to C6 are not ported
    const MAX_CONS: i32 = 0;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Swap, Owner::Char(ci as u8));
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::Drain, Owner::Char(ci as u8));
            s.subscribe(EventKind::Heal, Owner::Char(ci as u8));
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// He charges only right after a normal attack.
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        (a == Action::Charge && s.player.last.action != Action::Attack).then(|| "need to use attack right before charge".into())
    }

    fn charge_stam(s: &Sim, ci: usize) -> Option<f64> {
        rebuke_ready(s, ci).then_some(0.0)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "gracious-rebuke" => Some(Ok(Num::from_bool(rebuke_ready(s, ci)))),
            // Radiance's, which needs a Polestar Field
            "gracious-rebuke-unveiling" => Some(Ok(Num::from_bool(false))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // the combo survives the skill and dashes
        if matches!(s.current_state(), AnimState::Dash | AnimState::Skill) {
            s.chars[ci].normal_counter = s.st::<State>(ci).saved_normal;
        }
        let n = s.chars[ci].normal_counter as usize;
        let hitmarks = g::ATTACK_HITMARKS[n];
        for (i, &h) in hitmarks.iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: (n * 8 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hitmarks[hitmarks.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        let next = s.chars[ci].normal_counter;
        s.st_mut::<State>(ci).saved_normal = next;
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.blunt = true;
        ai.poise = 110.0;
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = 0.09 * 60.0;
        let rebuke = rebuke_ready(s, ci);
        ai.pattern = if rebuke { s.box_on_player(0.0, -0.8, 4.0, 5.0) } else { s.box_on_player(0.0, -1.2, 2.8, 3.6) };
        let cbs = if rebuke { CB_REBUKE | CB_PARTICLE } else { 0 };
        let mut ev = s.snapshot_event(ai, cbs);
        // A1: Rebuke: Vaulting Fist, +50% DMG, heals on hit
        if rebuke {
            s.add_status(ci, A1, -1, false);
            ev.info.abil = "Rebuke: Vaulting Fist";
            ev.info.hitlag_factor = 0.03;
            ev.info.hitlag_halt = 0.12 * 60.0;
            ev.stats[st::DMGP] += 0.5;
        }
        s.queue_attack_with_snap(ev, cbs, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn dash_info(s: &mut Sim, ci: usize, info: ActInfo) -> ActInfo {
        keep_combo(s, ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        keep_combo(s, ci);
        let dur = 10 * 60 + g::SKILL_CD_DELAY;
        s.st_mut::<State>(ci).a4_stacks = 0;
        s.add_status(ci, SKILL, dur, true);
        // A4: 6% ATK for each Prosecution Edict stack
        if s.chars[ci].ascension >= 4 {
            s.add_mod(ci, Mod::owned(A4, ModKind::Stat, Owner::Char(ci as u8), st::ATKP, false, true, -1), dur);
        }
        s.set_cd_with_delay(ci, Action::Skill, 16 * 60, g::SKILL_CD_DELAY);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Burst as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST[lvl]);
        ai.abil = "Darkgold Wolfbite";
        ai.pattern = s.box_on_player(0.0, 0.0, 8.0, 16.0);
        let ev = s.snapshot_event(ai, 0);
        for &h in g::BURST_HITMARKS {
            s.queue_attack_with_snap(ev, 0, h);
        }
        s.st_mut::<State>(ci).burst = Some(ev);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_OUSIA, arg: 0 }, g::BURST_OUSIA_HITMARK);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 5);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = ((arg / 8) as usize, (arg % 8) as usize);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NAMES[n];
                ai.hitlag_factor = g::ATTACK_HITLAG_FACTOR[n][i];
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.def_halt = true;
                // Chilling Penalty, decided as the hit lands
                let buff = skill_buff(s, ci);
                let mut cbs = 0;
                if buff {
                    cbs = CB_PARTICLE | CB_PENALTY;
                    ai.abil = ENHANCED[n];
                    ai.mult *= g::SKILL[s.talent(ci, 1)];
                }
                // gcsim reads the hitbox at the combo's counter now, not the
                // hit's
                let k = buff as usize;
                let box_ = g::ATTACK_HITBOXES[k][s.chars[ci].normal_counter as usize];
                ai.pattern = s.box_on_player(0.0, g::ATTACK_OFFSETS[k], box_[0], box_[1]);
                let ev = s.snapshot_event(ai, cbs);
                s.queue_attack_with_snap(ev, cbs, 0);
            }
            // Surging Blade, once every 10 s, on the burst's snapshot
            TASK_OUSIA => {
                if s.mod_active(ci, OUSIA_ICD) {
                    return;
                }
                s.add_status(ci, OUSIA_ICD, 10 * 60, true);
                let Some(mut ev) = s.st::<State>(ci).burst else { return };
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 0.0, g::BURST_OUSIA[lvl]);
                ai.abil = "Surging Blade";
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = 0.03 * 60.0;
                ai.pattern = s.box_on_player(0.0, 0.0, 10.0, 16.0);
                ev.info = ai;
                s.queue_attack_with_snap(ev, 0, 0);
            }
            _ => unreachable!(),
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        out[st::ATKP] += s.st::<State>(ci).a4_stacks as f64 * 0.06;
        true
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_REBUKE != 0 && s.mod_active(ci, A1) {
            s.delete_mod(ci, A1);
            s.add_status(ci, A1_ICD, g::A1_ICD, true);
            let (hp, bonus) = (0.3 * s.max_hp(ci), s.stat(ci, st::HEAL));
            s.heal(ci, ci, hp, bonus);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD, true);
            s.queue_particle(1.0, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_PENALTY != 0 && !s.mod_active(ci, LOSE_HP_ICD) {
            s.add_status(ci, LOSE_HP_ICD, g::SKILL_LOSE_HPICD, true);
            let hp = 0.045 * s.max_hp(ci);
            s.drain(ci, hp);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // leaving the field ends the skill
            Event::Swap { prev, .. } => {
                if prev == ci && s.mod_active(ci, SKILL) {
                    s.delete_mod(ci, SKILL);
                }
            }
            // A4: a stack whenever his HP changes during the skill, on field
            Event::Drain { on, amount, .. } => {
                if s.player.active as usize == ci && on == ci && amount > 0.0 {
                    a4_stack(s, ci);
                }
            }
            Event::Heal { on, amount, overheal, .. } => {
                if s.player.active as usize == ci && on == ci && amount > 0.0 && (amount - overheal).abs() > 1e-9 {
                    a4_stack(s, ci);
                }
            }
            _ => {}
        }
    }
}

fn a4_stack(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, SKILL) {
        let st = s.st_mut::<State>(ci);
        st.a4_stacks = (st.a4_stacks + 1).min(5);
    }
}
