//! Gorou, from gcsim's internal/characters/gorou, all of it: normal
//! attacks, aimed shots, Inuzaka All-Round Defense and its General's War
//! Banner (DEF, and Geo DMG with three Geo characters, for the active
//! character in its field), Juuga: Forward Unto Victory with General's
//! Glory (the same buff wherever the player is, Crystal Collapse every
//! 1.5 s, which also picks up a crystallize shard), A1, A4 and C1 to C6.
//! gcsim's package has no plunge.
//!
//! C2 also extends Glory on gcsim's OnLunarCrystallize, which the engine
//! never triggers (it is an error, `Sim::check_lunar_crystallize`).

use crate::content::{Character, Event};
use crate::gcsimdata::chars::gorou as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

/// A shard Crystal Collapse picks up: gcsim's AddShieldKillShard runs on
/// it when the pickup arrives, whatever happened to it meanwhile.
#[derive(Clone, Copy)]
struct Pull {
    src: i32,
    earliest_pickup: i32,
    shield: Shield,
}

pub struct State {
    geo_count: i32,
    /// The skill's DEF bonus at Init's talent level.
    skill_def: f64,
    /// The War Banner's area (its center, radius 8).
    e_field: (f64, f64),
    e_field_src: i32,
    q_field_src: i32,
    c2_extension: i32,
    c6_cd: f64,
    /// C1's next frame.
    c1_icd: i32,
    /// C4: the total DEF of the burst's snapshot of the stats.
    heal_def: f64,
    pulls: [Option<Pull>; 8],
    next_pull: usize,
}

const WAR_BANNER: ModKey = ModKey::new(g::GENERAL_WAR_BANNER_KEY);
const GLORY: ModKey = ModKey::new(g::GENERAL_GLORY_KEY);
const DEF_BUFF: ModKey = ModKey::new(g::DEFENSE_BUFF_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const C6: ModKey = ModKey::new(g::C6KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;

const TASK_SKILL: u8 = 0;
const TASK_BURST: u8 = 1;
// arg: the field's source frame
const TASK_FIELD: u8 = 2;
const TASK_COLLAPSE: u8 = 3;
const TASK_HEAL: u8 = 4;
// arg: the pull's slot
const TASK_PICKUP: u8 = 5;

impl Character for Char {
    const KEY: &'static str = "gorou";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { geo_count: 0, skill_def: 0.0, e_field: (0.0, 0.0), e_field_src: 0, q_field_src: 0, c2_extension: 0, c6_cd: 0.0, c1_icd: -1, heal_def: 0.0, pulls: [None; 8], next_pull: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let geo = s.chars.iter().filter(|c| c.element == Element::Geo).count() as i32;
        let lvl = s.talent(ci, 1);
        let cons = s.chars[ci].cons;
        let st = s.st_mut::<State>(ci);
        st.geo_count = geo;
        st.skill_def = g::SKILL_DEF_BONUS[lvl];
        st.c6_cd = match geo {
            1 => 0.1,
            2 => 0.2,
            _ => 0.4,
        };
        if cons >= 1 {
            s.subscribe(EventKind::EnemyDamage, o);
        }
        if cons >= 2 {
            s.subscribe(EventKind::Shielded, o);
        }
        if cons >= 6 {
            s.subscribe(EventKind::SpecialReactionAttack, o);
        }
        // Crystal Collapse picks up shards by where they lie
        s.shard_positions = true;
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            Action::Aim if !matches!(p.get("hold"), None | Some(0) | Some(1)) => Some("aim[hold]".into()),
            Action::Aim => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let h = g::ATTACK_HITMARKS[n];
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        s.queue_attack(ai, 0, h, h + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::FULLAIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        ai.deployable = true;
        // its hitlag applies on weak spot hits only (HitlagOnHeadshotOnly)
        if p.get("weakspot") == Some(1) {
            ai.weak_point = true;
            ai.hitlag_halt = 0.12 * 60.0;
        }
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        let hitmark = g::AIMED_HITMARKS[hold];
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        s.queue_attack(ai, 0, hitmark, hitmark + travel);
        let table = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Table(table), table[0], hitmark, AnimState::Aim)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL, arg: 0 }, g::SKILL_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 600, 32);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST, arg: 0 }, g::BURST_HITMARK);
        s.st_mut::<State>(ci).c2_extension = 0;
        s.set_cd(ci, Action::Burst, 20 * 60);
        s.consume_energy(ci, 7);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL => {
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL[lvl]);
                ai.abil = "Inuzaka All-Round Defense";
                ai.blunt = true;
                ai.poise = 80.0;
                ai.flat_dmg = a4(s, ci, 1.56);
                let Pattern::Circle { x, y, .. } = s.circle_on_player(0.0, 2.0, 8.0) else { unreachable!() };
                s.st_mut::<State>(ci).e_field = (x, y);
                ai.pattern = Pattern::Circle { x, y, r: 5.0 };
                s.queue_attack(ai, CB_PARTICLE, 0, 0);
                if s.core_status_key(GLORY) == 0 {
                    let f = s.f;
                    s.st_mut::<State>(ci).e_field_src = f;
                    s.add_task(Task::Char { c: ci as u8, id: TASK_FIELD, arg: f }, 17);
                    s.add_core_status(WAR_BANNER, g::GENERAL_WAR_BANNER_DURATION);
                }
                if s.chars[ci].cons == 6 {
                    c6(s, ci);
                }
            }
            TASK_BURST => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::BURST[lvl]);
                ai.abil = "Juuga: Forward Unto Victory";
                ai.blunt = true;
                ai.poise = 40.0;
                ai.flat_dmg = a4(s, ci, 0.156);
                ai.use_def = true;
                ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
                s.queue_attack(ai, 0, 0, 0);
                let f = s.f;
                s.st_mut::<State>(ci).e_field_src = f;
                s.add_task(Task::Char { c: ci as u8, id: TASK_FIELD, arg: f }, 17);
                s.delete_core_status(WAR_BANNER);
                s.add_core_status(GLORY, g::GENERAL_GLORY_DURATION);
                s.st_mut::<State>(ci).q_field_src = f;
                s.add_task(Task::Char { c: ci as u8, id: TASK_COLLAPSE, arg: f }, 90);
                if s.chars[ci].ascension >= 1 {
                    for i in 0..s.chars.len() {
                        s.add_mod(i, Mod::stat(A1, st::DEFP, 0.25, false, true, -1), 720);
                    }
                }
                if s.chars[ci].cons >= 4 && s.st::<State>(ci).geo_count > 1 {
                    let def = total_def(&s.stats_pruning(ci));
                    s.st_mut::<State>(ci).heal_def = def;
                    s.add_task(Task::Char { c: ci as u8, id: TASK_HEAL, arg: f }, 90);
                }
                if s.chars[ci].cons >= 6 {
                    c6(s, ci);
                }
            }
            TASK_FIELD => {
                let st = s.st::<State>(ci);
                if st.e_field_src != arg {
                    return;
                }
                let (x, y) = st.e_field;
                let e = s.core_status_key(WAR_BANNER) > 0;
                let q = s.core_status_key(GLORY) > 0;
                if !e && !q {
                    return;
                }
                if e && !q && !s.player_within(Pattern::Circle { x, y, r: 8.0 }) {
                    return;
                }
                let active = s.player.active as usize;
                s.add_mod(active, Mod::owned(DEF_BUFF, ModKind::Stat, Owner::Char(ci as u8), st::NONE, false, true, -1), 120);
                s.add_task(Task::Char { c: ci as u8, id: TASK_FIELD, arg }, 18);
            }
            TASK_COLLAPSE => {
                if s.st::<State>(ci).q_field_src != arg || s.core_status_key(GLORY) == 0 {
                    return;
                }
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::BURST_TICK[lvl]);
                ai.abil = "Crystal Collapse";
                ai.flat_dmg = a4(s, ci, 0.156);
                ai.use_def = true;
                let area = s.circle_on_player(0.0, 0.0, 8.0);
                if s.enemy_within(area) {
                    ai.pattern = s.circle_on_target(3.5);
                    s.queue_attack(ai, 0, 0, 1);
                }
                let (px, py) = s.player_pos;
                for k in 0..s.shards.len() {
                    let sh = s.shards[k];
                    if area.contains_point(sh.pos.0, sh.pos.1) != Some(true) {
                        continue;
                    }
                    let d = ((sh.pos.0 - px).powi(2) + (sh.pos.1 - py).powi(2)).sqrt();
                    let travel = (d / 0.4).ceil() as i32;
                    if s.f + travel < sh.earliest_pickup {
                        continue;
                    }
                    let st = s.st_mut::<State>(ci);
                    let slot = st.next_pull;
                    st.next_pull = (slot + 1) % st.pulls.len();
                    st.pulls[slot] = Some(Pull { src: sh.src, earliest_pickup: sh.earliest_pickup, shield: sh.shield });
                    s.add_task(Task::Char { c: ci as u8, id: TASK_PICKUP, arg: slot as i32 }, travel);
                    break;
                }
                s.add_task(Task::Char { c: ci as u8, id: TASK_COLLAPSE, arg }, 90);
            }
            TASK_HEAL => {
                if s.st::<State>(ci).q_field_src != arg || s.core_status_key(GLORY) == 0 {
                    return;
                }
                let active = s.player.active as usize;
                let src = s.st::<State>(ci).heal_def * 0.5;
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, Some(active), false, src, bonus);
                s.add_task(Task::Char { c: ci as u8, id: TASK_HEAL, arg }, 90);
            }
            TASK_PICKUP => {
                let Some(p) = s.st_mut::<State>(ci).pulls[arg as usize].take() else { return };
                // gcsim's AddShieldKillShard
                if s.f < p.earliest_pickup {
                    return;
                }
                if let Some(k) = s.shards.iter().position(|sh| sh.src == p.src) {
                    s.shards.remove(k);
                }
                let mut shield = p.shield;
                shield.expiry = s.f + (15.1 * 60.0) as i32;
                s.add_shield(shield);
            }
            _ => unreachable!(),
        }
    }

    /// The War Banner's buff: DEF, and Geo DMG with three Geo characters.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        let st = s.st::<State>(ci);
        out[st::DEF] += st.skill_def;
        if st.geo_count > 2 {
            out[st::GEOP] += 0.15;
        }
        true
    }

    /// C6's CRIT DMG on Geo attacks.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.element == Element::Geo {
            ev.stats[st::CD] += s.st::<State>(ci).c6_cd;
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 12, true);
            s.queue_particle(2.0, Element::Geo, s.particle_delay[ci]);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // C1: a party member's Geo damage in the field cuts the skill's CD
            Event::EnemyDamage(ev) => {
                let e = s.core_status_key(WAR_BANNER) > 0;
                let q = s.core_status_key(GLORY) > 0;
                if !e && !q || s.st::<State>(ci).c1_icd > s.f {
                    return;
                }
                let area = if e {
                    let (x, y) = s.st::<State>(ci).e_field;
                    Pattern::Circle { x, y, r: 8.0 }
                } else {
                    s.circle_on_player(0.0, 0.0, 8.0)
                };
                if !s.enemy_within(area) || ev.info.actor as usize == ci || ev.info.element != Element::Geo || ev.damage == 0.0 {
                    return;
                }
                s.st_mut::<State>(ci).c1_icd = s.f + 600;
                s.reduce_cd(ci, Action::Skill, 120);
            }
            // C2: crystallize shields extend Glory
            Event::Shielded(sh) => {
                if s.core_status_key(GLORY) <= 0 || sh.typ != ShieldType::Crystallize {
                    return;
                }
                c2_extend(s, ci);
            }
            Event::SpecialReactionAttack(ev) => {
                if ev.info.element == Element::Geo && s.mod_active(ev.info.actor as usize, C6) {
                    s.special_atk.stats[st::CD] += s.st::<State>(ci).c6_cd;
                }
            }
            _ => {}
        }
    }
}

/// A4's flat damage: a share of Gorou's DEF (TotalDef, no snapshot).
fn a4(s: &Sim, ci: usize, k: f64) -> f64 {
    if s.chars[ci].ascension < 4 {
        return 0.0;
    }
    total_def(&s.select_stats(ci, [st::BASEDEF, st::DEFP, st::DEF], false)) * k
}

fn c2_extend(s: &mut Sim, ci: usize) {
    let st = s.st_mut::<State>(ci);
    if st.c2_extension >= 3 {
        return;
    }
    st.c2_extension += 1;
    // gcsim's Status.Extend: only while it lasts
    let f = s.f;
    if let Some(e) = s.statuses.iter_mut().find(|e| e.0 == GLORY) {
        if e.1 > f {
            e.1 += 60;
        }
    }
}

fn c6(s: &mut Sim, ci: usize) {
    let o = Owner::Char(ci as u8);
    for i in 0..s.chars.len() {
        s.add_mod(i, Mod::owned(C6, ModKind::Attack, o, st::NONE, false, true, -1), 720);
    }
}
