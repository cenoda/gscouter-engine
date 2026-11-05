//! Yanfei, from gcsim's internal/characters/yanfei: normal attacks that
//! give Scarlet Seals (with their `travel`), the charged attack that uses
//! them (its stamina, windup and radius by seal), the skill and the burst
//! that fill them (the burst's seal every second and charged attack DMG),
//! A1's Pyro DMG by seal, A4's Blazing Eye on a crit, every constellation
//! (C2 in damage mode, C4's shield, C6's fourth seal), the seals lost on any
//! swap, and the plunges from Xianyun's airborne state.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gamedata::chars::yanfei as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    seals: i32,
    max_tags: i32,
    stam_reduction: f64,
    /// The seals the last charged attack used (A1's Pyro DMG).
    a1_stacks: i32,
    /// The enemy's HP at the start (C2, damage mode).
    enemy_max_hp: f64,
}

const SEAL: ModKey = ModKey::new(g::SEAL_BUFF_KEY);
const BURST: ModKey = ModKey::new(g::BURST_BUFF_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new("yanfei-a1");
const C2: ModKey = ModKey::new("yanfei-c2");
const C4_SHIELD: ModKey = ModKey::new("yanfei-c4");

const CB_NORMAL_SEAL: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;
const CB_MAX_SEALS: u16 = 1 << 2;
const CB_A4: u16 = 1 << 3;

const TASK_CLEAR_SEALS: u8 = 0;
const TASK_BURST_SEAL: u8 = 1;

const PLUNGE: plunge::Spec = plunge::Spec::catalyst(Element::Pyro, g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

/// The seals the charged attack reads: none once the seals' status ended
/// (gcsim's ActionStam and ChargeAttack clear them then).
fn live_seals(s: &Sim, ci: usize) -> i32 {
    if s.mod_active(ci, SEAL) {
        s.st::<State>(ci).seals
    } else {
        0
    }
}

fn max_seals(s: &mut Sim, ci: usize) {
    let st = s.st_mut::<State>(ci);
    if st.seals < st.max_tags {
        st.seals = st.max_tags;
    }
    s.add_status(ci, SEAL, 600, true);
}

impl Character for Char {
    const KEY: &'static str = "yanfei";
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        State { max_tags: if cs.cons >= 6 { 4 } else { 3 }, stam_reduction: if cs.cons >= 1 { 0.25 } else { 0.15 }, ..Default::default() }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Swap, Owner::Char(ci as u8));
        if s.chars[ci].cons >= 2 && s.damage_mode {
            s.st_mut::<State>(ci).enemy_max_hp = s.enemy_hp;
            s.add_mod(ci, Mod::owned(C2, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1), -1);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack => p.0.iter().find(|(k, _)| k != "travel").map(|(k, _)| format!("{name} param {k}")),
            Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn charge_stam(s: &Sim, ci: usize) -> Option<f64> {
        Some(50.0 * (1.0 - s.st::<State>(ci).stam_reduction * live_seals(s, ci) as f64))
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        (fields[0] == "seal-count").then(|| Ok(Num::Int(s.st::<State>(ci).seals as i64)))
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.circle_on_target(0.75);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, CB_NORMAL_SEAL, h, h + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let seals = live_seals(s, ci);
        s.st_mut::<State>(ci).seals = seals;
        // A1: Pyro DMG by seal used, for 6 s
        if s.chars[ci].ascension >= 1 {
            s.st_mut::<State>(ci).a1_stacks = seals;
            s.add_mod(ci, Mod::owned(A1, ModKind::Stat, Owner::Char(ci as u8), st::PYROP, false, true, -1), 360);
        }
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::CHARGE[seals as usize][lvl]);
        ai.abil = "Charge Attack";
        ai.blunt = true;
        ai.poise = 80.0;
        let windup = if matches!(s.current_state(), AnimState::Idle | AnimState::Swap) { 0 } else { 16 };
        ai.pattern = s.circle_on_target(g::CHARGE_RADIUS[seals as usize]);
        let cbs = if s.chars[ci].ascension >= 4 { CB_A4 } else { 0 };
        let h = g::CHARGE_HITMARK - windup;
        s.queue_attack(ai, cbs, h, h);
        // the seals go the next frame
        s.add_task(Task::Char { c: ci as u8, id: TASK_CLEAR_SEALS, arg: 0 }, 1);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: 0, arg: windup as i16 }, t[0] - windup, t[Action::Jump as usize] - windup, AnimState::Charge)
    }

    fn frames(_: u8, windup: i16, next: Action) -> i32 {
        g::CHARGE_FRAMES[next as usize] - windup as i32
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL[lvl]);
        ai.abil = "Signed Edict";
        ai.blunt = true;
        ai.poise = 120.0;
        ai.pattern = s.circle_on_target(3.5);
        s.queue_attack(ai, CB_PARTICLE | CB_MAX_SEALS, 0, g::SKILL_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 540, 28);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // gcsim adds the status, then an attack mod of the same key, which
        // replaces it: the burst is the attack mod, for 15 s
        s.add_status(ci, BURST, 15 * 60 + 1, true);
        s.add_mod(ci, Mod::owned(BURST, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 15 * 60);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 50.0, g::BURST[lvl]);
        ai.abil = "Done Deal";
        ai.blunt = true;
        ai.poise = 200.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.pattern = s.circle_on_player(0.0, 0.0, 6.5);
        s.queue_attack(ai, CB_MAX_SEALS, 0, g::BURST_HITMARK);
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_SEAL, arg: 0 }, 60);
        // C4: a shield of 45% of her max HP for 15 s
        if s.chars[ci].cons >= 4 {
            let f = s.f;
            let hp = s.max_hp(ci) * 0.45;
            s.add_shield(Shield { owner: ci as u8, key: C4_SHIELD, typ: ShieldType::YanfeiC4, target: -1, ele: Element::Pyro, hp, expiry: f + 15 * 60, em_bonus: 0.0 });
        }
        s.set_cd(ci, Action::Burst, 1200);
        s.consume_energy(ci, 5);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_CLEAR_SEALS => {
                s.st_mut::<State>(ci).seals = 0;
                s.delete_mod(ci, SEAL);
            }
            // a seal every second while the burst lasts
            TASK_BURST_SEAL => {
                if !s.mod_active(ci, BURST) {
                    return;
                }
                let st = s.st_mut::<State>(ci);
                if st.seals < st.max_tags {
                    st.seals += 1;
                }
                s.add_status(ci, SEAL, 600, true);
                s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_SEAL, arg: 0 }, 60);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        // a seal from a normal attack, on the field only
        if cbs & CB_NORMAL_SEAL != 0 && s.player.active as usize == ci {
            let st = s.st_mut::<State>(ci);
            if st.seals < st.max_tags {
                st.seals += 1;
            }
            s.add_status(ci, SEAL, 600, true);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 12, true);
            s.queue_particle(3.0, Element::Pyro, s.particle_delay[ci]);
        }
        if cbs & CB_MAX_SEALS != 0 {
            max_seals(s, ci);
        }
        // A4: Blazing Eye on a crit while on the field
        if cbs & CB_A4 != 0 && s.player.active as usize == ci && ev.crit {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, 0.8);
            ai.abil = "Blazing Eye (A4)";
            ai.hitlag_factor = 0.05;
            ai.def_halt = true;
            ai.pattern = s.circle_on_target(3.5);
            s.queue_attack(ai, 0, 10, 10);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key != A1 {
            return false;
        }
        out[st::PYROP] += s.st::<State>(ci).a1_stacks as f64 * 0.05;
        true
    }

    /// The burst's charged attack DMG; C2's CRIT Rate on an enemy below
    /// half its HP.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag != AttackTag::Extra {
            return;
        }
        if m.key == BURST {
            ev.stats[st::DMGP] += g::BURST_BONUS[s.talent(ci, 2)];
        } else if s.enemy_hp / s.st::<State>(ci).enemy_max_hp < 0.5 {
            ev.stats[st::CR] += 0.20;
        }
    }

    /// Any swap takes her seals.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Swap { .. } = *e {
            s.st_mut::<State>(ci).seals = 0;
            s.delete_mod(ci, SEAL);
        }
    }
}
