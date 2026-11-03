//! Tighnari, from gcsim's internal/characters/tighnari: normal attacks,
//! aimed shots (hold 0 and 1, and the Wreath Arrow with its four
//! Clusterbloom Arrows: shortened in Vijnana Suffusion and from C6),
//! Vijnana-Phala Mine, Fashioner's Tanglevine Shaft, A1, A4 and every
//! constellation.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::tighnari as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The Wreath Arrows left in Vijnana Suffusion (gcsim's tag).
    wreath_arrows: i32,
    /// The skill's area (a circle of 6 on the enemy), which C2 reads.
    skill_area: Pattern,
}

const VIJNANA: ModKey = ModKey::new(g::VIJNANASUFFUSION_STATUS);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new("tighnari-a1");
const A4: ModKey = ModKey::new("tighnari-a4");
const C1: ModKey = ModKey::new("tighnari-c1");
const C2: ModKey = ModKey::new("tighnari-c2");
const C4: ModKey = ModKey::new("tighnari-c4");

const CB_PARTICLE: u16 = 1 << 0;

/// Vijnana Suffusion and its three arrows (the fight's queue).
const TASK_VIJNANA: u8 = 0;
/// A1: EM +50 for 4 s after a Wreath Arrow (the fight's queue).
const TASK_A1: u8 = 1;
/// The Clusterbloom Arrows: arg is their travel (the fight's queue).
const TASK_CLUSTERBLOOM: u8 = 2;
/// C2 starts checking the player in the skill's area (her queue).
const TASK_C2_START: u8 = 3;
/// C2: one check (the fight's queue).
const TASK_C2: u8 = 4;

const FR_WREATH: u8 = 0;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];

impl Character for Char {
    const KEY: &'static str = "tighnari";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { wreath_arrows: 0, skill_area: Pattern::Single }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        if s.chars[ci].ascension >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        if s.chars[ci].cons >= 1 {
            s.chars[ci].mods.push(Mod::owned(C1, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        if s.chars[ci].cons >= 4 {
            s.subscribe(EventKind::Burst, o);
            s.subscribe(EventKind::Reaction, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let known: &[&str] = match a {
            Action::Attack | Action::Skill | Action::Burst => &["travel"],
            Action::Aim => &["hold", "travel", "weakspot", "wreath"],
            _ => return Some(crate::gcsl::action_name(a).into()),
        };
        let name = crate::gcsl::action_name(a);
        p.0.iter().find(|(k, _)| !known.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
    }

    /// gcsim's error for an aim with another hold (outside Vijnana
    /// Suffusion, where every aim is a Wreath Arrow).
    fn invalid(s: &Sim, ci: usize, a: Action, p: &Params) -> Option<String> {
        if a == Action::Aim && !s.mod_active(ci, VIJNANA) {
            if let Some(h) = p.get("hold") {
                if !(0..=2).contains(&h) {
                    return Some(format!("invalid hold param supplied, got {h}"));
                }
            }
        }
        crate::content::template_invalid(s, a)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        (fields[0] == "tags" && fields.get(1).map(String::as_str) == Some(g::WREATHARROWS)).then(|| Ok(Num::Int(s.st::<State>(ci).wreath_arrows as i64)))
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let hits = g::ATTACK_HITMARKS[n];
        for (i, &h) in hits.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
            s.queue_attack(ai, 0, h, h + travel);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hits[hits.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    /// Aimed shots: physical (hold 0), charged (hold 1, the default), or
    /// the Wreath Arrow (hold 2, and every aim in Vijnana Suffusion).
    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let mut hold = p.get("hold").unwrap_or(1) as usize;
        if s.mod_active(ci, VIJNANA) {
            hold = 2;
        }
        if hold == 2 {
            return wreath(s, ci, p);
        }
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let mut ai = arrow_info(ci, p.get("weakspot") == Some(1));
        ai.mult = g::FULLAIM[lvl];
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let h = g::AIMED_HITMARKS[hold];
        s.queue_attack(ai, 0, h, h + travel);
        let t = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Aim)
    }

    /// Vijnana-Phala Mine: the hit on the enemy `travel` frames after its
    /// release; Vijnana Suffusion with three Wreath Arrows 13 frames in;
    /// C2's field.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(5, |t| t as i32);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::SKILL[lvl]);
        ai.abil = "Vijnana-Phala Mine";
        let area = s.circle_toward_target(0.0, 0.0, 6.0);
        s.st_mut::<State>(ci).skill_area = area;
        ai.pattern = area;
        s.queue_attack(ai, CB_PARTICLE, g::SKILL_RELEASE, g::SKILL_RELEASE + travel);
        s.set_cd_with_delay(ci, Action::Skill, 12 * 60, 13);
        s.add_task(Task::Char { c: ci as u8, id: TASK_VIJNANA, arg: 0 }, 13);
        if s.chars[ci].cons >= 2 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C2_START, arg: 0 }, g::SKILL_RELEASE + travel);
        }
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Aim as usize], AnimState::Skill)
    }

    /// Fashioner's Tanglevine Shaft: six shafts snapshot at the release
    /// and six secondary ones each snapshot when its first lands.
    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").unwrap_or(0) as i32;
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::BURST[lvl]);
        ai.abil = "Tanglevine Shaft";
        ai.pattern = s.circle_toward_target(0.0, 0.0, 1.0);
        for &h in g::BURST_HITMARKS {
            s.queue_attack(ai, 0, g::BURST_RELEASE, h + travel);
        }
        ai.abil = "Secondary Tanglevine Shaft";
        ai.mult = g::BURST_SECOND[lvl];
        for (i, &h) in g::BURST_HITMARKS.iter().enumerate() {
            s.queue_attack(ai, 0, h + travel, g::BURST_SECOND_HITMARKS[i] + travel);
        }
        s.consume_energy(ci, 7);
        s.set_cd(ci, Action::Burst, 12 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Burst)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        debug_assert_eq!(id, FR_WREATH);
        g::AIMED_WREATH_FRAMES[next as usize] - arg as i32
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        let c = ci as u8;
        match id {
            TASK_VIJNANA => {
                s.add_status(ci, VIJNANA, 12 * 60, false);
                s.st_mut::<State>(ci).wreath_arrows = 3;
            }
            TASK_A1 => s.add_stat_mod(ci, A1, st::EM, 50.0, 4 * 60, false),
            TASK_CLUSTERBLOOM => {
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_TIGHNARI, Element::Dendro, 25.0, g::CLUSTERBLOOM[lvl]);
                ai.abil = "Clusterbloom Arrow";
                let mut ev = s.snapshot_event(ai, 0);
                ev.info.pattern = s.circle_toward_target(0.0, 0.0, 1.0);
                for _ in 0..4 {
                    s.queue_attack_with_snap(ev, 0, arg);
                }
                // C6: one more arrow on the same snapshot
                if s.chars[ci].cons >= 6 {
                    ev.info = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, 1.5);
                    ev.info.abil = "Karma Adjudged From the Leaden Fruit";
                    ev.info.pattern = s.circle_toward_target(0.0, 0.0, 1.0);
                    s.queue_attack_with_snap(ev, 0, arg);
                }
            }
            TASK_C2_START => {
                let mut i = 0;
                while i < 8 * 60 {
                    s.add_task(Task::Char { c, id: TASK_C2, arg: 0 }, i);
                    i += 30;
                }
            }
            TASK_C2 => {
                let area = s.st::<State>(ci).skill_area;
                if s.player_within(area) {
                    s.add_stat_mod(ci, C2, st::DENDROP, 0.2, 6 * 60, false);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, true);
            let count = if s.rng.float64() < 0.5 { 4.0 } else { 3.0 };
            s.queue_particle(count, Element::Dendro, s.particle_delay[ci]);
        }
    }

    /// A4: charged attacks and the burst +0.06% DMG per EM (up to 60%);
    /// C1: charged attacks CRIT Rate +15%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A4 && matches!(ev.info.tag, AttackTag::Extra | AttackTag::ElementalBurst) {
            ev.stats[st::DMGP] += (s.stat(ci, st::EM) * 0.0006).min(0.6);
        }
        if m.key == C1 && ev.info.tag == AttackTag::Extra {
            ev.stats[st::CR] += 0.15;
        }
    }

    /// C4: her burst on field gives the party EM +60 for 8 s; a dendro
    /// reaction its hits trigger on the enemy makes it +120.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let em = match *e {
            Event::Burst => {
                if s.player.active as usize != ci {
                    return;
                }
                60.0
            }
            Event::Reaction(Reaction::Burning | Reaction::Bloom | Reaction::LunarBloom | Reaction::Quicken | Reaction::Spread, ev) => {
                if !ev.on_enemy() || ev.info.actor as usize != ci || ev.info.tag != AttackTag::ElementalBurst {
                    return;
                }
                120.0
            }
            _ => return,
        };
        for k in 0..s.chars.len() {
            s.add_stat_mod(k, C4, st::EM, em, 8 * 60, true);
        }
    }
}

/// The aimed shots' info: deployable, their hitlag on weak spot hits only.
fn arrow_info(ci: usize, weakspot: bool) -> AttackInfo {
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, 0.0);
    ai.abil = "Fully-Charged Aimed Shot";
    ai.deployable = true;
    if weakspot {
        ai.weak_point = true;
        ai.hitlag_halt = 0.12 * 60.0;
        ai.hitlag_factor = 0.01;
    }
    ai
}

/// The Wreath Arrow: 142 frames shorter in Vijnana Suffusion (an arrow
/// spent) and 54 from C6; A1 a frame after it fires; the Clusterbloom
/// Arrows `wreath` frames after it lands.
fn wreath(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
    let travel = p.get("travel").map_or(10, |t| t as i32);
    let wreath_travel = p.get("wreath").map_or(35, |t| t as i32);
    let mut skip = 0;
    if s.mod_active(ci, VIJNANA) {
        skip = 142;
        let st = s.st_mut::<State>(ci);
        st.wreath_arrows -= 1;
        if st.wreath_arrows == 0 {
            s.delete_mod(ci, VIJNANA);
        }
    }
    if s.chars[ci].cons >= 6 {
        skip += 54;
    }
    skip = skip.min(g::AIMED_WREATH_HITMARK);
    let lvl = s.talent(ci, 0);
    let mut ai = arrow_info(ci, p.get("weakspot") == Some(1));
    ai.abil = "Fully-Charged Aimed Shot (Wreath Arrow)";
    ai.mult = g::WREATH[lvl];
    ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
    let h = g::AIMED_WREATH_HITMARK - skip;
    s.queue_attack(ai, 0, h, h + travel);
    let c = ci as u8;
    if s.chars[ci].ascension >= 1 {
        s.add_task(Task::Char { c, id: TASK_A1, arg: 0 }, h + 1);
    }
    s.add_task(Task::Char { c, id: TASK_CLUSTERBLOOM, arg: wreath_travel }, h + travel);
    let t = &g::AIMED_WREATH_FRAMES;
    ActInfo::new(Frames::Char { id: FR_WREATH, arg: skip as i16 }, t[0] - skip, h, AnimState::Aim)
}
