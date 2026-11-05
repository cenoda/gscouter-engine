//! Tartaglia, from gcsim's internal/characters/tartaglia: the ranged
//! stance (normal attacks, aimed shots that apply Riptide, Riptide Flash),
//! Foul Legacy's melee stance (its hydro normal and charged attacks, Riptide
//! Slash, the stance's end after 30 s, on a swap or a second press, with
//! the cooldown by the time spent in it), both bursts (the ranged one's
//! energy refund and Riptide, the melee one's Riptide Blast), Riptide Burst
//! on a kill, his talent passive (+1 normal attack level for the party), A1,
//! A4 and every constellation (C1's cooldown, C2's energy on a kill, C4's
//! periodic Flash or Slash, C6's cooldown reset).

use crate::content::{Character, Event};
use crate::gamedata::chars::tartaglia as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    riptide_duration: i32,
    /// The frame Foul Legacy was cast to enter the melee stance.
    e_cast: i32,
    /// C4: the frame of the last Riptide applied to an enemy without it.
    c4_src: i32,
    /// C6: the melee burst was used in this stance.
    ml_burst_used: bool,
}

const RIPTIDE: ModKey = ModKey::new(g::RIPTIDE_KEY);
const RIPTIDE_FLASH_ICD: ModKey = ModKey::new(g::RIPTIDE_FLASH_ICD_KEY);
const RIPTIDE_SLASH_ICD: ModKey = ModKey::new(g::RIPTIDE_SLASH_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const MELEE: ModKey = ModKey::new(g::MELEE_KEY);

const TRAVEL: i32 = 10;

// the callbacks, run in the order of each attack's gcsim callbacks
const CB_FLASH: u16 = 1 << 0;
const CB_AIM_RIPTIDE: u16 = 1 << 1;
const CB_A4: u16 = 1 << 2;
const CB_RANGED_BURST: u16 = 1 << 3;
const CB_BLAST: u16 = 1 << 4;
const CB_SLASH: u16 = 1 << 5;
const CB_PARTICLE: u16 = 1 << 6;

const TASK_STANCE_END: u8 = 0;
const TASK_C4: u8 = 1;
const TASK_RT_BURST: u8 = 2;

const FR_WALK: u8 = 0;

impl Character for Char {
    const KEY: &'static str = "tartaglia";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let mut dur = 10 * 60;
        // a1: Riptide lasts 8 s longer
        if s.chars[ci].ascension >= 1 {
            dur += 8 * 60;
        }
        s.st_mut::<State>(ci).riptide_duration = dur;
        let o = Owner::Char(ci as u8);
        // onExitField, then onDefeatTargets
        s.subscribe(EventKind::Swap, o);
        s.subscribe(EventKind::TargetDied, o);
        // the talent passive: the party's normal attack level goes up by 1
        for c in s.chars.iter_mut() {
            c.attack_talent_bonus = 1;
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let allowed: &[&str] = match a {
            Action::Attack => &["travel"],
            Action::Aim => &["hold", "travel", "weakspot"],
            Action::Charge => &["hitWeakPoint"],
            // gcsim validates no params for them and reads none
            Action::Skill | Action::Burst => return None,
            _ => return Some(name.into()),
        };
        if let Some((k, _)) = p.0.iter().find(|(k, _)| !allowed.contains(&k.as_str())) {
            return Some(format!("{name} param {k}"));
        }
        if a == Action::Aim && !matches!(p.get("hold"), None | Some(0) | Some(1)) {
            return Some("aim[hold]".into());
        }
        None
    }

    /// NextQueueItemIsValid: a charged attack only right after a normal
    /// attack; and the errors of the actions in the wrong stance.
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Charge if s.player.last.action != Action::Attack => Some("need to use attack right before charge".into()),
            Action::Charge if !s.mod_active(ci, MELEE) => Some("charge called when not in melee stance".into()),
            Action::Aim if s.mod_active(ci, MELEE) => Some("aim called when not in ranged stance".into()),
            _ => crate::content::template_invalid(s, a),
        }
    }

    fn action_stam(_: &Sim, _: usize, a: Action) -> Option<f64> {
        (a == Action::Charge).then_some(20.0)
    }

    /// AnimationXingqiuN0StartDelay: 12 in the melee stance.
    fn start_delay(s: &Sim, ci: usize, key: usize) -> Option<i32> {
        (key == 2 && s.mod_active(ci, MELEE)).then_some(12)
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.mod_active(ci, MELEE) {
            return melee_attack(s, ci);
        }
        let travel = p.get("travel").map_or(TRAVEL, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").map_or(TRAVEL, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::FULLAIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        ai.deployable = true;
        // its hitlag applies on weak spot hits only (HitlagOnHeadshotOnly)
        if p.get("weakspot") == Some(1) {
            ai.weak_point = true;
            ai.hitlag_halt = 0.12 * 60.0;
            ai.hitlag_factor = 0.01;
        }
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let h = g::AIMED_HITMARKS[hold];
        // Flash before Riptide: a fully-charged shot alone does not flash
        s.queue_attack(ai, CB_FLASH | CB_AIM_RIPTIDE | CB_SLASH, h, h + travel);
        let t = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Aim)
    }

    fn charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let weak = p.get("hitWeakPoint").unwrap_or(0) != 0;
        const NAMES: [&str; 2] = ["Charged Attack 0", "Charged Attack 1"];
        for (i, mult) in g::E_CHARGE.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Hydro, 25.0, mult[lvl]);
            ai.abil = NAMES[i];
            ai.weak_point = weak;
            ai.add_tags = add_tag::TARTAGLIA_SLASH;
            ai.pattern = s.circle_on_player(0.0, 0.0, 2.2);
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, CB_A4 | CB_SLASH, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let state = s.current_state();
        if s.mod_active(ci, MELEE) {
            let cd_delay = if matches!(state, AnimState::Walk | AnimState::Dash) { 0 } else { 11 };
            exit_melee(s, ci, cd_delay);
            s.chars[ci].normal_counter = 0;
            let t = match state {
                AnimState::Walk => &g::SKILL_MELEE_WALK_FRAMES,
                AnimState::Dash => &g::SKILL_MELEE_DASH_FRAMES,
                _ => &g::SKILL_MELEE_FRAMES,
            };
            return skill_info(t);
        }
        let f = s.f;
        s.st_mut::<State>(ci).e_cast = f;
        s.add_status(ci, MELEE, 30 * 60, true);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 50.0, g::SKILL[lvl]);
        ai.abil = "Foul Legacy: Raging Tide";
        ai.blunt = true;
        ai.poise = 51.75;
        let (hitmark, cd_delay) = match state {
            AnimState::Walk => (g::SKILL_WALK_HITMARK, 0),
            AnimState::Dash => (g::SKILL_DASH_HITMARK, 0),
            _ => (g::SKILL_HITMARK, 14),
        };
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
        s.queue_attack(ai, 0, hitmark, hitmark);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_STANCE_END, arg: f }, 30 * 60);
        s.set_cd_with_delay(ci, Action::Skill, 60, cd_delay);
        let t = match state {
            AnimState::Walk => &g::SKILL_RANGED_WALK_FRAMES,
            AnimState::Dash => &g::SKILL_RANGED_DASH_FRAMES,
            _ => &g::SKILL_RANGED_FRAMES,
        };
        skill_info(t)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let melee = s.mod_active(ci, MELEE);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 50.0, g::BURST[lvl]);
        ai.abil = "Ranged Stance: Flash of Havoc";
        let (t, hitmark, cb) = if melee {
            ai.abil = "Melee Stance: Light of Obliteration";
            ai.mult = g::MELEE_BURST[lvl];
            ai.pattern = s.circle_on_player(0.0, 0.0, 8.0);
            if s.chars[ci].cons >= 6 {
                s.st_mut::<State>(ci).ml_burst_used = true;
            }
            (&g::BURST_MELEE_FRAMES, g::BURST_MELEE_HITMARK, CB_BLAST)
        } else {
            ai.pattern = s.circle_on_target(6.0);
            s.add_task(Task::AddEnergy { c: ci as u8, amt: 20.0 }, 4);
            (&g::BURST_RANGED_FRAMES, g::BURST_RANGED_HITMARK, CB_RANGED_BURST)
        };
        s.queue_attack(ai, cb, hitmark, hitmark);
        if melee {
            s.consume_energy(ci, 71);
            s.set_cd_with_delay(ci, Action::Burst, 900, 66);
        } else {
            s.consume_energy(ci, 3);
            s.set_cd(ci, Action::Burst, 900);
        }
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn dash_info(_: &mut Sim, _: usize, _: ActInfo) -> ActInfo {
        let t = &g::DASH_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Skill as usize], AnimState::Dash)
    }

    fn own_walk(_: &mut Sim, _: usize, p: &Params) -> Option<ActInfo> {
        let f = p.get("f").unwrap_or(1) as i32;
        let len = f.max(g::WALK_FRAMES[Action::Skill as usize]);
        Some(ActInfo::new(Frames::Char { id: FR_WALK, arg: f as i16 }, len, f, AnimState::Walk))
    }

    fn frames(_: u8, arg: i16, next: Action) -> i32 {
        (arg as i32).max(g::WALK_FRAMES[next as usize])
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_STANCE_END => {
                if s.st::<State>(ci).e_cast == arg && s.mod_active(ci, MELEE) {
                    exit_melee(s, ci, 0);
                    s.chars[ci].normal_counter = 0;
                }
            }
            TASK_C4 => c4_tick(s, ci, arg),
            TASK_RT_BURST => {
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 50.0, g::RT_BURST[lvl]);
                ai.abil = "Riptide Burst";
                ai.add_tags = add_tag::TARTAGLIA_SLASH;
                ai.pattern = s.circle_on_target(5.0);
                s.queue_attack(ai, 0, 0, 0);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_FLASH != 0 && ev.info.element == Element::Hydro && s.enemy_status_active(RIPTIDE) && !s.enemy_status_active(RIPTIDE_FLASH_ICD) {
            s.enemy_add_status(RIPTIDE_FLASH_ICD, 42, true);
            flash_tick(s, ci);
        }
        if cbs & CB_AIM_RIPTIDE != 0 && (ev.info.element == Element::Hydro || (s.mod_active(ci, MELEE) && ev.info.element == Element::Physical)) {
            apply_riptide(s, ci);
        }
        if cbs & CB_A4 != 0 && s.chars[ci].ascension >= 4 && ev.crit {
            apply_riptide(s, ci);
        }
        if cbs & CB_RANGED_BURST != 0 {
            apply_riptide(s, ci);
        }
        if cbs & CB_BLAST != 0 && s.enemy_status_active(RIPTIDE) && !s.enemy_status_active(RIPTIDE_SLASH_ICD) {
            let lvl = s.talent(ci, 2);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 50.0, g::RT_BLAST[lvl]);
            ai.abil = "Riptide Blast";
            ai.pattern = s.circle_on_target(5.0);
            s.queue_attack(ai, 0, 1, 1);
            s.enemy_delete_status(RIPTIDE);
        }
        if cbs & CB_SLASH != 0 && s.mod_active(ci, MELEE) && s.enemy_status_active(RIPTIDE) && !s.enemy_status_active(RIPTIDE_SLASH_ICD) {
            s.enemy_add_status(RIPTIDE_SLASH_ICD, 90, true);
            slash_tick(s, ci);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 3 * 60, true);
            s.queue_particle(1.0, Element::Hydro, s.particle_delay[ci]);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match e {
            // onExitField: the stance ends when he leaves the field
            Event::Swap { .. } => {
                if s.mod_active(ci, MELEE) {
                    s.reset_cd(ci, Action::Skill);
                    exit_melee(s, ci, 0);
                }
            }
            // onDefeatTargets: Riptide Burst, and C2's energy
            Event::TargetDied(_) => {
                if !s.enemy_status_active(RIPTIDE) {
                    return;
                }
                s.add_task(Task::Char { c: ci as u8, id: TASK_RT_BURST, arg: 0 }, 5);
                if s.chars[ci].cons >= 2 {
                    s.add_energy(ci, 4.0);
                }
            }
            _ => {}
        }
    }
}

/// The skill's frames: its animation is the table's, and it can be queued
/// after the earliest of its cancels.
fn skill_info(t: &'static FrameTable) -> ActInfo {
    let min = *t.iter().min().unwrap();
    ActInfo::new(Frames::Table(t), t[0], min, AnimState::Skill)
}

/// The melee stance's normal attacks: hydro strikes queued at their
/// hitmarks on his queue, with the areas taken when the attack starts.
fn melee_attack(s: &mut Sim, ci: usize) -> ActInfo {
    let n = s.chars[ci].normal_counter as usize;
    let lvl = s.talent(ci, 1);
    for (i, mult) in g::ATTACK_E[n].iter().enumerate() {
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Hydro, 25.0, mult[lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.hitlag_halt = g::MELEE_HITLAG_HALT_FRAMES[n][i] * 60.0;
        let oy = g::MELEE_OFFSETS[n][i];
        let hb = g::MELEE_HITBOXES[n][i];
        if n == 5 && i == 0 {
            // a spear thrust: not slash
            ai.pattern = s.box_on_player(0.0, oy, hb[0], hb[1]);
        } else {
            ai.add_tags = add_tag::TARTAGLIA_SLASH;
            ai.pattern = s.fan_on_player(0.0, oy, hb[0], g::MELEE_FAN_ANGLES[n]);
        }
        // Riptide can come from the same hit it slashes on
        s.queue_attack_at(ci, ai, CB_A4 | CB_SLASH, g::MELEE_HITMARKS[n][i]);
    }
    let atkspd = s.stat(ci, st::ATKSPD);
    let table = &g::MELEE_FRAMES[n];
    let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::MELEE_HITMARKS[n].last().unwrap(), AnimState::Normal);
    s.advance_normal(ci);
    info
}

/// onExitMeleeStance: the skill's cooldown by the time spent in the stance
/// (C1 shortens it; C6 after a melee burst resets it instead).
fn exit_melee(s: &mut Sim, ci: usize, delay: i32) {
    let t = s.f - s.st::<State>(ci).e_cast;
    let mut cd = if t < 2 * 60 {
        7 * 60
    } else if t < 4 * 60 {
        8 * 60
    } else if t < 5 * 60 {
        9 * 60
    } else if t < 8 * 60 {
        5 * 60 + t
    } else if t < 30 * 60 {
        6 * 60 + t
    } else {
        45 * 60
    };
    if s.chars[ci].cons >= 1 {
        cd = (cd as f64 * 0.8) as i32;
    }
    if s.st::<State>(ci).ml_burst_used {
        s.reset_cd(ci, Action::Skill);
        s.st_mut::<State>(ci).ml_burst_used = false;
    } else {
        s.set_cd_with_delay(ci, Action::Skill, cd, delay);
    }
    s.delete_mod(ci, MELEE);
}

fn apply_riptide(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons >= 4 && !s.enemy_status_active(RIPTIDE) {
        let f = s.f;
        s.st_mut::<State>(ci).c4_src = f;
        s.queue_enemy_task(Task::Char { c: ci as u8, id: TASK_C4, arg: f }, 234);
    }
    let dur = s.st::<State>(ci).riptide_duration;
    s.enemy_add_status(RIPTIDE, dur, true);
}

/// C4: every 3.9 s while Riptide lasts, a Slash in the melee stance, a
/// Flash otherwise, outside their ICDs.
fn c4_tick(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).c4_src != src || !s.enemy_status_active(RIPTIDE) {
        return;
    }
    if s.mod_active(ci, MELEE) {
        slash_tick(s, ci);
    } else {
        flash_tick(s, ci);
    }
    s.queue_enemy_task(Task::Char { c: ci as u8, id: TASK_C4, arg: src }, 234);
}

fn flash_tick(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_TARTAGLIA_RIPTIDE_FLASH, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::RT_FLASH[lvl]);
    ai.abil = "Riptide Flash";
    ai.add_tags = add_tag::TARTAGLIA_SLASH;
    ai.pattern = s.circle_on_target(3.0);
    for _ in 0..3 {
        s.queue_attack(ai, CB_PARTICLE, 1, 1);
    }
}

fn slash_tick(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::RT_SLASH[lvl]);
    ai.abil = "Riptide Slash";
    ai.pattern = s.circle_on_target(3.0);
    s.queue_attack(ai, CB_PARTICLE, 1, 1);
}
