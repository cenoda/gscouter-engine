//! Yoimiya, from gcsim's internal/characters/yoimiya: normal attacks
//! (with the skill's pyro and multiplier, applied as they snapshot), the
//! skill, the burst and Aurous Blaze, A1, A4, C2, C4 and C6. C1 adds ATK
//! when an enemy with Aurous Blaze dies, which no fight here has, so only
//! its longer Aurous Blaze applies. No aimed shots.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::yoimiya as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    a1_stacks: i32,
    /// Aurous Blaze went on the enemy since the last burst.
    ab_applied: bool,
}

const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const A4: ModKey = ModKey::new("yoimiya-a4");
const C2: ModKey = ModKey::new("yoimiya-c2");
const AB: ModKey = ModKey::new(g::AB_DEBUFF);
const AB_ICD: ModKey = ModKey::new(g::AB_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_A1: u16 = 1 << 1;
const CB_C2: u16 = 1 << 2;
const CB_AB: u16 = 1 << 3;

const TASK_A4: u8 = 0;

const NORMAL_TRAVEL: i32 = 10;

impl Character for Char {
    const KEY: &'static str = "yoimiya";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Swap, Owner::Char(ci as u8));
        s.subscribe(EventKind::EnemyDamage, Owner::Char(ci as u8));
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// The skill turns normal attacks pyro and multiplies them as they
    /// snapshot.
    fn snapshot(s: &Sim, ci: usize, info: &mut AttackInfo) {
        if info.tag == AttackTag::Normal && s.mod_active(ci, SKILL) {
            info.element = Element::Pyro;
            info.mult *= g::SKILL[s.talent(ci, 1)];
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(NORMAL_TRAVEL, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let skill = s.mod_active(ci, SKILL);
        let icd_tag = if skill { icd::TAG_NORMAL_ATTACK } else { icd::TAG_NONE };
        let cb_a1 = if s.chars[ci].ascension >= 1 { CB_A1 } else { 0 };
        let cb_c2 = if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
        let cbs = if skill { CB_PARTICLE } else { 0 } | cb_a1 | cb_c2;
        let mut total_mv = 0.0;
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd_tag, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
            total_mv += mult[lvl];
            let h = g::ATTACK_HITMARKS[n][i];
            s.queue_attack(ai, cbs, h, h + travel);
        }
        if s.chars[ci].cons >= 6 && skill && s.rng.float64() < 0.5 {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Pyro, 25.0, total_mv * 0.6);
            ai.abil = ["Kindling (C6) - N0", "Kindling (C6) - N1", "Kindling (C6) - N2", "Kindling (C6) - N3", "Kindling (C6) - N4"][n];
            ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
            s.queue_attack(ai, cb_a1 | cb_c2, 0, g::ATTACK_HITMARKS[n][0] + travel + 5);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_status(ci, SKILL, 600 + g::SKILL_START, true);
        if !s.mod_active(ci, A1) {
            s.st_mut::<State>(ci).a1_stacks = 0;
        }
        s.set_cd_with_delay(ci, Action::Skill, 1080, 11);
        ActInfo::new(Frames::Table(&g::SKILL_FRAMES), g::SKILL_FRAMES[0], g::SKILL_FRAMES[Action::Attack as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Pyro, 50.0, g::BURST[lvl]);
        ai.abil = "Aurous Blaze (Initial)";
        ai.blunt = true;
        ai.poise = 220.0;
        ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
        if s.chars[ci].ascension >= 4 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, g::BURST_HITMARK);
        }
        let cb_c2 = if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
        s.queue_attack(ai, CB_AB | cb_c2, 0, g::BURST_HITMARK);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 5);
        s.st_mut::<State>(ci).ab_applied = false;
        ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_FRAMES[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_A4 => {
                let v = 0.1 + s.st::<State>(ci).a1_stacks as f64 * 0.01;
                for i in 0..s.chars.len() {
                    if i != ci {
                        s.add_stat_mod(i, A4, st::ATKP, v, 900, true);
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        // A1's stacks, read as the stat is
        out[st::PYROP] += s.st::<State>(ci).a1_stacks as f64 * 0.02;
        true
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 2 * 60, true);
            s.queue_particle(1.0, Element::Pyro, s.particle_delay[ci]);
        }
        if cbs & CB_A1 != 0 && s.player.active as usize == ci && s.mod_active(ci, SKILL) {
            if !s.mod_active(ci, A1) {
                s.st_mut::<State>(ci).a1_stacks = 0;
            }
            let st = s.st_mut::<State>(ci);
            if st.a1_stacks < 10 {
                st.a1_stacks += 1;
            }
            s.add_mod(ci, Mod::owned(A1, ModKind::Stat, Owner::Char(ci as u8), st::PYROP, false, false, -1), 3 * 60);
        }
        if cbs & CB_C2 != 0 && ev.crit && ev.info.element == Element::Pyro {
            s.add_stat_mod(ci, C2, st::PYROP, 0.25, 360, false);
        }
        if cbs & CB_AB != 0 && !s.st::<State>(ci).ab_applied {
            s.st_mut::<State>(ci).ab_applied = true;
            let dur = if s.chars[ci].cons >= 1 { 840 } else { 600 };
            s.enemy_add_status(AB, dur, true);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { prev, next } => {
                if prev == ci && next != ci {
                    s.delete_mod(ci, SKILL);
                }
            }
            Event::EnemyDamage(ev) => {
                if !s.enemy_status_active(AB) || ev.info.actor as usize == ci || s.enemy_status_active(AB_ICD) {
                    return;
                }
                if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge | AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst) {
                    return;
                }
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BURST_EXPLODE[lvl]);
                ai.abil = "Aurous Blaze (Explode)";
                ai.blunt = true;
                ai.poise = 60.0;
                ai.pattern = s.circle_on_target(3.0);
                let cb_c2 = if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
                s.queue_attack(ai, cb_c2, 0, 1);
                s.enemy_add_status(AB_ICD, 120, true);
                if s.chars[ci].cons >= 4 {
                    s.reduce_cd(ci, Action::Skill, 72);
                }
            }
            _ => {}
        }
    }
}
