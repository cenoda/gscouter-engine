//! Shikanoin Heizou, from gcsim's internal/characters/heizou: normal and
//! charged attacks (a charge only after a normal attack, 25 stamina), the
//! skill pressed or held with its Declension stacks and Conviction, the
//! burst with its aura check and Windmuster Iris, A1's stacks from his
//! swirls, A4's EM for the party, the utility dash's stamina, every
//! constellation, and the plunges from Xianyun's airborne state.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::heizou as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// Declension stacks (0 to 4; 4 is Conviction).
    dec: i32,
    /// The enemies the current burst's aura check tagged (at most 4).
    tagged: i32,
    /// The skill's release has hit a target (gcsim's `done`, per release).
    skill_done: bool,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1_ICD: ModKey = ModKey::new("heizou-a1-icd");
const A4: ModKey = ModKey::new("heizou-a4");
const C1: ModKey = ModKey::new("heizou-c1");
const C1_ICD: ModKey = ModKey::new("heizou-c1-icd");

// the skill's callbacks, in gcsim's order: skillCB, then particleCB
const CB_SKILL: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;
const CB_BURST: u16 = 1 << 2;

const TASK_RELEASE: u8 = 0;
const TASK_DEC_STACK: u8 = 1;

const PLUNGE: plunge::Spec = plunge::Spec::catalyst(Element::Anemo, g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

fn add_dec_stack(s: &mut Sim, ci: usize) {
    let st = s.st_mut::<State>(ci);
    if st.dec < 4 {
        st.dec += 1;
    }
}

/// gcsim's skillHoldDuration: 0.75 s for each stack still to charge.
fn hold_duration(s: &Sim, ci: usize, stacks: i32) -> i32 {
    45 * (stacks - s.st::<State>(ci).dec).clamp(0, 4)
}

/// gcsim's skillRelease: the hit is queued `SKILL_CD_START + delay` frames
/// after the action starts, with the stacks of that moment.
fn release(s: &mut Sim, ci: usize, delay: i32) -> ActInfo {
    s.add_task(Task::Char { c: ci as u8, id: TASK_RELEASE, arg: 0 }, g::SKILL_CD_START + delay);
    let len = delay + g::SKILL_END_FRAMES[0] + g::SKILL_HITMARK;
    let caf = delay + g::SKILL_END_FRAMES[Action::Swap as usize] + g::SKILL_HITMARK;
    ActInfo::new(Frames::Char { id: 0, arg: delay as i16 }, len, caf, AnimState::Skill)
}

fn release_hit(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 1);
    let dec = s.st::<State>(ci).dec;
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 50.0, g::SKILL[lvl] + dec as f64 * g::DEC_BONUS[lvl]);
    ai.abil = "Heartstopper Strike";
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = g::SKILL_HITLAG_HALT_FRAME * 60.0;
    let offset = if dec == 0 { -0.4 } else { -0.3 };
    let (mut w, mut h) = (3.0, 5.0);
    if dec == 4 {
        ai.abil = "Heartstopper Strike (Max Stacks)";
        ai.mult += g::CONVIC_BONUS[lvl];
        ai.hitlag_halt = g::SKILL_HITLAG_MAX_STACK_HALT_FRAME * 60.0;
        w += 1.0;
        h += 1.0;
    }
    s.st_mut::<State>(ci).skill_done = false;
    ai.pattern = s.box_on_player(0.0, offset, w, h);
    let mut ev = s.snapshot_event(ai, 0);
    // C6: CRIT Rate by stack, CRIT DMG with Conviction
    if s.chars[ci].cons >= 6 {
        ev.stats[st::CR] += 0.04 * dec as f64;
        if dec == 4 {
            ev.stats[st::CD] += 0.32;
        }
    }
    s.queue_attack_with_snap(ev, CB_SKILL | CB_PARTICLE, g::SKILL_HITMARK - g::SKILL_CD_START);
    s.set_cd(ci, Action::Skill, 600);
}

/// gcsim's irisDmg: a hit of the first of pyro, hydro, electro and cryo
/// on the enemy, 40 frames later.
fn iris(s: &mut Sim, ci: usize) {
    let Some(ele) = [Element::Pyro, Element::Hydro, Element::Electro, Element::Cryo].into_iter().find(|&e| s.enemy_aura_contains(e)) else { return };
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, ele, 25.0, g::BURST_IRIS[lvl]);
    ai.abil = "Windmuster Iris";
    ai.pattern = s.circle_on_target(2.5);
    s.queue_attack(ai, 0, 0, 40);
}

impl Character for Char {
    const KEY: &'static str = "shikanoinheizou";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::EnemyDamage, Owner::Char(ci as u8));
        }
        // gcsim's "utility-dash" stamina mod, one key for everyone
        s.utility_dash = Some(ci as u8);
        if s.chars[ci].cons >= 1 {
            s.subscribe(EventKind::Swap, Owner::Char(ci as u8));
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Skill => p.0.iter().find(|(k, _)| k != "hold").map(|(k, _)| format!("{name} param {k}")),
            Action::Attack | Action::Charge | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    /// A charge only right after a normal attack, as for swords; plunges
    /// only while airborne.
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::Charge && s.chars[ci].normal_counter == 0 {
            return Some("need to use attack right before charge".into());
        }
        plunge::grounded(s, a)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn charge_stam(_: &Sim, _: usize) -> Option<f64> {
        Some(25.0)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        (fields[0] == "declension").then(|| Ok(Num::Int(s.st::<State>(ci).dec as i64)))
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let (off, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
        let hits = g::ATTACK_HITMARKS[n];
        for (i, &h) in hits.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::ATTACK[n][i][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            ai.pattern = if n == 0 || n == 1 || n == 3 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
            s.queue_attack(ai, 0, h, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hits[hits.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = 0.09 * 60.0;
        ai.pattern = s.box_on_player(0.0, -1.2, 2.8, 3.6);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn frames(_: u8, delay: i16, next: Action) -> i32 {
        delay as i32 + g::SKILL_END_FRAMES[next as usize] + g::SKILL_HITMARK
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if p.get("hold").unwrap_or(0) == 0 {
            return release(s, ci, 0);
        }
        let dec = s.st::<State>(ci).dec;
        if dec == 4 {
            return release(s, ci, g::HOLD_AT_FULL_STACKS_PENALTY);
        }
        for i in dec + 1..=4 {
            let d = hold_duration(s, ci, i);
            s.add_task(Task::Char { c: ci as u8, id: TASK_DEC_STACK, arg: 0 }, d);
        }
        let d = hold_duration(s, ci, 4);
        release(s, ci, d)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.st_mut::<State>(ci).tagged = 0;
        // the aura check, on the enemy only (it skips gadgets in gcsim; a
        // physical hit of no durability does nothing to one anyway)
        let mut check = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 0.0, 0.0);
        check.abil = "Windmuster Iris (Aura check)";
        check.no_impulse = true;
        check.pattern = s.circle_on_target(6.0);
        s.queue_attack(check, CB_BURST, g::BURST_HITMARK, g::BURST_HITMARK);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST[lvl]);
        ai.abil = "Fudou Style Vacuum Slugger";
        ai.pattern = s.circle_on_target(6.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.set_cd(ci, Action::Burst, 720);
        s.consume_energy(ci, 3);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_RELEASE => release_hit(s, ci),
            TASK_DEC_STACK => add_dec_stack(s, ci),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_SKILL != 0 {
            // the stacks go as the release hits; A4 once a release
            let st = s.st_mut::<State>(ci);
            st.dec = 0;
            if !st.skill_done {
                st.skill_done = true;
                if s.chars[ci].ascension >= 4 {
                    for i in 0..s.chars.len() {
                        if i != ci {
                            s.add_stat_mod(i, A4, st::EM, 80.0, 600, true);
                        }
                    }
                }
            }
        }
        // after skillCB has cleared the stacks, so always 2 particles (as
        // in gcsim, whose particleCB reads the stacks then)
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 12, true);
            let dec = s.st::<State>(ci).dec;
            let count = match dec {
                2 | 3 => {
                    if s.rng.float64() < 0.5 {
                        3.0
                    } else {
                        2.0
                    }
                }
                4 => 3.0,
                _ => 2.0,
            };
            s.queue_particle(count, Element::Anemo, s.particle_delay[ci]);
        }
        if cbs & CB_BURST != 0 {
            let st = s.st_mut::<State>(ci);
            if st.tagged == 4 {
                return;
            }
            st.tagged += 1;
            let n = st.tagged;
            // C4: 9 energy for the first iris, 1.5 for each other
            if s.chars[ci].cons >= 4 {
                s.add_energy(ci, if n == 1 { 9.0 } else { 1.5 });
            }
            iris(s, ci);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // A1: a stack for his swirl while on the field, once every 0.1 s
            Event::EnemyDamage(ev) => {
                if s.mod_active(ci, A1_ICD) || ev.info.actor as usize != ci || s.player.active as usize != ci {
                    return;
                }
                if !matches!(ev.info.tag, AttackTag::SwirlPyro | AttackTag::SwirlHydro | AttackTag::SwirlElectro | AttackTag::SwirlCryo | AttackTag::DirectStellarSwirl | AttackTag::ReactionStellarSwirl) {
                    return;
                }
                s.add_status(ci, A1_ICD, 6, true);
                add_dec_stack(s, ci);
            }
            // C1: ATK SPD and a stack as he takes the field, once every 10 s
            Event::Swap { next, .. } => {
                if s.mod_active(ci, C1_ICD) || next != ci {
                    return;
                }
                s.add_stat_mod(ci, C1, st::ATKSPD, 0.15, 300, true);
                add_dec_stack(s, ci);
                s.add_status(ci, C1_ICD, 600, true);
            }
            _ => {}
        }
    }
}
