//! Diona, from gcsim's internal/characters/diona: normal attacks, the
//! skill's paws and their shield (pressed or held), the burst's field with
//! its heals, A1 and every constellation (C4 only speeds up aimed shots,
//! which are not ported). gcsim does not implement A4.

use crate::content::Character;
use crate::gcsimdata::chars::diona as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The skill that last made or refreshed the shield.
    shield_src: i32,
    /// The skill now firing: its frame, shield HP and paw duration.
    skill_src: i32,
    skill_hp: f64,
    skill_dur: i32,
    burst: Option<AttackEvent>,
    heal: f64,
    heal_bonus: f64,
}

const SHIELD: ModKey = ModKey::new("diona-shield");
const BURST: ModKey = ModKey::new("diona-q");
const C2: ModKey = ModKey::new("diona-c2");
const C6: ModKey = ModKey::new("diona-c6");
const C6_HEAL: ModKey = ModKey::new("diona-c6-healbonus");

const CB_PAW: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;

const TASK_FIELD: u8 = 0;
const TASK_TICK: u8 = 1;
const TASK_C1: u8 = 2;
const TASK_C6: u8 = 3;

/// The burst's field, a circle of 7 on the enemy: does it hold the player?
fn in_field(s: &Sim) -> bool {
    s.player_in_circle(s.target_dist, 7.0)
}

impl Character for Char {
    const KEY: &'static str = "diona";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        // A1: stamina -10% while her shield is up
        if s.chars[ci].ascension >= 1 {
            s.diona_a1 = true;
        }
        if s.chars[ci].cons >= 2 {
            s.chars[ci].mods.push(Mod::owned(C2, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1));
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::AUTO[n][lvl]);
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

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let hold = p.get("hold") == Some(1);
        let (hitmark, paws) = if hold { (g::SKILL_HOLD_HITMARK, 5) } else { (g::SKILL_PRESS_HITMARK, 2) };
        let lvl = s.talent(ci, 1);
        let mut hp = (g::PAW_SHIELD_PER[lvl] * s.max_hp(ci) + g::PAW_SHIELD_FLAT[lvl]) * if hold { 1.75 } else { 1.0 };
        if s.chars[ci].cons >= 2 {
            hp *= 1.15;
        }
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        st.skill_src = f;
        st.skill_hp = hp;
        st.skill_dur = (g::PAW_DUR[lvl] * 60.0) as i32;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::PAW[lvl]);
        ai.abil = "Icy Paw";
        ai.pattern = s.circle_on_target(0.5);
        for i in 0..paws {
            s.queue_attack(ai, CB_PAW | CB_PARTICLE, 0, travel + hitmark - 5 + i);
        }
        if hold {
            s.set_cd_with_delay(ci, Action::Skill, 900, hitmark);
            let t = &g::SKILL_HOLD_FRAMES;
            ActInfo::new(Frames::Table(t), t[0], hitmark, AnimState::Skill)
        } else {
            s.set_cd_with_delay(ci, Action::Skill, 360, hitmark);
            let t = &g::SKILL_PRESS_FRAMES;
            ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
        }
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST[lvl]);
        ai.abil = "Signature Mix (Initial)";
        ai.pattern = s.circle_on_target(3.0);
        s.queue_attack(ai, 0, 0, g::BURST_START);
        ai.abil = "Signature Mix (Tick)";
        ai.mult = g::BURST_DOT[lvl];
        ai.pattern = s.circle_on_target(6.5);
        let ev = s.snapshot_event(ai, 0);
        let heal = g::BURST_HEAL_PER[lvl] * s.max_hp(ci) + g::BURST_HEAL_FLAT[lvl];
        let st = s.st_mut::<State>(ci);
        st.burst = Some(ev);
        st.heal = heal;
        st.heal_bonus = ev.stats[st::HEAL];
        s.add_core_status(BURST, 750 + g::BURST_START);
        s.add_task(Task::Char { c: ci as u8, id: TASK_FIELD, arg: 0 }, g::BURST_START);
        // C1: 15 energy as it ends
        if s.chars[ci].cons >= 1 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_C1, arg: 0 }, g::BURST_START + 750);
        }
        s.set_cd_with_delay(ci, Action::Burst, 1200, 41);
        s.consume_energy(ci, 43);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            // the field lands: ticks every 2 s, six times
            TASK_FIELD => {
                for i in 0..6 {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg: 0 }, 120 + i * 120);
                }
                if s.chars[ci].cons >= 6 {
                    let mut i = 30;
                    while i <= 750 {
                        s.add_task(Task::Char { c: ci as u8, id: TASK_C6, arg: 0 }, i);
                        i += 30;
                    }
                }
            }
            TASK_TICK => {
                let st = s.st::<State>(ci);
                let (Some(ev), heal, bonus) = (st.burst, st.heal, st.heal_bonus) else { return };
                s.queue_attack_with_snap(ev, 0, 0);
                if in_field(s) {
                    let active = s.player.active as usize;
                    s.heal(ci, active, heal, bonus);
                }
            }
            TASK_C1 => s.add_energy(ci, 15.0),
            // C6: in the field, 200 EM for the active character above half
            // HP, else +30% incoming healing
            TASK_C6 => {
                if !in_field(s) {
                    return;
                }
                let active = s.player.active as usize;
                if s.chars[active].hp_ratio > 0.5 {
                    s.add_stat_mod(active, C6, st::EM, 200.0, 120, true);
                } else {
                    s.add_mod(active, Mod::owned(C6_HEAL, ModKind::HealBonus, Owner::Char(ci as u8), st::NONE, false, true, -1), 120);
                }
            }
            _ => unreachable!(),
        }
    }

    fn mod_amount(_: &Sim, _: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C6_HEAL {
            out[st::HEAL] += 0.3;
            return true;
        }
        false
    }

    /// C2: skill DMG +15%.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::ElementalArt {
            ev.stats[st::DMGP] += 0.15;
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        // each paw makes or lengthens the shield; the skill's first paw
        // to land also renews its HP
        if cbs & CB_PAW != 0 {
            let st = s.st::<State>(ci);
            let (src, hp, dur) = (st.skill_src, st.skill_hp, st.skill_dur);
            let f = s.f;
            let mut sh = match s.shields.iter().find(|x| x.typ == ShieldType::DionaSkill) {
                Some(&x) => {
                    let mut x = x;
                    if s.st::<State>(ci).shield_src != src {
                        x.hp = hp;
                        x.expiry += dur;
                    }
                    x.expiry += dur;
                    x
                }
                None => Shield { owner: ci as u8, key: SHIELD, typ: ShieldType::DionaSkill, target: -1, ele: Element::Cryo, hp, expiry: f + dur, em_bonus: 0.0 },
            };
            s.st_mut::<State>(ci).shield_src = src;
            sh.expiry = sh.expiry.min(f + dur * 5);
            s.add_shield(sh);
        }
        // each paw, 80% of the time, a particle
        if cbs & CB_PARTICLE != 0 && s.rng.float64() < 0.8 {
            s.queue_particle(1.0, Element::Cryo, s.particle_delay[ci]);
        }
    }
}
