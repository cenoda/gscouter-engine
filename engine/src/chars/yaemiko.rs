//! Yae Miko, from gcsim's internal/characters/yaemiko: normal attacks, the
//! skill's Sesshou Sakura (three charges, up to three totems), the burst's
//! thunderbolts, A1, A4 and the revelation (longer totems, superconduct's
//! enhanced strike, A1's extra strike), charged attacks and constellations
//! 1 to 6, with their Stellar-Conduct parts and Radiance: Stellar-Conduct
//! (stellar.go): inside a Polestar Field an enhanced strike adds a direct
//! Stellar-Conduct hit of 200% ATK, A1's strike becomes one of 50%, C1 is
//! also triggered by Stellar-Conduct (and Yae's direct Stellar-Conduct
//! DMG) and adds 50% Stellar-Conduct DMG, and C6 gives her direct
//! Stellar-Conduct DMG 200% CRIT DMG. Plunges from Xianyun's airborne state.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::yaemiko as g;
use crate::chars::plunge;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    revelation: bool,
    /// The totems, oldest first: their ids, the frames they appeared and
    /// the centers of their detection areas (the player's position then).
    totems: [(i32, i32, (f64, f64)); 3],
    count: usize,
    next_id: i32,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const REVELATION: ModKey = ModKey::new(g::REVELATION_KEY);
const REVELATION_ICD: ModKey = ModKey::new(g::REVELATION_ICD_KEY);
const TOTEM_STATUS: ModKey = ModKey::new(g::YAE_TOTEM_STATUS);
const A4: ModKey = ModKey::new("yaemiko-a4");
const C1_ELECTRO: ModKey = ModKey::new("yae-c1-electro");
const C1_SSC: ModKey = ModKey::new("yae-c1-ssc");
const C6: ModKey = ModKey::new(g::C6_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C4: ModKey = ModKey::new(g::C4_KEY);
const C4_ELECTRO: ModKey = ModKey::new("yaemiko-c4");
const C4_ENERGY_ICD: ModKey = ModKey::new(g::C4_ENERGY_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_C4: u16 = 1 << 1;
/// Radiance: Stellar-Conduct: the enhanced strike's extra hit.
const CB_REV_SSC: u16 = 1 << 2;

const TASK_MAKE: u8 = 0;
// arg: the totem's id
const TASK_TICK: u8 = 1;
const TASK_TIMEOUT: u8 = 2;
const TASK_C1_ENERGY: u8 = 3;

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

/// The airborne plunges (Xianyun's), from gcsim's plunge.go.
const PLUNGE: plunge::Spec = plunge::Spec::catalyst(Element::Electro, g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

impl Character for Char {
    const KEY: &'static str = "yaemiko";
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
        let o = Owner::Char(ci as u8);
        let revelation = s.st::<State>(ci).revelation;
        if revelation {
            s.subscribe(EventKind::Reaction, o);
        }
        // C1 with the revelation: her direct Stellar-Conduct DMG on the
        // enemy renews the buffs (only Radiance makes it)
        if s.chars[ci].cons >= 1 && revelation && s.stellar.conduct {
            s.subscribe(EventKind::EnemyDamage, o);
        }
        // C2 with the revelation: EM to Yae and the active character while
        // there are totems
        if s.chars[ci].cons >= 2 && revelation {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(C2, ModKind::Stat, o, st::EM, false, false, -1));
            }
        }
        // C4 with the revelation: burst DMG +100%
        if s.chars[ci].cons >= 4 && revelation {
            s.chars[ci].mods.push(Mod::owned(C4, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        // C6 with the revelation: CRIT DMG +200% on her direct
        // Stellar-Conduct DMG
        if s.chars[ci].cons >= 6 && revelation {
            s.chars[ci].mods.push(Mod::owned(C6, ModKind::Attack, o, st::NONE, false, false, -1));
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            // gcsim's ChargeAttack reads no params
            Action::Charge => p.0.first().map(|(k, _)| format!("charge param {k}")),
            Action::Attack | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// Plunges only while airborne (Xianyun's jump).
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
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

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_YAE_CHARGED, Element::Electro, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        // the windup is skipped out of a normal attack
        let windup = if s.current_state() == AnimState::Normal { 14 } else { 0 };
        // five 2 by 2 boxes, each 1.65 further toward the target, one every
        // 9 frames; the group's ICD lets one deal damage every 0.5 s. A box
        // that misses the enemy still reaches the gadgets it covers.
        for i in 0..5 {
            ai.pattern = s.box_from_player(s.point_from_player(0.0, 1.65 * (i + 1) as f64), 0.0, 0.0, 2.0, 2.0);
            let d = g::CHARGE_HITMARK + i * 9 - windup;
            if s.hits_enemy(ai.pattern) == Some(true) {
                s.queue_attack(ai, 0, 0, d);
            } else {
                s.queue_gadget_attack(ai, d);
            }
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: 0, arg: windup as i16 }, t[0] - windup, t[Action::Dash as usize] - windup, AnimState::Charge)
    }

    fn frames(_: u8, windup: i16, next: Action) -> i32 {
        g::CHARGE_FRAMES[next as usize] - windup as i32
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
            // C1: 8 energy as each thunderbolt lands
            if s.chars[ci].cons >= 1 {
                s.add_task(Task::Char { c: ci as u8, id: TASK_C1_ENERGY, arg: 0 }, d);
            }
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
                        // Radiance: a direct Stellar-Conduct hit of 50%
                        if radiance(s, ci) {
                            ai.tag = AttackTag::DirectStellarConduct;
                            ai.abil = "Yae A1 (Stellar-Conduct)";
                            ai.ignore_def = 1.0;
                            ai.mult = 0.5;
                        }
                        ai.pattern = s.circle_on_target(0.5);
                        s.queue_attack(ai, 0, 64, 64);
                    }
                }
                // the detection area: a circle on the player's position now
                let pos = s.player_pos;
                let st = s.st_mut::<State>(ci);
                st.totems[st.count] = (id, f, pos);
                st.count += 1;
            }
            TASK_TICK => {
                if !s.st::<State>(ci).has(arg) {
                    return;
                }
                // RandomEnemyWithinArea on the totem's area, a circle of
                // radius 12.5 (20 with C2) on where the player stood when it
                // appeared: the one enemy. An enemy out of it would make the
                // totem strike a gadget or nothing, which is not ported.
                let r = if s.chars[ci].cons >= 2 { 20.0 } else { 12.5 };
                let st = s.st::<State>(ci);
                let (x, y) = st.totems[..st.count].iter().find(|t| t.0 == arg).unwrap().2;
                if !s.enemy_within(Pattern::Circle { x, y, r }) {
                    s.unsupported("yaemiko Sesshou Sakura with the enemy out of its range");
                    return;
                }
                let _ = s.rng.intn(1);
                // the Sakura level: the totems, one more with C2
                let level = s.st::<State>(ci).count + usize::from(s.chars[ci].cons >= 2);
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL[level - 1][lvl]);
                ai.abil = "Sesshou Sakura Tick";
                ai.pattern = s.circle_on_target(0.5);
                // C6: the strikes ignore 60% of DEF
                if s.chars[ci].cons >= 6 {
                    ai.ignore_def = 0.6;
                }
                // the revelation: after a superconduct or Stellar-Conduct,
                // the next strike deals 80% of her ATK more, once every
                // 2.5 s; in Radiance it also adds a direct Stellar-Conduct
                // hit
                let mut rev = 0;
                if s.st::<State>(ci).revelation && s.mod_active(ci, REVELATION) && !s.mod_active(ci, REVELATION_ICD) {
                    s.add_status(ci, REVELATION_ICD, 150, true);
                    s.delete_mod(ci, REVELATION);
                    ai.flat_dmg = 0.8 * total_atk(&s.stats(ci));
                    if radiance(s, ci) {
                        rev = CB_REV_SSC;
                    }
                }
                let c4 = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
                s.queue_attack(ai, CB_PARTICLE | rev | c4, 1, 1);
                s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg }, 176);
            }
            TASK_TIMEOUT => {
                if s.st::<State>(ci).has(arg) {
                    pop_oldest(s, ci);
                }
            }
            TASK_C1_ENERGY => s.add_energy(ci, 8.0),
            _ => unreachable!(),
        }
    }

    /// C2: 60 / 90 / 120 / 200 EM by the Sakura level, to Yae and the
    /// active character.
    fn mod_amount(s: &Sim, ci: usize, on: usize, _: &Mod, out: &mut Stats) -> bool {
        if on != ci && s.player.active as usize != on {
            return false;
        }
        let count = s.st::<State>(ci).count;
        if count == 0 {
            return false;
        }
        out[st::EM] += g::C2_BUFF_VAL[count + 1];
        true
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C6 {
            if ev.info.tag == AttackTag::DirectStellarConduct {
                ev.stats[st::CD] += 2.0;
            }
        } else if m.key == C4 {
            if ev.info.tag == AttackTag::ElementalBurst {
                ev.stats[st::DMGP] += 1.0;
            }
        } else if ev.info.tag == AttackTag::ElementalArt {
            ev.stats[st::DMGP] += s.stat(ci, st::EM) * 0.0015;
        }
    }

    /// C1 with the revelation: Stellar-Conduct DMG +50%.
    fn react_bonus(_: &Sim, _: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if info.tag == AttackTag::DirectStellarConduct { 0.5 } else { 0.0 }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 150, true);
            s.queue_particle(1.0, Element::Electro, s.particle_delay[ci]);
        }
        // Radiance: the enhanced strike's extra direct Stellar-Conduct hit,
        // 200% of her ATK, 12 frames later
        if cbs & CB_REV_SSC != 0 {
            let mut ai = AttackInfo::new(ci, AttackTag::DirectStellarConduct, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, 2.0);
            ai.abil = "Sesshou Sakura (Stellar-Conduct)";
            ai.ignore_def = 1.0;
            ai.pattern = Pattern::Single;
            s.queue_attack(ai, 0, 12, 12);
        }
        // C4: a strike hitting the enemy gives the party 20% electro DMG for
        // 5 s and, with the revelation, Yae 8 energy once every 5 s
        if cbs & CB_C4 != 0 {
            for i in 0..s.chars.len() {
                s.add_stat_mod(i, C4_ELECTRO, st::ELECTROP, 0.2, 5 * 60, true);
            }
            if s.st::<State>(ci).revelation && !s.mod_active(ci, C4_ENERGY_ICD) {
                s.add_status(ci, C4_ENERGY_ICD, 5 * 60, true);
                s.add_energy(ci, 8.0);
            }
        }
    }

    /// The revelation: a superconduct or Stellar-Conduct on the enemy by
    /// anyone readies the enhanced strike. C1 with the revelation: such a
    /// reaction by Yae, or her direct Stellar-Conduct DMG on the enemy,
    /// gives the party 50% electro DMG and 50% Stellar-Conduct DMG for
    /// 10 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Reaction(Reaction::Superconduct | Reaction::StellarConduct, ev) if ev.on_enemy() => {
                s.add_status(ci, REVELATION, 8 * 60, true);
                if s.chars[ci].cons >= 1 && ev.info.actor as usize == ci {
                    c1_buffs(s, ci);
                }
            }
            Event::EnemyDamage(ev) if ev.info.actor as usize == ci && ev.info.tag == AttackTag::DirectStellarConduct => {
                c1_buffs(s, ci);
            }
            _ => {}
        }
    }
}

/// Radiance: Stellar-Conduct (with the revelation): inside a Polestar
/// Field.
fn radiance(s: &Sim, ci: usize) -> bool {
    s.st::<State>(ci).revelation && s.mod_active(ci, crate::stellar::POLESTAR_FIELD)
}

/// C1's buffs to every member: 50% electro DMG and 50% Stellar-Conduct
/// DMG for 10 s.
fn c1_buffs(s: &mut Sim, ci: usize) {
    for i in 0..s.chars.len() {
        s.add_stat_mod(i, C1_ELECTRO, st::ELECTROP, 0.5, 10 * 60, true);
        s.add_mod(i, Mod::owned(C1_SSC, ModKind::ReactBonus, Owner::Char(ci as u8), st::NONE, false, true, -1), 10 * 60);
    }
}
