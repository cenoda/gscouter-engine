//! Yae Miko, from gcsim's internal/characters/yaemiko, at C0: normal
//! attacks, the skill's Sesshou Sakura (three charges, up to three
//! totems), the burst's thunderbolts, A1, A4 and the revelation (longer
//! totems, superconduct's enhanced strike, A1's extra strike). No charged
//! attacks, plunges or constellations. Radiance needs a Polestar Field and
//! Stellar-Conduct, which nothing the engine has makes, so it never
//! applies; port it with the first content that makes them.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::yaemiko as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    revelation: bool,
    /// The totems, oldest first: their ids and the frames they appeared.
    totems: [(i32, i32); 3],
    count: usize,
    next_id: i32,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const REVELATION: ModKey = ModKey::new(g::REVELATION_KEY);
const REVELATION_ICD: ModKey = ModKey::new(g::REVELATION_ICD_KEY);
const TOTEM_STATUS: ModKey = ModKey::new(g::YAE_TOTEM_STATUS);
const A4: ModKey = ModKey::new("yaemiko-a4");

const CB_PARTICLE: u16 = 1 << 0;

const TASK_MAKE: u8 = 0;
// arg: the totem's id
const TASK_TICK: u8 = 1;
const TASK_TIMEOUT: u8 = 2;

impl State {
    fn has(&self, id: i32) -> bool {
        self.totems[..self.count].iter().any(|t| t.0 == id)
    }
}

/// A totem's life after it appears.
fn totem_dur(s: &Sim, ci: usize) -> i32 {
    g::KITSUNE_DUR + if s.st::<State>(ci).revelation { 10 * 60 } else { 0 } - g::SKILL_START
}

fn pop_oldest(s: &mut Sim, ci: usize) {
    let st = s.st_mut::<State>(ci);
    if st.count == 0 {
        return;
    }
    st.totems.copy_within(1..st.count, 0);
    st.count -= 1;
    if st.count > 0 {
        let src = st.totems[0].1;
        let dur = s.f - src + totem_dur(s, ci);
        s.add_core_status(TOTEM_STATUS, dur);
    } else {
        s.delete_core_status(TOTEM_STATUS);
    }
}

impl Character for Char {
    const KEY: &'static str = "yaemiko";
    // C1 to C6 are not ported
    const MAX_CONS: i32 = 0;
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        let revelation = cs.params.iter().find(|p| p.0 == "revelation").is_none_or(|p| p.1 > 0);
        State { revelation, ..State::default() }
    }

    fn init(s: &mut Sim, ci: usize) {
        let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
        cd.available = 3;
        cd.max_extra = 2;
        // A4: 0.15% Sesshou Sakura DMG for each point of EM
        if s.chars[ci].ascension >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1));
        }
        if s.st::<State>(ci).revelation {
            s.subscribe(EventKind::Reaction, Owner::Char(ci as u8));
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match (fields[0].as_str(), fields.get(1).map(|f| f.as_str())) {
            ("tags", Some(g::YAE_TOTEM_COUNT)) => Some(Ok(Num::Int(s.st::<State>(ci).count as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.box_on_target(0.0, 0.0, 2.0, 8.0);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, 0, h + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_task(Task::Char { c: ci as u8, id: TASK_MAKE, arg: 0 }, g::SKILL_START);
        s.set_cd_with_delay(ci, Action::Skill, 4 * 60, 16);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::BURST[0][lvl]);
        ai.abil = "Great Secret Art: Tenko Kenshin";
        ai.pattern = s.circle_on_target(7.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        // a thunderbolt for each totem, each giving back a skill charge (A1)
        ai.abil = "Tenko Thunderbolt";
        ai.mult = g::BURST[1][lvl];
        let count = s.st::<State>(ci).count;
        for i in 0..count as i32 {
            let d = g::BURST_THUNDERBOLT1_HITMARK + i * 24;
            s.queue_attack(ai, 0, d, d);
            if s.chars[ci].ascension >= 1 {
                s.reset_cd(ci, Action::Skill);
            }
        }
        if !s.st::<State>(ci).revelation {
            s.st_mut::<State>(ci).count = 0;
            s.delete_core_status(TOTEM_STATUS);
        }
        s.consume_energy(ci, 2);
        s.set_cd(ci, Action::Burst, 22 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_MAKE => {
                let f = s.f;
                let dur = totem_dur(s, ci);
                let st = s.st_mut::<State>(ci);
                let id = st.next_id;
                st.next_id += 1;
                let first = st.count == 0;
                s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg: id }, 120 - g::SKILL_START);
                s.add_task(Task::Char { c: ci as u8, id: TASK_TIMEOUT, arg: id }, dur);
                if first {
                    s.add_core_status(TOTEM_STATUS, dur);
                }
                if s.st::<State>(ci).count == 3 {
                    pop_oldest(s, ci);
                    // A1 with the revelation: a strike when a fourth totem
                    // replaces the oldest
                    if s.chars[ci].ascension >= 1 && s.st::<State>(ci).revelation {
                        let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, 0.4);
                        ai.abil = "Yae A1";
                        ai.pattern = s.circle_on_target(0.5);
                        s.queue_attack(ai, 0, 64, 64);
                    }
                }
                let st = s.st_mut::<State>(ci);
                st.totems[st.count] = (id, f);
                st.count += 1;
            }
            TASK_TICK => {
                if !s.st::<State>(ci).has(arg) {
                    return;
                }
                // RandomEnemyWithinArea: the one enemy, always in range
                let _ = s.rng.intn(1);
                let level = s.st::<State>(ci).count;
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL[level - 1][lvl]);
                ai.abil = "Sesshou Sakura Tick";
                ai.pattern = s.circle_on_target(0.5);
                // the revelation: after a superconduct, the next strike deals
                // 80% of her ATK more, once every 2.5 s
                if s.st::<State>(ci).revelation && s.mod_active(ci, REVELATION) && !s.mod_active(ci, REVELATION_ICD) {
                    s.add_status(ci, REVELATION_ICD, 150, true);
                    s.delete_mod(ci, REVELATION);
                    ai.flat_dmg = 0.8 * total_atk(&s.stats(ci));
                }
                s.queue_attack(ai, CB_PARTICLE, 1, 1);
                s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg }, 176);
            }
            TASK_TIMEOUT => {
                if s.st::<State>(ci).has(arg) {
                    pop_oldest(s, ci);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::ElementalArt {
            ev.stats[st::DMGP] += s.stat(ci, st::EM) * 0.0015;
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 150, true);
            s.queue_particle(1.0, Element::Electro, s.particle_delay[ci]);
        }
    }

    /// The revelation: a superconduct on the enemy by anyone readies the
    /// enhanced strike.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Reaction(Reaction::Superconduct, ev) = *e {
            if !ev.on_player {
                s.add_status(ci, REVELATION, 8 * 60, true);
            }
        }
    }
}
