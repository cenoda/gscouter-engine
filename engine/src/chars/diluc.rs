//! Diluc, from gcsim's internal/characters/diluc: normal attacks, the
//! three-part skill, the burst and its infusion, A4 and every
//! constellation. C2 needs Diluc to be hit, which no fight here does.
//! Plunges need Xianyun.

use std::cell::Cell;

use crate::content::Character;
use crate::gcsimdata::chars::diluc as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    e_counter: usize,
    saved_normal: i32,
    /// C6's boosted normal attack hits, counted as its attack mod is read.
    c6_count: Cell<i32>,
    /// The enemy's HP at the start, for C1.
    enemy_max_hp: f64,
}

const E_WINDOW: ModKey = ModKey::new(g::E_WINDOW_KEY);
const BURST: ModKey = ModKey::new(g::BURST_BUFF_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const C1: ModKey = ModKey::new("diluc-c1");
const C4: ModKey = ModKey::new(g::C4_BUFF_KEY);
const C6_DMG: ModKey = ModKey::new("diluc-c6-dmg");
const C6_SPEED: ModKey = ModKey::new("diluc-c6-speed");

const CB_PARTICLE: u16 = 1 << 0;

const TASK_C4: u8 = 0;
const TASK_BURST: u8 = 1;

const SKILL_NAMES: [&str; 3] = ["Searing Onslaught 0", "Searing Onslaught 1", "Searing Onslaught 2"];

impl Character for Char {
    const KEY: &'static str = "diluc";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].cons >= 1 && s.damage_mode {
            s.st_mut::<State>(ci).enemy_max_hp = s.enemy_hp;
            s.chars[ci].mods.push(Mod::owned(C1, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1));
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// The skill can follow up while its window lasts, whatever its
    /// cooldown.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && s.mod_active(ci, E_WINDOW)).then_some(None)
    }

    /// The burst's pyro, which nothing overrides.
    fn snapshot(s: &Sim, ci: usize, info: &mut AttackInfo) {
        if s.mod_active(ci, BURST) && matches!(info.tag, AttackTag::Normal | AttackTag::Plunge | AttackTag::Extra) {
            info.element = Element::Pyro;
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // C6: the skill keeps the combo
        if s.chars[ci].cons >= 6 && s.current_state() == AnimState::Skill {
            s.chars[ci].normal_counter = s.st::<State>(ci).saved_normal;
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.blunt = true;
        ai.poise = g::ATTACK_POISE_DMG[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        s.st_mut::<State>(ci).saved_normal = s.chars[ci].normal_counter;
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if !s.mod_active(ci, E_WINDOW) {
            s.st_mut::<State>(ci).e_counter = 0;
        }
        if s.chars[ci].cons >= 6 {
            s.st::<State>(ci).c6_count.set(0);
            s.add_mod(ci, Mod::owned(C6_DMG, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 360);
            s.add_mod(ci, Mod::owned(C6_SPEED, ModKind::Stat, Owner::Char(ci as u8), st::ATKSPD, false, true, -1), 360);
        }
        let e = s.st::<State>(ci).e_counter;
        let h = g::SKILL_HITMARKS[e];
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL[e][lvl]);
        ai.abil = SKILL_NAMES[e];
        ai.blunt = true;
        ai.poise = 120.0;
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::SKILL_HITLAG_STAGES[e] * 60.0;
        ai.def_halt = true;
        s.queue_attack(ai, CB_PARTICLE, h, h);
        if s.chars[ci].cons >= 4 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_C4, arg: 0 }, h + 120);
        }
        s.add_status(ci, E_WINDOW, 4 * 60, true);
        let next = e + 1;
        s.st_mut::<State>(ci).e_counter = next;
        match next {
            1 => s.set_cd(ci, Action::Skill, 10 * 60),
            3 => {
                s.delete_mod(ci, E_WINDOW);
                s.st_mut::<State>(ci).e_counter = 0;
            }
            _ => {}
        }
        let t = &g::SKILL_FRAMES[e];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let a4 = s.chars[ci].ascension >= 4;
        let duration = if a4 { 480 + 240 } else { 480 };
        s.add_status(ci, BURST, duration, true);
        if a4 {
            // the A4 bonus shares the infusion's key and replaces it
            s.add_stat_mod(ci, BURST, st::PYROP, 0.2, duration, true);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST, arg: 0 }, g::BURST_HITMARK);
        s.consume_energy(ci, 21);
        s.set_cd_with_delay(ci, Action::Burst, 720, 14);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_C4 => s.add_mod(ci, Mod::owned(C4, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 120),
            TASK_BURST => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DILUC, Element::Pyro, 50.0, g::BURST_INITIAL[lvl]);
                ai.abil = "Dawn (Strike)";
                ai.blunt = true;
                ai.poise = 100.0;
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = 0.09 * 60.0;
                ai.def_halt = true;
                s.queue_attack(ai, 0, 0, 1);
                ai.blunt = false;
                ai.abil = "Dawn (Tick)";
                ai.mult = g::BURST_DOT[lvl];
                ai.hitlag_halt = 0.0;
                ai.def_halt = false;
                // the phoenix flies away from the player: 8 ticks, a box 16
                // wide and 8 long 1 + 2.8 i ahead less 5, then the
                // explosion's 10 long 1 + 14 * 1.7 ahead less 6
                for i in 0..8 {
                    if s.box_hits(1.0 + 2.8 * i as f64 - 5.0, 16.0, 8.0) {
                        s.queue_attack(ai, 0, 0, (i + 1) * 12);
                    }
                }
                ai.abil = "Dawn (Explode)";
                ai.mult = g::BURST_EXPLODE[lvl];
                if s.box_hits(1.0 + 14.0 * 1.7 - 6.0, 16.0, 10.0) {
                    s.queue_attack(ai, 0, 0, (1.7 * 60.0) as i32);
                }
            }
            _ => unreachable!(),
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        // C6's attack speed, during normal attacks, for two of them
        if s.current_state() != AnimState::Normal || s.st::<State>(ci).c6_count.get() > 1 {
            return false;
        }
        out[st::ATKSPD] += 0.3;
        true
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C1 {
            if s.enemy_hp / s.st::<State>(ci).enemy_max_hp > 0.5 {
                ev.stats[st::DMGP] += 0.15;
            }
        } else if m.key == C4 {
            if ev.info.tag == AttackTag::ElementalArt {
                ev.stats[st::DMGP] += 0.4;
            }
        } else if ev.info.tag == AttackTag::Normal {
            // C6: counted as it is read
            let st = s.st::<State>(ci);
            if st.c6_count.get() <= 1 {
                st.c6_count.set(st.c6_count.get() + 1);
                ev.stats[st::DMGP] += 0.3;
            }
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, true);
            let count = if s.rng.float64() < 0.33 { 2.0 } else { 1.0 };
            s.queue_particle(count, Element::Pyro, s.particle_delay[ci]);
        }
    }
}
