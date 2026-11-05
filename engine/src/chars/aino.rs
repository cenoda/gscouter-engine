//! Aino, from gcsim's internal/characters/aino: normal attacks,
//! Musecatcher (tap; gcsim has no hold), Precision Hydronic Cooler, A1, A4
//! and every constellation. No charged attack or plunges (gcsim has none
//! for her).
//!
//! As in gcsim:
//! - Her skill's second hit is copied before it is made blunt, so neither
//!   hit has hitlag or poise.
//! - Her burst's water balls are tasks of the fight (no hitlag), queued at
//!   the cast: one every 1.5 s of radius 1, or with A1 and Ascendant Gleam
//!   (Moonsign level 2) one every 0.7 s of radius 4 on their own ICD. Each
//!   lands on a random enemy within 10 of where the player was at the cast
//!   that no ball targeted in the last 0.8 s, else on a random point 1.5
//!   to 9 from there. A4's flat damage (50% of EM) is read at each ball.
//! - C2's hit comes on a hit on the enemy by the active character while
//!   she is off field and her burst lasts, once every 5 s.

use crate::content::{Character, Event};
use crate::gamedata::chars::aino as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The centers of the burst areas, of the last two casts (gcsim's
    /// burstArea, on the player at the cast); a burst's balls last less
    /// than two of its cooldowns.
    centers: [(f64, f64); 2],
    casts: i32,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const BURST_MARK: ModKey = ModKey::new(g::BURST_MARK_KEY);
const C1: ModKey = ModKey::new(g::C1_KEY);
const C1_BUFF: ModKey = ModKey::new("aino-c1-buff");
const C2_ICD: ModKey = ModKey::new(g::C2_ICD_KEY);
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);
const C6_BUFF: ModKey = ModKey::new("aino-c6-buff");

const CB_PARTICLE: u16 = 1 << 0;
const CB_C4: u16 = 1 << 1;

// arg: the cast's slot in `centers`
const TASK_BALL: u8 = 0;
const TASK_BURST_STATUS: u8 = 1;

impl Character for Char {
    const KEY: &'static str = "aino";
    const MOONSIGN: i32 = 1;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    /// gcsim's Init: C1, C2, C6.
    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        if s.chars[ci].cons >= 1 {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(C1_BUFF, ModKind::Stat, o, st::EM, false, false, -1));
            }
        }
        if s.chars[ci].cons >= 2 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        if s.chars[ci].cons >= 6 {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(C6_BUFF, ModKind::ReactBonus, o, st::NONE, false, false, -1));
            }
        }
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
            ai.blunt = true;
            ai.poise = g::ATTACK_POISE_DMG[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let off = g::ATTACK_OFFSETS[n][i];
            let hb = g::ATTACK_HITBOXES[n][i];
            ai.pattern = if n == 2 && i == 0 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
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
        const NAMES: [&str; 2] = ["Musecatcher 0", "Musecatcher 1"];
        for (i, mult) in g::SKILL.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Hydro, 25.0, mult[lvl]);
            ai.abil = NAMES[i];
            // the first on the enemy, the second on the player; gcsim makes
            // the second blunt with hitlag after copying it, so it has none
            ai.pattern = if i == 0 { s.circle_on_enemy(0.0, g::SKILL_OFFSETS[i], g::SKILL_HITBOXES[i]) } else { s.circle_on_player(0.0, g::SKILL_OFFSETS[i], g::SKILL_HITBOXES[i]) };
            s.queue_attack_at(ci, ai, CB_PARTICLE | CB_C4, g::SKILL_HITMARKS[i]);
        }
        s.set_cd_with_delay(ci, Action::Skill, 10 * 60, 13);
        c1_on_skill_burst(s, ci);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let (rate, _, _, _) = a1_burst(s, ci);
        let center = s.player_pos;
        let st = s.st_mut::<State>(ci);
        let slot = st.casts & 1;
        st.casts += 1;
        st.centers[slot as usize] = center;
        let mut i = 0;
        while i < 852 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_BALL, arg: slot }, i + g::BURST_START);
            i += rate;
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_STATUS, arg: 0 }, g::BURST_START);
        s.set_cd(ci, Action::Burst, 810);
        s.consume_energy(ci, 5);
        c1_on_skill_burst(s, ci);
        if s.chars[ci].cons >= 6 {
            s.add_status(ci, C6, 15 * 60, true);
        }
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_BALL => {
                let (_, r, group, tag) = a1_burst(s, ci);
                let (cx, cy) = s.st::<State>(ci).centers[arg as usize];
                let area = Pattern::Circle { x: cx, y: cy, r: 10.0 };
                // RandomEnemyWithinArea, filtered to an enemy no ball
                // targeted in the last 0.8 s: the one enemy
                let pos = if s.enemy_alive && !s.enemy_status_active(BURST_MARK) && s.enemy_within(area) {
                    let _ = s.rng.intn(1);
                    s.enemy_add_status(BURST_MARK, 48, true);
                    s.enemy_pos
                } else {
                    s.random_point(cx, cy, 1.5, 9.0)
                };
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, tag, group, Element::Hydro, 25.0, g::BURST[lvl]);
                ai.abil = "Cool Your Jets Ducky";
                ai.flat_dmg = a4_dmg(s, ci);
                ai.pattern = Pattern::Circle { x: pos.0, y: pos.1, r };
                s.queue_attack(ai, 0, 0, 10);
            }
            TASK_BURST_STATUS => s.add_status(ci, BURST, 14 * 60, false),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 36, true);
            s.queue_particle(3.0, Element::Hydro, s.particle_delay[ci]);
        }
        // C4: 10 energy, once every 10 s
        if cbs & CB_C4 != 0 && s.chars[ci].cons >= 4 && !s.mod_active(ci, C4_ICD) {
            s.add_status(ci, C4_ICD, 10 * 60, true);
            s.add_energy(ci, 10.0);
        }
    }

    /// C1: 80 EM for her, and for the others while they are active, while
    /// C1's status lasts.
    fn mod_amount(s: &Sim, ci: usize, on: usize, _: &Mod, out: &mut Stats) -> bool {
        if on != ci && s.player.active as usize != on {
            return false;
        }
        if !s.mod_active(ci, C1) {
            return false;
        }
        out[st::EM] += 80.0;
        true
    }

    /// C6: the active character's lunar, electro-charged and bloom damage
    /// +15%, +35% with Ascendant Gleam, while C6's status lasts.
    fn react_bonus(s: &Sim, ci: usize, on: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if !s.mod_active(ci, C6) {
            return 0.0;
        }
        if s.player.active as usize != on {
            return 0.0;
        }
        // gcsim's Bountiful Core damage is tagged Bloom (its own tag is
        // never set), as the engine's is
        if !info.tag.is_lunar() && !matches!(info.tag, AttackTag::ECDamage | AttackTag::Bloom) {
            return 0.0;
        }
        if moonsign_level(s) >= 2 {
            0.35
        } else {
            0.15
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        // C2: a hit on the enemy by the active character, while she is off
        // field and her burst lasts, adds a hit around it, once every 5 s
        if let Event::EnemyHit(ev) = *e {
            let active = s.player.active as usize;
            if active == ci || ev.info.actor as usize != active {
                return;
            }
            if !s.mod_active(ci, BURST) || s.mod_active(ci, C2_ICD) {
                return;
            }
            s.add_status(ci, C2_ICD, 5 * 60, true);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.25);
            ai.abil = "Cool Your Jets Ducky (C2)";
            ai.flat_dmg = s.stat(ci, st::EM) + a4_dmg(s, ci);
            ai.pattern = s.circle_on_target(1.5);
            s.queue_attack(ai, 0, 0, 10);
        }
    }
}

/// gcsim's GetMoonsignLevel: the Moonsign of the team.
fn moonsign_level(s: &Sim) -> i32 {
    s.chars.iter().map(|c| c.moonsign).sum()
}

/// A1: the burst's tick rate, radius, ICD group and tag.
fn a1_burst(s: &Sim, ci: usize) -> (i32, f64, IcdGroup, IcdTag) {
    if s.chars[ci].ascension < 1 || moonsign_level(s) < 2 {
        return (90, 1.0, icd::GROUP_DEFAULT, icd::TAG_ELEMENTAL_BURST);
    }
    (42, 4.0, icd::GROUP_AINO_BURST_MOON_HIT, icd::TAG_AINO_BURST_MOON_HIT)
}

/// A4: flat damage of 50% of her EM on the burst's hits.
fn a4_dmg(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 4 {
        return 0.0;
    }
    s.stat(ci, st::EM) * 0.5
}

fn c1_on_skill_burst(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons >= 1 {
        s.add_status(ci, C1, 15 * 60, true);
    }
}
