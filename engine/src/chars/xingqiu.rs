//! Xingqiu, from gcsim's internal/characters/xingqiu.

use super::watch::Watcher;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::xingqiu as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    num_swords: i32,
    next_regen: bool,
    orbital_active: bool,
    watch_src: i32,
}

const A4: ModKey = ModKey::new("xingqiu-a4");
const ORBITAL: ModKey = ModKey::new(g::ORBITAL_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const BURST_ICD: ModKey = ModKey::new(g::BURST_ICD_KEY);
const C2_RES: ModKey = ModKey::new("xingqiu-c2");

const CB_PARTICLE: u16 = 1 << 0;
const CB_C2: u16 = 1 << 1;
const CB_C6: u16 = 1 << 2;

const TASK_ORBITAL: u8 = 0;
const TASK_C2_RES: u8 = 1;
const TASK_TRIGGER: u8 = 2;
const TASK_TICK: u8 = 3;

const WATCH: Watcher = Watcher { status: BURST, icd: BURST_ICD, delay_key: 2, trigger_task: TASK_TRIGGER, tick_task: TASK_TICK, fire: sword_wave };

impl Character for Char {
    const KEY: &'static str = "xingqiu";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].ascension >= 4 {
            s.chars[ci].mods.push(Mod::stat(A4, st::HYDROP, 0.2, false, false, -1));
        }
        s.subscribe(EventKind::StateChange, Owner::Char(ci as u8));
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAMES[n] * 60.0;
            ai.hitlag_factor = 0.01;
            ai.def_halt = true;
            s.queue_attack_at(ci, ai, 0, g::ATTACK_HITMARKS[n][i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let c4 = s.chars[ci].cons >= 4 && s.mod_active(ci, BURST);
        for (i, v) in g::RAINSCREEN.iter().enumerate() {
            let mut mult = v[lvl];
            if c4 {
                mult *= 1.5;
            }
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, mult);
            ai.abil = ["Guhua Sword: Fatal Rainscreen 0", "Guhua Sword: Fatal Rainscreen 1"][i];
            ai.hitlag_halt = 0.02 * 60.0;
            ai.hitlag_factor = 0.01;
            ai.def_halt = true;
            s.queue_attack_at(ci, ai, CB_PARTICLE, g::SKILL_HITMARKS[i]);
        }
        apply_orbital(s, ci, 15 * 60, 43);
        s.set_cd_with_delay(ci, Action::Skill, 21 * 60, 10);
        ActInfo::new(Frames::Table(&g::SKILL_FRAMES), g::SKILL_FRAMES[0], g::SKILL_FRAMES[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mut dur = 15;
        if s.chars[ci].cons >= 2 {
            dur += 3;
        }
        dur *= 60;
        s.add_status(ci, BURST, dur + 33, true);
        apply_orbital(s, ci, dur, g::BURST_HITMARK);
        let st = s.st_mut::<State>(ci);
        st.num_swords = 2;
        st.next_regen = false;
        s.set_cd(ci, Action::Burst, 20 * 60);
        s.consume_energy(ci, 3);
        ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_HITMARK, AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_ORBITAL => orbital_tick(s, ci),
            TASK_C2_RES => s.enemy_add_resist(C2_RES, Element::Hydro, -0.15, 240),
            TASK_TRIGGER => {
                if let Some(src) = WATCH.trigger(s, ci) {
                    s.st_mut::<State>(ci).watch_src = src;
                }
            }
            TASK_TICK => {
                let current = s.st::<State>(ci).watch_src;
                WATCH.tick(s, ci, arg, current);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16) {
        if cbs & CB_PARTICLE != 0 {
            particle_cb(s, ci);
        }
        if cbs & CB_C2 != 0 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_C2_RES, arg: 0 }, 1);
        }
        if cbs & CB_C6 != 0 {
            s.add_energy(ci, 3.0);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::StateChange(next) = *e {
            if let Some(src) = WATCH.on_state(s, ci, next) {
                s.st_mut::<State>(ci).watch_src = src;
            }
        }
    }
}

fn particle_cb(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, PARTICLE_ICD) {
        return;
    }
    s.add_status(ci, PARTICLE_ICD, 60, true);
    s.queue_particle(5.0, Element::Hydro, s.particle_delay[ci]);
}

fn apply_orbital(s: &mut Sim, ci: usize, duration: i32, delay: i32) {
    if !s.st::<State>(ci).orbital_active {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ORBITAL, arg: 0 }, delay);
        s.st_mut::<State>(ci).orbital_active = true;
    }
    s.add_status(ci, ORBITAL, duration, true);
}

fn orbital_tick(s: &mut Sim, ci: usize) {
    if !s.mod_active(ci, ORBITAL) {
        s.st_mut::<State>(ci).orbital_active = false;
        return;
    }
    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
    ai.abil = "Xingqiu Orbital";
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ORBITAL, arg: 0 }, 135);
    s.queue_attack(ai, 0, -1, 1);
}

fn sword_wave(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::BURST[lvl]);
    ai.abil = "Guhua Sword: Raincutter";
    let (stats, lvl) = s.make_snapshot(&mut ai);
    let mut ev = AttackEvent::new(ai, 0);
    ev.stats = stats;
    ev.char_lvl = lvl;
    let (num_swords, next_regen) = {
        let st = s.st::<State>(ci);
        (st.num_swords, st.next_regen)
    };
    for i in 0..num_swords {
        // the C2 shred has a 1 frame icd and the C6 energy comes once:
        // both land with the first sword of the wave
        let mut cb = 0;
        if i == 0 {
            if s.chars[ci].cons >= 2 {
                cb |= CB_C2;
            }
            if next_regen {
                cb |= CB_C6;
            }
        }
        s.queue_attack_with_snap(ev, cb, 20);
    }
    let c6 = s.chars[ci].cons >= 6;
    let st = s.st_mut::<State>(ci);
    match st.num_swords {
        2 => {
            st.num_swords = 3;
            st.next_regen = false;
        }
        3 => {
            if c6 {
                st.num_swords = 5;
                st.next_regen = true;
            } else {
                st.num_swords = 2;
                st.next_regen = false;
            }
        }
        _ => {
            st.num_swords = 2;
            st.next_regen = false;
        }
    }
    s.add_status(ci, BURST_ICD, 60, true);
}
