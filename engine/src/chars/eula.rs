//! Eula, from gcsim's internal/characters/eula: normal attacks, Icetide
//! Vortex pressed (Grimheart stacks) and held (the Icewhirls and their
//! shred, A1's Shattered Lightfall Sword, C1), Glacial Illumination (the
//! hit, the Lightfall Sword's stacks from her hits while she is on the
//! field, and its explosion at the end or when she leaves), A4, the
//! airborne plunges and every constellation.
//!
//! As gcsim writes her: A4's Grimheart stack does not refresh its duration
//! (a stack added to expired Grimheart is lost), and the burst's initial hit
//! gives no Lightfall stack.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gamedata::chars::eula as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    burst_counter: i32,
    grimheart: i32,
    /// The Grimheart stacks the current hold consumed (its shred's length).
    shred_v: i32,
    /// C4: the enemy's HP at the start, in damage mode.
    enemy_max_hp: f64,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const STACK_ICD: ModKey = ModKey::new(g::BURST_STACK_ICD_KEY);
const GRIM_ICD: ModKey = ModKey::new(g::GRIMHEART_ICD);
const GRIM_DUR: ModKey = ModKey::new(g::GRIMHEART_DURATION);
const PRESS_PARTICLE_ICD: ModKey = ModKey::new(g::PRESS_PARTICLE_ICD_KEY);
const HOLD_PARTICLE_ICD: ModKey = ModKey::new(g::HOLD_PARTICLE_ICD_KEY);
const SHRED_CRYO: ModKey = ModKey::new("eula-icewhirl-shred-cryo");
const SHRED_PHYS: ModKey = ModKey::new("eula-icewhirl-shred-phys");
const C1: ModKey = ModKey::new("eula-c1");
const C4: ModKey = ModKey::new("eula-c4");

const CB_GRIM: u16 = 1 << 0;
const CB_PRESS_PARTICLE: u16 = 1 << 1;
const CB_HOLD_PARTICLE: u16 = 1 << 2;
const CB_SHRED: u16 = 1 << 3;
const CB_STACK: u16 = 1 << 4;

/// The second Icewhirl, on her queue.
const TASK_ICEWHIRL2: u8 = 0;
/// A1's sword, queued as the hold's hit lands.
const TASK_A1: u8 = 1;
/// The burst's status, from its earliest cancel.
const TASK_BURST_STATUS: u8 = 2;
/// The Lightfall Sword before the status would end.
const TASK_BURST_END: u8 = 3;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];

/// gcsim's plunge.go: the physical plunges on Xianyun's airborne state.
const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

impl Character for Char {
    const KEY: &'static str = "eula";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        // onExitField
        s.subscribe(EventKind::Swap, Owner::Char(ci as u8));
        if s.chars[ci].cons >= 4 && s.damage_mode {
            s.st_mut::<State>(ci).enemy_max_hp = s.enemy_hp;
            s.chars[ci].mods.push(Mod::owned(C4, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1));
        }
    }

    /// gcsim validates hold on the skill and collision on the plunges, and
    /// reads no params of the other actions.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Burst => None,
            Action::Skill => p.0.iter().find(|(k, _)| k != "hold").map(|(k, _)| format!("{name} param {k}")),
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::AUTO[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.blunt = true;
            ai.poise = g::ATTACK_POISE_DMG[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let hb = g::ATTACK_HITBOXES[n];
            let oy = g::ATTACK_OFFSETS[n];
            ai.pattern = if n == 4 { s.box_on_player(0.0, oy, hb[0], hb[1]) } else { s.fan_on_player(0.0, oy, hb[0], g::ATTACK_FAN_ANGLES[n]) };
            s.queue_attack_at(ci, ai, CB_STACK, g::ATTACK_HITMARKS[n][i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let marks = g::ATTACK_HITMARKS[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], marks[marks.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        if p.get("hold").unwrap_or(0) == 0 {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL_PRESS[lvl]);
            ai.abil = "Icetide Vortex";
            ai.blunt = true;
            ai.poise = 120.0;
            ai.hitlag_halt = 0.09 * 60.0;
            ai.hitlag_factor = 0.01;
            ai.def_halt = true;
            ai.pattern = s.circle_on_player(0.0, 1.0, 3.5);
            s.queue_attack(ai, CB_GRIM | CB_PRESS_PARTICLE | CB_STACK, g::SKILL_PRESS_HITMARK, g::SKILL_PRESS_HITMARK);
            s.set_cd_with_delay(ci, Action::Skill, 60 * 4, 16);
            let t = &g::SKILL_PRESS_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL_HOLD[lvl]);
        ai.abil = "Icetide Vortex (Hold)";
        ai.blunt = true;
        ai.poise = 150.0;
        ai.hitlag_halt = 0.12 * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.pattern = s.circle_on_player(0.0, 1.0, 5.5);
        s.queue_attack(ai, CB_HOLD_PARTICLE | CB_STACK, g::SKILL_HOLD_HITMARK, g::SKILL_HOLD_HITMARK);

        let v = current_grimheart(s, ci);
        s.st_mut::<State>(ci).shred_v = v;
        let shred = if v > 0 { CB_SHRED } else { 0 };
        if v > 0 {
            let h = g::ICEWHIRL_HITMARKS[0];
            s.queue_attack(icewhirl(s, ci), shred | CB_STACK, h, h);
        }
        if v > 1 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ICEWHIRL2, arg: 0 }, g::ICEWHIRL_HITMARKS[1]);
        }
        // a1: queued after the hold's hitlag starts, before it ends
        if v == 2 && s.chars[ci].ascension >= 1 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, g::SKILL_HOLD_HITMARK + 1);
        }
        if s.chars[ci].cons >= 1 && v > 0 {
            s.add_mod(ci, Mod::stat(C1, st::PHYP, 0.3, false, true, -1), (6 * v + 6) * 60);
        }
        // consumeGrimheartStacks
        s.st_mut::<State>(ci).grimheart = 0;
        s.delete_mod(ci, GRIM_DUR);
        let cd = if s.chars[ci].cons >= 2 { 4 } else { 10 };
        s.set_cd_with_delay(ci, Action::Skill, cd * 60, 46);
        let t = &g::SKILL_HOLD_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.st_mut::<State>(ci).burst_counter = if s.chars[ci].cons >= 6 { 5 } else { 0 };
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 50.0, g::BURST_INITIAL[lvl]);
        ai.abil = g::BURST_INITIAL_ABIL;
        ai.blunt = true;
        ai.pattern = s.circle_on_player(0.0, 0.0, 8.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        // a4: a Grimheart stack (its duration not refreshed) and the
        // skill's cooldown reset
        if s.chars[ci].ascension >= 4 {
            let st = s.st_mut::<State>(ci);
            if st.grimheart < 2 {
                st.grimheart += 1;
            }
            s.reset_cd(ci, Action::Skill);
        }
        let walk = g::BURST_FRAMES[Action::Walk as usize];
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_STATUS, arg: 0 }, walk);
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_END, arg: 0 }, 600 - g::LIGHTFALL_HITMARK);
        s.consume_energy(ci, 107);
        s.set_cd_with_delay(ci, Action::Burst, 20 * 60, 97);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], walk, AnimState::Burst)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_ICEWHIRL2 => {
                let shred = if s.st::<State>(ci).shred_v > 0 { CB_SHRED } else { 0 };
                let ai = icewhirl(s, ci);
                s.queue_attack(ai, shred | CB_STACK, 0, 0);
            }
            TASK_A1 => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::BURST_EXPLODE_BASE[lvl] * 0.5);
                ai.abil = "Icetide (Lightfall)";
                ai.blunt = true;
                ai.poise = 180.0;
                ai.pattern = s.circle_on_player(0.0, 2.0, 6.5);
                let d = g::A1_HITMARK - (g::SKILL_HOLD_HITMARK + 1);
                s.queue_attack(ai, CB_STACK, d, d);
            }
            TASK_BURST_STATUS => {
                let walk = g::BURST_FRAMES[Action::Walk as usize];
                s.add_core_status(BURST, 600 - g::LIGHTFALL_HITMARK - walk + 1);
            }
            TASK_BURST_END => {
                if s.core_status_key(BURST) > 0 {
                    trigger_burst(s, ci);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_GRIM != 0 && !s.mod_active(ci, GRIM_ICD) {
            s.add_status(ci, GRIM_ICD, 18, true);
            // addGrimheartStack
            if !s.mod_active(ci, GRIM_DUR) {
                s.st_mut::<State>(ci).grimheart = 0;
            }
            let st = s.st_mut::<State>(ci);
            if st.grimheart < 2 {
                st.grimheart += 1;
            }
            s.add_status(ci, GRIM_DUR, 1080, true);
        }
        if cbs & CB_PRESS_PARTICLE != 0 && !s.mod_active(ci, PRESS_PARTICLE_ICD) {
            s.add_status(ci, PRESS_PARTICLE_ICD, 18, true);
            let count = if s.rng.float64() < 0.5 { 2.0 } else { 1.0 };
            s.queue_particle(count, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_HOLD_PARTICLE != 0 && !s.mod_active(ci, HOLD_PARTICLE_ICD) {
            s.add_status(ci, HOLD_PARTICLE_ICD, 30, true);
            let count = if s.rng.float64() < 0.5 { 3.0 } else { 2.0 };
            s.queue_particle(count, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_SHRED != 0 {
            let v = s.st::<State>(ci).shred_v;
            let r = g::RES_RED[s.talent(ci, 1)];
            s.enemy_add_resist(SHRED_CRYO, Element::Cryo, -r, 7 * v * 60);
            s.enemy_add_resist(SHRED_PHYS, Element::Physical, -r, 7 * v * 60);
        }
        if cbs & CB_STACK != 0 {
            // burstStackCB: her hits while she is on the field, once every
            // 0.1 s
            if s.player.active as usize != ci || s.core_status_key(BURST) == 0 || ev.damage == 0.0 || s.mod_active(ci, STACK_ICD) {
                return;
            }
            s.add_status(ci, STACK_ICD, 6, false);
            s.st_mut::<State>(ci).burst_counter += 1;
            if s.chars[ci].cons == 6 && s.rng.float64() < 0.5 {
                s.st_mut::<State>(ci).burst_counter += 1;
            }
        }
    }

    /// C4: the burst's hit DMG +25% on an enemy below half HP.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C4 && ev.info.abil == g::BURST_INITIAL_ABIL && s.enemy_hp / s.st::<State>(ci).enemy_max_hp < 0.5 {
            ev.stats[st::DMGP] += 0.25;
        }
    }

    /// onExitField: any swap during the burst drops the Lightfall Sword.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Swap { .. } = *e {
            if s.core_status_key(BURST) > 0 {
                trigger_burst(s, ci);
            }
        }
    }
}

/// currentGrimheartStacks: none once its duration is over.
fn current_grimheart(s: &mut Sim, ci: usize) -> i32 {
    if !s.mod_active(ci, GRIM_DUR) {
        s.st_mut::<State>(ci).grimheart = 0;
        return 0;
    }
    let st = s.st_mut::<State>(ci);
    st.grimheart = st.grimheart.min(2);
    st.grimheart
}

/// An Icewhirl, around the player as it is queued.
fn icewhirl(s: &Sim, ci: usize) -> AttackInfo {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::ICEWHIRL[lvl]);
    ai.abil = "Icetide Vortex (Icewhirl)";
    ai.pattern = s.circle_on_player(0.0, 0.0, 3.5);
    ai
}

/// triggerBurst: the Lightfall Sword with its stacks (up to 30).
fn trigger_burst(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 2);
    let n = s.st::<State>(ci).burst_counter.min(30);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 50.0, g::BURST_EXPLODE_BASE[lvl] + g::BURST_EXPLODE_STACK[lvl] * n as f64);
    ai.abil = g::BURST_LIGHTFALL_ABIL;
    ai.blunt = true;
    ai.poise = 400.0;
    ai.pattern = s.circle_on_player(0.0, 0.0, 6.5);
    s.queue_attack(ai, 0, g::LIGHTFALL_HITMARK, g::LIGHTFALL_HITMARK);
    s.delete_core_status(BURST);
    s.st_mut::<State>(ci).burst_counter = 0;
}
