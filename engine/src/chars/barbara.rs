//! Barbara, from gcsim's internal/characters/barbara: normal and charged
//! attacks with the Melody Loop's heals, the skill (its droplets, the
//! Melody Loop's heal and wet ticks and its 0 DMG hydro ticks), the
//! burst's heal, her own dash frames, A1's stamina cut, A4's extension,
//! C1's energy, C2's hydro DMG bonus and cooldown, C4's energy and C6's
//! revival (which only lifts an active character at 0 HP to 1). Plunges
//! from Xianyun's airborne state.

use crate::chars::plunge;
use crate::content::{Character, Event, EventKind, Owner};
use crate::gamedata::chars::barbara as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The frame of the last skill (gcsim's skillInitF).
    skill_init_f: i32,
    /// The Melody Loop's tick heal and the Healing Bonus, at the skill.
    tick_heal: f64,
    tick_bonus: f64,
    /// A4's extensions in this Melody Loop.
    a4_count: i32,
    /// The burst's Healing Bonus, at the burst.
    burst_bonus: f64,
    /// C6's cooldown: the frame it is ready again (0 at first).
    c6_icd: i32,
}

const SKILL: ModKey = ModKey::new(g::BARB_SKILL_KEY);
const C2: ModKey = ModKey::new("barbara-c2");

const CB_HEAL_NORMAL: u16 = 1 << 0;
const CB_HEAL_CHARGE: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;

/// arg: the skill's frame.
const TASK_SKILL_START: u8 = 0;
const TASK_SELF_TICK: u8 = 1;
const TASK_MELODY_TICK: u8 = 2;
const TASK_BURST_HEAL: u8 = 3;
const TASK_C1: u8 = 4;

const FRAMES_CHARGE: u8 = 0;

const PLUNGE: plunge::Spec = plunge::Spec::catalyst(Element::Hydro, g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];

fn skill_active(s: &Sim) -> bool {
    s.core_status_key(SKILL) > 0
}

/// The Melody Loop's heal on a normal attack's hit (times 4 on a charged
/// attack's).
fn proc_heal(s: &Sim, ci: usize) -> f64 {
    let lvl = s.talent(ci, 1);
    g::PROCHPP[lvl] * s.max_hp(ci) + g::PROCHP[lvl]
}

impl Character for Char {
    const KEY: &'static str = "barbara";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::ParticleReceived, o);
        }
        if s.chars[ci].cons >= 1 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_C1, arg: 0 }, 1 + 10 * 60);
        }
        if s.chars[ci].cons >= 6 {
            s.subscribe(EventKind::Drain, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// Her NextQueueItemIsValid: no charged attack right after a fourth
    /// normal attack. Plunges only while airborne.
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        if let Some(why) = plunge::grounded(s, a) {
            return Some(why);
        }
        let active = &s.chars[s.player.active as usize];
        if a == Action::Charge && s.player.last.action == Action::Attack && active.normal_counter == 0 {
            return Some("need to use attack right before charge".into());
        }
        crate::content::template_invalid(s, a)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.circle_on_target(g::ATTACK_RADIUS[n]);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, CB_HEAL_NORMAL, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], table[Action::Dash as usize], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.pattern = s.circle_on_player(0.0, 5.0, 3.0);
        // the windup is skipped out of a normal attack
        let windup = if s.current_state() == AnimState::Normal { 14 } else { 0 };
        let cbs = CB_HEAL_CHARGE | if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
        let h = g::CHARGE_HITMARK - windup;
        s.queue_attack(ai, cbs, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: FRAMES_CHARGE, arg: windup as i16 }, t[0] - windup, h, AnimState::Charge)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FRAMES_CHARGE => g::CHARGE_FRAMES[next as usize] - arg as i32,
            _ => unreachable!(),
        }
    }

    fn dash_info(_: &mut Sim, _: usize, _: ActInfo) -> ActInfo {
        let t = &g::DASH_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Burst as usize], AnimState::Dash)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.st_mut::<State>(ci).a4_count = 0;
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::SKILL[lvl]);
        ai.abil = "Let the Show Begin♪ (Droplet)";
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
        for &h in g::SKILL_HITMARKS {
            s.queue_attack(ai, 0, 5, h);
        }
        let f = s.f;
        let bonus = s.stats_pruning(ci)[st::HEAL];
        let heal = g::SKILLHP[lvl] + g::SKILLHPP[lvl] * s.max_hp(ci);
        {
            let st = s.st_mut::<State>(ci);
            st.skill_init_f = f;
            st.tick_heal = heal;
            st.tick_bonus = bonus;
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_START, arg: f }, g::SKILL_CD_START);
        if s.chars[ci].cons >= 2 {
            // C2: hydro DMG +15% for the others during the skill
            for i in 0..s.chars.len() {
                if i != ci {
                    s.add_mod(i, Mod::stat(C2, st::HYDROP, 0.15, false, false, -1), g::SKILL_DURATION);
                }
            }
            s.set_cd_with_delay(ci, Action::Skill, 1632, g::SKILL_CD_START);
        } else {
            s.set_cd_with_delay(ci, Action::Skill, 32 * 60, g::SKILL_CD_START);
        }
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let bonus = s.stats_pruning(ci)[st::HEAL];
        s.st_mut::<State>(ci).burst_bonus = bonus;
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_HEAL, arg: 0 }, 77);
        s.consume_energy(ci, 6);
        s.set_cd_with_delay(ci, Action::Burst, 20 * 60, 1);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Charge as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL_START => {
                s.add_core_status(SKILL, g::SKILL_DURATION);
                // A1: stamina -12% during the Melody Loop
                if s.chars[ci].ascension >= 1 {
                    s.barbara_a1 = s.f + g::SKILL_DURATION;
                }
                Self::task(s, ci, TASK_SELF_TICK, arg);
                Self::task(s, ci, TASK_MELODY_TICK, arg);
            }
            TASK_SELF_TICK => {
                if s.st::<State>(ci).skill_init_f != arg || !skill_active(s) {
                    return;
                }
                let (heal, bonus) = (s.st::<State>(ci).tick_heal, s.st::<State>(ci).tick_bonus);
                let active = s.player.active as usize;
                s.heal(ci, active, heal, bonus);
                // wet: hydro on the player for 0.3 s
                s.apply_self_infusion(AURA_HYDRO, 25.0, 18);
                s.add_task(Task::Char { c: ci as u8, id: TASK_SELF_TICK, arg }, 5 * 60);
            }
            TASK_MELODY_TICK => {
                if s.st::<State>(ci).skill_init_f != arg || !skill_active(s) {
                    return;
                }
                // a 0 DMG hydro hit, not snapshot, that makes hitlag on
                // the enemy only
                let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
                ai.abil = "Let the Show Begin♪ (Melody Loop)";
                ai.hitlag_factor = 0.05;
                ai.hitlag_halt = 0.05 * 60.0;
                ai.def_halt = true;
                ai.deployable = true;
                ai.pattern = s.circle_on_player(0.0, 0.0, 1.0);
                s.queue_attack(ai, 0, -1, 0);
                s.add_task(Task::Char { c: ci as u8, id: TASK_MELODY_TICK, arg }, 90);
            }
            TASK_BURST_HEAL => {
                let lvl = s.talent(ci, 2);
                let heal = g::BURSTHP[lvl] + g::BURSTHPP[lvl] * s.max_hp(ci);
                let bonus = s.st::<State>(ci).burst_bonus;
                s.heal_info(ci, None, false, heal, bonus);
            }
            TASK_C1 => {
                s.add_energy(ci, 1.0);
                s.add_task(Task::Char { c: ci as u8, id: TASK_C1, arg: 0 }, 10 * 60);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & (CB_HEAL_NORMAL | CB_HEAL_CHARGE) != 0 && skill_active(s) {
            let mut heal = proc_heal(s, ci);
            if cbs & CB_HEAL_CHARGE != 0 {
                heal *= 4.0;
            }
            let bonus = s.stat(ci, st::HEAL);
            s.heal_info(ci, None, false, heal, bonus);
        }
        // C4: a charged attack's hit during the skill gives 1 energy
        if cbs & CB_C4 != 0 && skill_active(s) {
            s.add_energy(ci, 1.0);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // A4: a particle extends the Melody Loop by 1 s, 5 times at most
            Event::ParticleReceived => {
                if !skill_active(s) || s.st::<State>(ci).a4_count == 5 {
                    return;
                }
                s.st_mut::<State>(ci).a4_count += 1;
                s.extend_core_status(SKILL, 60);
            }
            // C6: a drain off field revives the active character at 0 HP
            Event::Drain { amount, .. } if amount > 0.0 => {
                if s.player.active as usize == ci {
                    return;
                }
                let icd = s.st::<State>(ci).c6_icd;
                if s.f < icd && icd != 0 {
                    return;
                }
                let active = s.player.active as usize;
                if s.current_hp(active) / s.max_hp(active) <= 0.0 {
                    s.set_hp(active, 1.0);
                }
                s.st_mut::<State>(ci).c6_icd = s.f + 60 * 60 * 15;
            }
            _ => {}
        }
    }
}
