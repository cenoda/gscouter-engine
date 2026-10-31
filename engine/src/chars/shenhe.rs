//! Shenhe, from gcsim's internal/characters/shenhe: normal and charged
//! attacks, the pressed and held skill with their Icy Quills (flat cryo
//! DMG from her ATK at each hit), the burst's field (its DoT, cryo and
//! physical shred), A1, A4 and every constellation. C2's bonus to
//! Stellar-Conduct reactions is out of reach: the engine has none. No
//! plunges.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::shenhe as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// Icy Quills left, by character (gcsim's tag on each).
    quills: [i32; 4],
    c4_count: i32,
}

const QUILL: ModKey = ModKey::new(g::QUILL_KEY);
const HOLD_PARTICLE_ICD: ModKey = ModKey::new(g::HOLD_PARTICLE_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const A1: ModKey = ModKey::new("shenhe-a1");
const A4_PRESS: ModKey = ModKey::new("shenhe-a4-press");
const A4_HOLD: ModKey = ModKey::new("shenhe-a4-hold");
const C2: ModKey = ModKey::new(g::C2_BUFF_KEY);
const C4: ModKey = ModKey::new(g::C4_BUFF_KEY);
const SHRED_CRYO: ModKey = ModKey::new("shenhe-burst-shred-cryo");
const SHRED_PHYS: ModKey = ModKey::new("shenhe-burst-shred-phys");

const CB_PRESS_PARTICLE: u16 = 1 << 0;
const CB_HOLD_PARTICLE: u16 = 1 << 1;

// arg: the normal attack's counter times 2 plus its hit
const TASK_NORMAL: u8 = 0;
// arg: 1 for hold
const TASK_SKILL_HIT: u8 = 1;
const TASK_QUILLS: u8 = 2;
const TASK_DOT: u8 = 3;
const TASK_FIELD: u8 = 4;

impl Character for Char {
    const KEY: &'static str = "shenhe";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].cons >= 1 {
            let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
            cd.available = 2;
            cd.max_extra = 1;
        }
        s.subscribe(EventKind::EnemyHit, Owner::Char(ci as u8));
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let hits = g::ATTACK_HITMARKS[n];
        for (i, &h) in hits.iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: (n * 2 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hits[hits.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGED[lvl]);
        ai.abil = "Charge";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = s.circle_on_target(0.8);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").is_some_and(|h| h != 0);
        let c = ci as u8;
        let (hitmark, cd_start, cd, t) = if hold {
            (g::SKILL_HOLD_HITMARK, g::SKILL_HOLD_CD_START + 1, 15 * 60, &g::SKILL_HOLD_FRAMES)
        } else {
            (g::SKILL_PRESS_HITMARK, g::SKILL_PRESS_CD_START, 10 * 60, &g::SKILL_PRESS_FRAMES)
        };
        s.add_task(Task::Char { c, id: TASK_SKILL_HIT, arg: hold as i32 }, hitmark);
        let quills_at = if hold { g::SKILL_HOLD_CD_START + 1 } else { g::SKILL_PRESS_CD_START + 1 };
        s.add_task(Task::Char { c, id: TASK_QUILLS, arg: hold as i32 }, quills_at);
        s.set_cd_with_delay(ci, Action::Skill, cd, cd_start);
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let c = ci as u8;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST[lvl]);
        ai.abil = "Divine Maiden's Deliverance (Initial)";
        ai.blunt = true;
        ai.poise = 250.0;
        // the field: a circle of radius 8, 2 in front of the player
        ai.pattern = s.fan_on_player(0.0, 2.0, 8.0, 120.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        let duration = if s.chars[ci].cons >= 2 { 18 * 60 } else { 12 * 60 };
        s.add_status(ci, BURST, duration, false);
        s.add_task(Task::Char { c, id: TASK_DOT, arg: 0 }, g::BURST_START);
        let mut i = g::BURST_START;
        while i < g::BURST_START + duration {
            s.add_task(Task::Char { c, id: TASK_FIELD, arg: 0 }, i);
            i += 18;
        }
        s.set_cd(ci, Action::Burst, 20 * 60);
        s.consume_energy(ci, 4);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = (arg as usize / 2, arg as usize % 2);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.def_halt = g::ATTACK_DEF_HALT[n][i];
                let (off, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
                ai.pattern = if n == 3 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.fan_on_player(0.0, off, hb[0], g::ATTACK_FAN_ANGLES[n]) };
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_SKILL_HIT => {
                let hold = arg == 1;
                let lvl = s.talent(ci, 1);
                let ai = if hold {
                    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 50.0, g::SKILL_HOLD[lvl]);
                    ai.abil = "Spring Spirit Summoning (Hold)";
                    ai.pattern = s.circle_on_player(0.0, 1.5, 4.0);
                    ai
                } else {
                    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL_PRESS[lvl]);
                    ai.abil = "Spring Spirit Summoning (Press)";
                    ai.hitlag_factor = 0.01;
                    ai.def_halt = true;
                    ai.deployable = true;
                    ai.pattern = s.circle_on_target(0.8);
                    ai
                };
                let cbs = if hold { CB_HOLD_PARTICLE } else { CB_PRESS_PARTICLE };
                let mut ev = s.snapshot_event(ai, cbs);
                ev.stats[st::DMGP] += c4_consume(s, ci);
                s.queue_attack_with_snap(ev, cbs, 0);
            }
            // every character gets 5 quills for 10 s (7 for 15 s held),
            // then A4's buff
            TASK_QUILLS => {
                let hold = arg == 1;
                let (dur, n) = if hold { (15 * 60, 7) } else { (10 * 60, 5) };
                for i in 0..s.chars.len() {
                    s.add_status(i, QUILL, dur, true);
                    s.st_mut::<State>(ci).quills[i] = n;
                }
                if s.chars[ci].ascension >= 4 {
                    let (key, dur) = if hold { (A4_HOLD, 15 * 60) } else { (A4_PRESS, 10 * 60) };
                    for i in 0..s.chars.len() {
                        s.add_mod(i, Mod::owned(key, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), dur);
                    }
                }
            }
            // the DoT snapshots as the field starts, before A1
            TASK_DOT => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURSTDOT[lvl]);
                ai.abil = "Divine Maiden's Deliverance (DoT)";
                ai.pattern = s.circle_on_player(0.0, 2.0, 7.0);
                let ev = s.snapshot_event(ai, 0);
                let count = if s.chars[ci].cons >= 2 { 9 } else { 6 };
                for i in 0..count {
                    let hitmark = 82 + i * 117;
                    for j in 0..2 {
                        s.queue_attack_with_snap(ev, 0, hitmark + j * (30 + g::BURST_TICK_OFFSET[i as usize]));
                    }
                }
            }
            // every 0.3 s: A1 and C2 on the active character, who is in the
            // field, and the shred on the enemy, for 0.6 s
            TASK_FIELD => {
                let active = s.player.active as usize;
                if s.chars[ci].ascension >= 1 {
                    s.add_stat_mod(active, A1, st::CRYOP, 0.15, 36, true);
                }
                if s.chars[ci].cons >= 2 {
                    s.add_mod(active, Mod::owned(C2, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 36);
                }
                let shred = g::BURSTRESPP[s.talent(ci, 2)];
                s.enemy_add_resist(SHRED_CRYO, Element::Cryo, -shred, 36);
                s.enemy_add_resist(SHRED_PHYS, Element::Physical, -shred, 36);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PRESS_PARTICLE != 0 {
            s.queue_particle(3.0, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_HOLD_PARTICLE != 0 && !s.mod_active(ci, HOLD_PARTICLE_ICD) {
            s.add_status(ci, HOLD_PARTICLE_ICD, 30, true);
            s.queue_particle(4.0, Element::Cryo, s.particle_delay[ci]);
        }
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        let tag = ev.info.tag;
        if m.key == A4_PRESS {
            if matches!(tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::ElementalBurst) {
                ev.stats[st::DMGP] += 0.15;
            }
        } else if m.key == A4_HOLD {
            if matches!(tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
                ev.stats[st::DMGP] += 0.15;
            }
        } else if m.key == C2 && ev.info.element == Element::Cryo {
            ev.stats[st::CD] += 0.15;
        }
    }

    /// Icy Quills: a cryo hit of these kinds by a character with quills
    /// adds her current ATK times the skill's multiplier, using up a quill
    /// (C6: not for normal and charged attacks). C4 counts each.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if ev.info.element != Element::Cryo {
            return;
        }
        let consume = match ev.info.tag {
            AttackTag::ElementalBurst | AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::Plunge => true,
            AttackTag::Normal | AttackTag::Extra => s.chars[ci].cons < 6,
            _ => return,
        };
        let a = ev.info.actor as usize;
        if !s.mod_active(a, QUILL) || s.st::<State>(ci).quills[a] <= 0 {
            return;
        }
        let amt = g::SKILLPP[s.talent(ci, 1)] * total_atk(&s.stats(ci));
        if consume {
            s.st_mut::<State>(ci).quills[a] -= 1;
        }
        s.hit_flat += amt;
        // C4: a Skyfrost Mantra stack, up to 50, for 60 s
        if s.chars[ci].cons >= 4 {
            if !s.mod_active(ci, C4) {
                s.st_mut::<State>(ci).c4_count = 0;
            }
            let st = s.st_mut::<State>(ci);
            if st.c4_count < 50 {
                st.c4_count += 1;
            }
            s.add_status(ci, C4, 3600, true);
        }
    }
}

/// C4: the skill uses up the stacks, 5% DMG each.
fn c4_consume(s: &mut Sim, ci: usize) -> f64 {
    if s.chars[ci].cons < 4 {
        return 0.0;
    }
    if !s.mod_active(ci, C4) {
        s.st_mut::<State>(ci).c4_count = 0;
        return 0.0;
    }
    let st = s.st_mut::<State>(ci);
    let bonus = 0.05 * st.c4_count as f64;
    st.c4_count = 0;
    s.delete_mod(ci, C4);
    bonus
}
