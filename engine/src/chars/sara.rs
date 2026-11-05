//! Kujou Sara, from gcsim's internal/characters/sara: normal attacks,
//! aimed shots (with A1's quick shot and Tengu Juurai: Ambush in Crowfeather
//! Cover), Tengu Stormcall (the cover and C2's ambush), Subjugation:
//! Koukou Sendou (the Titanbreaker and its Stormclusters, 4 lines or 6 at
//! C4), the ATK buff her ambushes and bursts give the active member when
//! they reach the player, A4's energy and every constellation. gcsim has
//! no plunges for her.
//!
//! Whether a buff's area reaches the player is decided when the buff
//! lands, where the player stands then, as gcsim's attackBuff does.

use crate::content::{Character, Event};
use crate::gamedata::chars::sara as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State;

const COVER: ModKey = ModKey::new(g::COVER_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);
const ATK_BUFF: ModKey = ModKey::new("sara-attack-buff");

const CB_A4: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;
const CB_C1: u16 = 1 << 2;

/// attackBuff on the fight's queue; arg 1 when its area reaches the player.
const TASK_ATTACK_BUFF: u8 = 0;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];

impl Character for Char {
    const KEY: &'static str = "kujousara";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].cons >= 6 {
            s.subscribe(EventKind::SpecialReactionAttack, Owner::Char(ci as u8));
        }
    }

    /// gcsim allows travel on her normal attacks, and hold (0 or 1), travel
    /// and weakspot on her aimed shots.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let bad = |known: &[&str]| p.0.iter().find(|(k, _)| !known.iter().any(|n| k == *n)).map(|(k, _)| format!("{name} param {k}"));
        match a {
            Action::Attack => bad(&["travel"]),
            Action::Aim => {
                if !matches!(p.get("hold"), None | Some(0) | Some(1)) {
                    return Some("aim[hold]".into());
                }
                bad(&["hold", "travel", "weakspot"])
            }
            Action::Skill | Action::Burst => None,
            _ => Some(name.into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        // NewBoxHit(Player, PrimaryTarget, {Y: -0.5}, 0.1, 1)
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let h = g::ATTACK_HITMARKS[n];
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
        let skill_active = s.chars[ci].ascension >= 1 && s.core_status(g::COVER_KEY) > 0;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::FULLAIM[lvl]);
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
        let quick = skill_active && hold == 1;
        let (table, queue_after) = if quick {
            ai.abil = "Fully-Charged Aimed Shot (A1)";
            (&g::AIMED_A1_FRAMES, g::AIMED_A1_HITMARK)
        } else {
            (&g::AIMED_FRAMES[hold], g::AIMED_HITMARKS[hold])
        };
        // NewBoxHit(Player, PrimaryTarget, {Y: -0.5}, 0.1, 1)
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        s.queue_attack(ai, 0, queue_after, queue_after + travel);
        if quick {
            let lvl = s.talent(ci, 1);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL[lvl]);
            ai.abil = "Tengu Juurai: Ambush";
            // NewCircleHit(Player, PrimaryTarget, nil, 6)
            ai.pattern = s.circle_on_target(6.0);
            let cbs = a4_cb(s, ci) | CB_PARTICLE;
            let h = g::AIMED_A1_HITMARK;
            s.queue_attack(ai, cbs, h, h + travel + 90);
            attack_buff(s, ci, ai.pattern, h + travel + 90 + 1);
            s.delete_core_status(COVER);
        }
        ActInfo::new(Frames::Table(table), table[0], queue_after, AnimState::Aim)
    }

    /// Tengu Stormcall: Crowfeather Cover for 18 s, and C2's ambush.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_core_status(COVER, 18 * 60);
        if s.chars[ci].cons >= 2 {
            let lvl = s.talent(ci, 1);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, 0.3 * g::SKILL[lvl]);
            ai.abil = "Tengu Juurai: Ambush (C2)";
            // NewCircleHitOnTarget(Player, nil, 6)
            ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
            let cbs = a4_cb(s, ci);
            s.queue_attack(ai, cbs, 50, g::C2_HITMARK);
            attack_buff(s, ci, ai.pattern, g::C2_HITMARK + 1);
        }
        s.set_cd_with_delay(ci, Action::Skill, 600, 7);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Skill)
    }

    /// Subjugation: Koukou Sendou: the Titanbreaker on the enemy, then six
    /// Stormclusters along each of 4 (6 at C4) lines turned from the
    /// player's facing.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::BURST_MAIN[lvl]);
        ai.abil = "Tengu Juurai: Titanbreaker";
        let c1 = if s.chars[ci].cons >= 1 { CB_C1 } else { 0 };
        let dir = s.player_dir;
        let pos = s.enemy_pos;
        // NewCircleHitOnTarget(the enemy's position, nil, 6)
        ai.pattern = Pattern::Circle { x: pos.0, y: pos.1, r: 6.0 };
        s.queue_attack(ai, c1, g::BURST_START, g::BURST_INITIAL_HITMARK);
        attack_buff(s, ci, ai.pattern, g::BURST_INITIAL_HITMARK + 1);

        ai.abil = "Tengu Juurai: Stormcluster";
        ai.icd_tag = icd::TAG_ELEMENTAL_BURST;
        ai.mult = g::BURST_CLUSTER[lvl];
        let count = if s.chars[ci].cons >= 4 { 6.0 } else { 4.0 };
        let step = 360.0 / count;
        let mut i = 0.0;
        while i < count {
            let (sx, sy) = degrees_to_direction(i * step);
            let d = rotate(sx, sy, dir.0, dir.1);
            for j in 0..6 {
                let (x, y) = offset_point(pos, (0.0, 3.6 + 1.35 * j as f64), d);
                ai.pattern = Pattern::Circle { x, y, r: 3.0 };
                let h = g::BURST_CLUSTER_HITMARK + 18 * j;
                s.queue_attack(ai, c1, g::BURST_START, h);
                attack_buff(s, ci, ai.pattern, h + 1);
            }
            i += 1.0;
        }
        s.set_cd_with_delay(ci, Action::Burst, 20 * 60, g::BURST_START);
        s.consume_energy(ci, 50);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_ATTACK_BUFF => {
                let p = s.slots[arg as usize].info.pattern;
                s.free.push(arg as u16);
                if !s.hits_player(p) {
                    return;
                }
                let active = s.player.active as usize;
                let lvl = s.talent(ci, 1);
                let buff = g::ATK_BUFF[lvl] * s.stat(ci, st::BASEATK);
                s.add_mod(active, Mod::stat(ATK_BUFF, st::ATK, buff, true, true, -1), 360);
                // Player.Drain of nothing, from outside the party
                s.emit(Event::Drain { on: active, amount: 0.0, external: true });
                if s.chars[ci].cons >= 1 {
                    c1(s, ci);
                }
                if s.chars[ci].cons >= 6 {
                    s.add_mod(active, Mod::owned(C6, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 360);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_A4 != 0 && !s.mod_active(ci, A4_ICD) {
            s.add_status(ci, A4_ICD, 180, true);
            let amt = 1.2 * s.stat_ex(ci, st::ER, true);
            for i in 0..s.chars.len() {
                s.add_energy(i, amt);
            }
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 6, false);
            s.queue_particle(3.0, Element::Electro, s.particle_delay[ci]);
        }
        if cbs & CB_C1 != 0 {
            c1(s, ci);
        }
    }

    /// C6: electro CRIT DMG +60% on the buffed member.
    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C6 && ev.info.element == Element::Electro {
            ev.stats[st::CD] += 0.6;
        }
    }

    fn event(s: &mut Sim, _: usize, e: &Event) {
        // C6 on Lunar-Charged: CRIT DMG +60% for a contributor with the buff
        if let Event::SpecialReactionAttack(ev) = *e {
            if ev.info.tag == AttackTag::ReactionLunarCharge && s.mod_active(ev.info.actor as usize, C6) {
                s.special_atk.stats[st::CD] += 0.6;
            }
        }
    }
}

/// makeA4CB: from A4, the ambush gives the party energy.
fn a4_cb(s: &Sim, ci: usize) -> u16 {
    if s.chars[ci].ascension >= 4 { CB_A4 } else { 0 }
}

/// attackBuff: delay frames later, if the area reaches the player where
/// the player is then (AttackWillLand in the task). A slot holds the area.
fn attack_buff(s: &mut Sim, ci: usize, p: Pattern, delay: i32) {
    let mut ev = AttackEvent::new(AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::NoElement, 0.0, 0.0), 0);
    ev.info.pattern = p;
    let slot = s.alloc(ev);
    s.add_task(Task::Char { c: ci as u8, id: TASK_ATTACK_BUFF, arg: slot as i32 }, delay);
}

/// C1: the skill's cooldown −1 s, once every 3 s.
fn c1(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, C1_ICD) {
        return;
    }
    s.add_status(ci, C1_ICD, 180, true);
    s.reduce_cd(ci, Action::Skill, 60);
}
