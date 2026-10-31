//! Rosaria, from gcsim's internal/characters/rosaria: normal and charged
//! attacks, the skill (from behind unless `nobehind`, for A1), the burst
//! and its lance, A1, A4 and every constellation. No plunges.

use crate::content::Character;
use crate::gcsimdata::chars::rosaria as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// A1 and C4 trigger once per skill.
    a1_done: bool,
    c4_done: bool,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST_STATUS: ModKey = ModKey::new("rosariaburst");
const A1: ModKey = ModKey::new("rosaria-a1");
const A4: ModKey = ModKey::new("rosaria-a4");
const C1_DMG: ModKey = ModKey::new("rosaria-c1-dmg");
const C1_SPEED: ModKey = ModKey::new("rosaria-c1-speed");
const C6: ModKey = ModKey::new("rosaria-c6");

const CB_A1: u16 = 1 << 0;
const CB_C1: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;
const CB_C6: u16 = 1 << 3;
const CB_PARTICLE: u16 = 1 << 4;

// arg: the normal's index * 8 + the hit's
const TASK_NORMAL: u8 = 0;
const TASK_SKILL_HIT_2: u8 = 1;
const TASK_LANCE: u8 = 2;

impl Character for Char {
    const KEY: &'static str = "rosaria";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let hitmarks = g::ATTACK_HITMARKS[n];
        for (i, &h) in hitmarks.iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: (n * 8 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hitmarks[hitmarks.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::NC[lvl]);
        ai.abil = "Charge";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = s.circle_on_target(0.8);
        s.queue_attack(ai, c1(s, ci), g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        ActInfo::new(Frames::Table(&g::CHARGE_FRAMES), g::CHARGE_FRAMES[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL[0][lvl]);
        ai.abil = "Ravaging Confession (Hit 1)";
        ai.hitlag_halt = 0.06 * 60.0;
        ai.hitlag_factor = 0.01;
        // A1 is assumed on the first hit, unless she is not behind
        let a1 = if p.get("nobehind") != Some(1) && s.chars[ci].ascension >= 1 { CB_A1 } else { 0 };
        let c4 = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
        let st = s.st_mut::<State>(ci);
        st.a1_done = false;
        st.c4_done = false;
        let cbs = a1 | c1(s, ci) | c4;
        ai.pattern = s.box_on_player(0.0, -1.0, 2.0, 4.0);
        s.queue_attack(ai, cbs, g::SKILL_HITMARK, g::SKILL_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_HIT_2, arg: 0 }, g::SKILL_HITMARK + 14);
        s.set_cd_with_delay(ci, Action::Skill, 360, 23);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST[0][lvl]);
        ai.abil = "Rites of Termination (Initial 1)";
        ai.hitlag_halt = 0.06 * 60.0;
        ai.hitlag_factor = 0.01;
        let cbs = c1(s, ci) | c6(s, ci);
        ai.pattern = s.circle_on_player(0.0, 0.5, 3.5);
        s.queue_attack(ai, cbs, 15, 15);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_LANCE, arg: 0 }, 56);
        // A4: a share of her CRIT Rate to the others
        if s.chars[ci].ascension >= 4 {
            let share = (0.15 * s.stat_ex(ci, st::CR, true)).min(0.15);
            for i in 0..s.chars.len() {
                if i != ci {
                    s.add_mod(i, Mod::stat(A4, st::CR, share, true, true, -1), 600);
                }
            }
        }
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 6);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = ((arg / 8) as usize, (arg % 8) as usize);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.def_halt = g::ATTACK_DEF_HALT[n][i];
                let (off, hb) = (g::ATTACK_OFFSETS[n][i], g::ATTACK_HITBOXES[n][i]);
                ai.pattern = if n == 2 || (n == 4 && i == 0) { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.fan_on_player(0.0, off, hb[0], g::ATTACK_FAN_ANGLES[n]) };
                let cbs = c1(s, ci);
                s.queue_attack(ai, cbs, 0, 0);
            }
            TASK_SKILL_HIT_2 => {
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL[1][lvl]);
                ai.abil = "Ravaging Confession (Hit 2)";
                ai.hitlag_halt = 0.09 * 60.0;
                ai.hitlag_factor = 0.01;
                ai.def_halt = true;
                let c4 = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
                let cbs = CB_PARTICLE | c1(s, ci) | c4;
                ai.pattern = s.circle_on_player(0.0, 0.5, 2.8);
                s.queue_attack(ai, cbs, 0, 0);
            }
            TASK_LANCE => {
                let lvl = s.talent(ci, 2);
                let cbs = c1(s, ci) | c6(s, ci);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST[1][lvl]);
                ai.abil = "Rites of Termination (Initial 2)";
                ai.hitlag_factor = 0.01;
                // around the lance, 3 in front of the player
                ai.pattern = s.circle_on_player(0.0, 3.0, 6.0);
                s.queue_attack(ai, cbs, 0, 0);
                let dur = if s.chars[ci].cons >= 2 { 510 + 240 } else { 510 };
                s.add_core_status(BURST_STATUS, dur);
                // the DoT snapshots as the lance lands
                let mut dot = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST_DOT[lvl]);
                dot.abil = "Rites of Termination (DoT)";
                dot.pattern = s.circle_on_player(0.0, 3.0, 6.5);
                for i in (120..dur).step_by(120) {
                    s.queue_attack(dot, cbs, 0, i);
                }
            }
            _ => unreachable!(),
        }
    }

    fn mod_amount(s: &Sim, _: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        // C1's attack speed, during normal attacks
        if s.current_state() != AnimState::Normal {
            return false;
        }
        out[st::ATKSPD] += 0.1;
        true
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        // C1's normal attack DMG
        if ev.info.tag == AttackTag::Normal {
            ev.stats[st::DMGP] += 0.1;
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_A1 != 0 && !s.st::<State>(ci).a1_done {
            s.st_mut::<State>(ci).a1_done = true;
            s.add_stat_mod(ci, A1, st::CR, 0.12, 300, true);
        }
        let active = s.player.active as usize == ci;
        if cbs & CB_C1 != 0 && active && ev.crit {
            s.add_mod(ci, Mod::owned(C1_DMG, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 240);
            s.add_mod(ci, Mod::owned(C1_SPEED, ModKind::Stat, Owner::Char(ci as u8), st::ATKSPD, false, true, -1), 240);
        }
        if cbs & CB_C4 != 0 && active && ev.crit && !s.st::<State>(ci).c4_done {
            s.st_mut::<State>(ci).c4_done = true;
            s.add_energy(ci, 5.0);
        }
        if cbs & CB_C6 != 0 {
            s.enemy_add_resist(C6, Element::Physical, -0.2, 600);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 36, true);
            s.queue_particle(3.0, Element::Cryo, s.particle_delay[ci]);
        }
    }
}

fn c1(s: &Sim, ci: usize) -> u16 {
    if s.chars[ci].cons >= 1 { CB_C1 } else { 0 }
}

fn c6(s: &Sim, ci: usize) -> u16 {
    if s.chars[ci].cons >= 6 { CB_C6 } else { 0 }
}
