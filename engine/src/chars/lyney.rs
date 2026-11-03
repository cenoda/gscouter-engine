//! Lyney, from gcsim's internal/characters/lyney: normal attacks, aimed
//! shots (physical, charged, and the Prop Arrow with its HP drain, Prop
//! Surplus, Grin-Malkin Hat and Spiritbreath Thorn), the hats (on their own
//! snapshot, a Pyrotechnic Strike when they expire or are replaced, an
//! explosion on the skill), Bewildering Lights (its stacks' DMG and heal),
//! Wondrous Trick: Miracle Parade (its ticks on unmarked enemies, its
//! Explosive Firework at the end or on the skill, and its hat), A1, A4 and
//! every constellation.
//!
//! As gcsim writes him: the hats take no hits that matter (nothing
//! subscribes to their OnGadgetHit), and the c2_stacks param only counts
//! above 3, as 3 (in 0 to 3 it is read into a variable gcsim drops).

use crate::content::{Character, Event};
use crate::gcsimdata::chars::lyney as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Clone, Copy)]
struct Hat {
    id: u8,
    x: f64,
    y: f64,
    /// The Pyrotechnic Strike on the hat's snapshot.
    ev: AttackEvent,
    /// A1's energy from its strike (an HP-draining Prop Arrow's hat).
    a1: bool,
}

pub struct State {
    hats: Vec<Hat>,
    max_hats: usize,
    next_id: u8,
    prop_surplus: i32,
    pyrotechnic_travel: i32,
    c2_src: i32,
    c2_stacks: i32,
    /// The pyro members of the team, for A4.
    pyro_count: i32,
    /// The skill's hit and heal (target, amount, bonus), taken as it is
    /// cast.
    skill_ai: AttackInfo,
    skill_heal: (usize, f64, f64),
    /// The pending Prop Arrow's weakspot param.
    prop_weak: bool,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const BURST_MARK: ModKey = ModKey::new(g::BURST_MARK_KEY);
const HAT_STATUS: ModKey = ModKey::new(g::GRIN_MALKIN_HAT_KEY);
const ALIGNED_ICD: ModKey = ModKey::new(g::SKILL_ALIGNED_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A4: ModKey = ModKey::new("lyney-a4");
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C2: ModKey = ModKey::new("lyney-c2");
const C4: ModKey = ModKey::new("lyney-c4");

const CB_PARTICLE: u16 = 1 << 0;
const CB_A1: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;

/// The Prop Arrow's release, with travel * 256 + c6 travel.
const TASK_PROP: u8 = 0;
/// After the Prop Arrow's travel: C1's stack, with hp drained.
const TASK_C1_STACK: u8 = 1;
/// The hat, with hp drained.
const TASK_HAT: u8 = 2;
/// The Spiritbreath Thorn, at the target's position.
const TASK_ALIGNED: u8 = 3;
/// The skill's hit, the hats' explosions and the heal, with the heal's
/// target * 2^24 and its amount kept in state.
const TASK_SKILL: u8 = 4;
/// The burst's start.
const TASK_BURST_START: u8 = 5;
const TASK_BURST_TICK: u8 = 6;
const TASK_FIREWORK: u8 = 7;
/// C2's stack check, with its src.
const TASK_C2: u8 = 8;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];

impl Character for Char {
    const KEY: &'static str = "lyney";
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        let param = |k: &str| cs.params.iter().find(|p| p.0 == k).map(|p| p.1);
        State {
            hats: Vec::with_capacity(2),
            max_hats: 1,
            next_id: 0,
            prop_surplus: 0,
            pyrotechnic_travel: param("pyrotechnic_travel").map_or(36, |v| v as i32),
            c2_src: 0,
            c2_stacks: if param("c2_stacks").unwrap_or(0) > 3 { 3 } else { 0 },
            pyro_count: 0,
            skill_ai: AttackInfo::new(0, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, 0.0),
            skill_heal: (0, 0.0, 0.0),
            prop_weak: false,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // c1
        if s.chars[ci].cons >= 1 {
            s.st_mut::<State>(ci).max_hats = 2;
        }
        // a4
        if s.chars[ci].ascension >= 4 {
            let n = s.chars.iter().filter(|c| c.element == Element::Pyro).count() as i32;
            s.st_mut::<State>(ci).pyro_count = n;
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        // c2Setup
        if s.chars[ci].cons >= 2 {
            s.subscribe(EventKind::Swap, o);
            s.chars[ci].mods.push(Mod::owned(C2, ModKind::Stat, o, st::CD, false, false, -1));
        }
    }

    /// gcsim validates travel on the normal attack and c6_travel, hold,
    /// travel and weakspot on the aimed shot, and reads no params of the
    /// others.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let only = |known: &[&str]| p.0.iter().find(|(k, _)| !known.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"));
        match a {
            Action::Attack => only(&["travel"]),
            Action::Aim => only(&["c6_travel", "hold", "travel", "weakspot"]),
            Action::Skill | Action::Burst => None,
            _ => Some(name.into()),
        }
    }

    /// No aimed shot after the fourth normal attack; gcsim's error for an
    /// aim with another hold.
    fn invalid(s: &Sim, _: usize, a: Action, p: &Params) -> Option<String> {
        if a == Action::Aim {
            if let Some(h) = p.get("hold") {
                if !(0..=2).contains(&h) {
                    return Some(format!("invalid hold param supplied, got {h}"));
                }
            }
            if s.player.last.action == Action::Attack && s.chars[s.player.active as usize].normal_counter == 0 {
                return Some("cannot use aim after n4".into());
            }
        }
        crate::content::template_invalid(s, a)
    }

    /// The skill during the burst, whatever its cooldown.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && s.mod_active(ci, BURST)).then_some(None)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<crate::gcsl::Num, String>> {
        (fields[0] == "propSurplusStacks").then(|| Ok(crate::gcsl::Num::Int(s.st::<State>(ci).prop_surplus as i64)))
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").unwrap_or(10) as i32;
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let rel = g::ATTACK_RELEASES[n];
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
            s.queue_attack(ai, 0, rel[i], rel[i] + travel);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], rel[rel.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(2) as usize;
        let travel = p.get("travel").unwrap_or(10) as i32;
        let weakspot = p.get("weakspot").unwrap_or(0) == 1;
        let c4 = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
        if hold == 2 {
            // PropAimed
            let c6_travel = p.get("c6_travel").unwrap_or(10) as i32;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_PROP, arg: travel << 8 | c6_travel.clamp(0, 255) }, g::AIMED_PROP_RELEASE);
            if weakspot {
                s.st_mut::<State>(ci).prop_weak = true;
            }
            let t = &g::AIMED_PROP_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], g::AIMED_PROP_RELEASE, AnimState::Aim);
        }
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::FULLAIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        ai.deployable = true;
        ai.hitlag_factor = 0.01;
        if weakspot {
            ai.weak_point = true;
            ai.hitlag_halt = 0.12 * 60.0;
        }
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let h = g::AIMED_HITMARKS[hold];
        s.queue_attack(ai, c4, h, h + travel);
        let t = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Aim)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, BURST) {
            explosive_firework(s, ci);
            let t = &g::SKILL_BURST_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill);
        }
        let lvl = s.talent(ci, 1);
        let stacks = s.st::<State>(ci).prop_surplus;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL[lvl] + g::SKILL_BONUS[lvl] * stacks as f64);
        ai.abil = "Bewildering Lights";
        ai.pattern = s.circle_on_player(0.0, 5.5, 5.5);
        let heal = (s.player.active as usize, 0.2 * s.max_hp(ci) * stacks as f64, s.stat(ci, st::HEAL));
        let st = s.st_mut::<State>(ci);
        st.prop_surplus = 0;
        st.skill_heal = heal;
        st.skill_ai = ai;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL, arg: 0 }, g::SKILL_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD, g::SKILL_CD_START);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_START, arg: 0 }, g::BURST_START);
        s.set_cd(ci, Action::Burst, g::BURST_CD);
        s.consume_energy(ci, g::BURST_ENERGY_DELAY);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Skill as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_PROP => {
                let (travel, c6_travel) = (arg >> 8, arg & 255);
                let c4 = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
                // c6
                if s.chars[ci].cons >= 6 {
                    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_LYNEY_END_BOOM, icd::GROUP_LYNEY_EXTRA, Element::Pyro, 25.0, g::PROP_PYROTECHNIC[s.talent(ci, 0)] * 0.8);
                    ai.abil = "Pyrotechnic Strike: Reprised (C6)";
                    ai.pattern = s.circle_toward_target(0.0, 0.0, 1.0);
                    s.queue_attack(ai, c4, 0, c6_travel);
                }
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::PROP[s.talent(ci, 0)]);
                ai.abil = "Fully-Charged Aimed Shot (Prop Arrow)";
                ai.deployable = true;
                ai.hitlag_factor = 0.01;
                if std::mem::take(&mut s.st_mut::<State>(ci).prop_weak) {
                    ai.weak_point = true;
                    ai.hitlag_halt = 0.12 * 60.0;
                }
                ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
                s.queue_attack(ai, c4, 0, travel);
                let drained = prop_surplus(s, ci) as i32;
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C1_STACK, arg: 0 }, travel);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_HAT, arg: drained }, travel);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ALIGNED, arg: 0 }, travel);
            }
            TASK_C1_STACK => {
                if s.chars[ci].cons >= 1 && !s.mod_active(ci, C1_ICD) {
                    add_surplus(s, ci);
                }
            }
            TASK_HAT => {
                // c1HatIncrease
                let mut n = 1;
                if s.chars[ci].cons >= 1 && !s.mod_active(ci, C1_ICD) {
                    n = 2;
                    s.add_status(ci, C1_ICD, g::C1_ICD, true);
                }
                let (x, y) = s.enemy_pos;
                for _ in 0..n {
                    if s.st::<State>(ci).hats.len() == s.st::<State>(ci).max_hats {
                        kill_hat(s, ci);
                    }
                    new_hat(s, ci, x, y, arg != 0, g::GRIN_MALKIN_HAT_AIMED_DURATION);
                }
            }
            TASK_ALIGNED => {
                if s.mod_active(ci, ALIGNED_ICD) {
                    return;
                }
                s.add_status(ci, ALIGNED_ICD, g::SKILL_ALIGNED_ICD, true);
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 0.0, g::PROP_ALIGNED[s.talent(ci, 0)]);
                ai.abil = "Spiritbreath Thorn (lyney)";
                ai.pattern = s.circle_on_target(2.0);
                let c4 = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
                s.queue_attack(ai, c4, 42, 42);
            }
            TASK_SKILL => {
                let ai = s.st::<State>(ci).skill_ai;
                s.queue_attack(ai, CB_PARTICLE, 0, 0);
                let n = s.st::<State>(ci).hats.len();
                for _ in 0..n {
                    explode_hat(s, ci);
                }
                let (target, amt, bonus) = s.st::<State>(ci).skill_heal;
                s.heal(ci, target, amt, bonus);
            }
            TASK_BURST_START => {
                s.add_status(ci, BURST, g::BURST_DURATION, true);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_TICK, arg: 0 }, g::BURST_FIRST_INTERVAL);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_FIREWORK, arg: 0 }, g::BURST_DURATION);
            }
            TASK_BURST_TICK => {
                if !s.mod_active(ci, BURST) {
                    return;
                }
                if s.enemy_alive && s.enemy_within(s.circle_on_player(0.0, 0.0, 4.0)) && !s.enemy_status_active(BURST_MARK) {
                    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BURST[s.talent(ci, 2)]);
                    ai.abil = "Wondrous Trick: Miracle Parade";
                    ai.pattern = Pattern::Single;
                    s.queue_attack(ai, 0, 0, 0);
                    let d = s.mod_duration(ci, g::BURST_KEY);
                    s.enemy_add_status(BURST_MARK, d, true);
                }
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_TICK, arg: 0 }, g::BURST_INTERVAL as i32);
            }
            TASK_FIREWORK => explosive_firework(s, ci),
            TASK_C2 => {
                let st = s.st::<State>(ci);
                if arg != st.c2_src || s.player.active as usize != ci || st.c2_stacks == 3 {
                    return;
                }
                s.st_mut::<State>(ci).c2_stacks += 1;
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C2, arg }, g::C2_INTERVAL);
            }
            _ => unreachable!(),
        }
    }

    /// A hat's end of duration: its Pyrotechnic Strike.
    fn gadget_tick(s: &mut Sim, ci: usize, id: u8, _: i32) {
        if s.gadget_index(ci, id).is_some() {
            return;
        }
        let Some(i) = s.st::<State>(ci).hats.iter().position(|h| h.id == id) else { return };
        pyrotechnic(s, ci, i);
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD as i32, true);
            s.queue_particle(g::PARTICLE_COUNT as f64, Element::Pyro, s.particle_delay[ci]);
        }
        if cbs & CB_A1 != 0 {
            s.add_energy(ci, 3.0);
        }
        if cbs & CB_C4 != 0 {
            s.enemy_add_resist(C4, Element::Pyro, -0.2, 6 * 60);
        }
    }

    /// C2: CRIT DMG +20% a stack.
    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key != C2 {
            return false;
        }
        out[st::CD] += s.st::<State>(ci).c2_stacks as f64 * 0.2;
        true
    }

    /// A4: DMG +60% (+20% for each other pyro member, up to 100%) on pyro-
    /// affected enemies.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A4 && s.enemy_aura_contains(Element::Pyro) {
            ev.stats[st::DMGP] += (0.6 + (s.st::<State>(ci).pyro_count - 1) as f64 * 0.2).min(1.0);
        }
    }

    /// c2's swap: stacks lost when he leaves, gained every 2 s on the field.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Swap { prev, next } = *e else { return };
        if prev == ci {
            let st = s.st_mut::<State>(ci);
            st.c2_src = -1;
            st.c2_stacks = 0;
            return;
        }
        if next == ci {
            let f = s.f;
            s.st_mut::<State>(ci).c2_src = f;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C2, arg: f }, g::C2_INTERVAL);
        }
    }
}

fn add_surplus(s: &mut Sim, ci: usize) {
    let st = s.st_mut::<State>(ci);
    st.prop_surplus = (st.prop_surplus + 1).min(5);
}

/// propSurplus: 20% of max HP drained above 60% HP (down to 60%), a stack.
fn prop_surplus(s: &mut Sim, ci: usize) -> bool {
    let ratio = s.chars[ci].hp_ratio;
    if ratio < g::PROP_SURPLUS_HP_DRAIN_THRESHOLD || (ratio - g::PROP_SURPLUS_HP_DRAIN_THRESHOLD).abs() < g::PROP_SURPLUS_ROUNDING_TOLERANCE {
        return false;
    }
    let cur = s.current_hp(ci);
    let max = s.max_hp(ci);
    let mut drain = g::PROP_SURPLUS_HP_DRAIN_RATIO * max;
    if (cur - drain) / max <= g::PROP_SURPLUS_HP_DRAIN_THRESHOLD {
        drain = cur - g::PROP_SURPLUS_HP_DRAIN_THRESHOLD * max;
    }
    s.drain_info(ci, drain, false, "Prop Surplus");
    add_surplus(s, ci);
    true
}

/// newGrinMalkinHat: a gadget at the point, its Pyrotechnic Strike on the
/// snapshot taken now.
fn new_hat(s: &mut Sim, ci: usize, x: f64, y: f64, drained: bool, duration: i32) {
    let id = s.st::<State>(ci).next_id;
    s.st_mut::<State>(ci).next_id = id.wrapping_add(1);
    s.add_status(ci, HAT_STATUS, duration, false);
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_LYNEY_END_BOOM, icd::GROUP_LYNEY_EXTRA, Element::Pyro, 25.0, g::PROP_PYROTECHNIC[s.talent(ci, 0)]);
    ai.abil = "Pyrotechnic Strike";
    let mut ev = s.snapshot_event(ai, 0);
    let a1 = s.chars[ci].ascension >= 1 && drained;
    if a1 {
        ev.info.mult += 0.8;
    }
    s.st_mut::<State>(ci).hats.push(Hat { id, x, y, ev, a1 });
    s.add_gadget(ci, id, duration, x, y, 1.0);
}

fn hat_cbs(s: &Sim, ci: usize, h: &Hat) -> u16 {
    let mut cbs = if h.a1 { CB_A1 } else { 0 };
    if s.chars[ci].cons >= 4 {
        cbs |= CB_C4;
    }
    cbs
}

/// skillPyrotechnic: the hat's strike on the enemy, and the hat gone.
fn pyrotechnic(s: &mut Sim, ci: usize, i: usize) {
    let h = s.st_mut::<State>(ci).hats.remove(i);
    let mut ev = h.ev;
    ev.snap_f = s.f;
    ev.info.pattern = s.circle_toward_target(0.0, 0.0, 1.0);
    let cbs = hat_cbs(s, ci, &h);
    let travel = s.st::<State>(ci).pyrotechnic_travel;
    s.queue_attack_with_snap(ev, cbs, travel);
}

/// Kill on the oldest hat: its OnKill strike.
fn kill_hat(s: &mut Sim, ci: usize) {
    let id = s.st::<State>(ci).hats[0].id;
    s.kill_gadget(ci, id);
    pyrotechnic(s, ci, 0);
}

/// skillExplode on the oldest hat: the enhanced strike around it.
fn explode_hat(s: &mut Sim, ci: usize) {
    let h = s.st_mut::<State>(ci).hats.remove(0);
    let mut ev = h.ev;
    ev.info.icd_tag = icd::TAG_LYNEY_END_BOOM_ENHANCED;
    ev.info.blunt = true;
    ev.info.poise = 90.0;
    ev.snap_f = s.f;
    ev.info.pattern = Pattern::Circle { x: h.x, y: h.y, r: 3.5 };
    let cbs = hat_cbs(s, ci, &h);
    s.queue_attack_with_snap(ev, cbs, g::SKILL_EXPLODE);
    s.kill_gadget(ci, h.id);
}

/// explosiveFirework: the burst's end, its explosion, and a hat at the
/// player.
fn explosive_firework(s: &mut Sim, ci: usize) {
    if !s.mod_active(ci, BURST) {
        return;
    }
    s.delete_mod(ci, BURST);
    if s.enemy_status_active(BURST_MARK) {
        s.enemy_delete_status(BURST_MARK);
    }
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::EXPLOSIVE_FIREWORK[s.talent(ci, 2)]);
    ai.abil = "Wondrous Trick: Miracle Parade (Explosive Firework)";
    let (x, y) = s.player_pos;
    ai.pattern = Pattern::Circle { x, y, r: 6.0 };
    s.queue_attack(ai, 0, 6, 6);
    if s.st::<State>(ci).hats.len() == s.st::<State>(ci).max_hats {
        kill_hat(s, ci);
    }
    new_hat(s, ci, x, y, false, g::GRIN_MALKIN_HAT_BURST_DURATION);
    add_surplus(s, ci);
}
