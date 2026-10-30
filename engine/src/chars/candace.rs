//! Candace, from gcsim's internal/characters/candace: normal attacks, the
//! skill pressed, held or perfect (with its shield), the burst with its
//! waves on swap and its infusion, A4 and every constellation. No charged
//! attacks or plunges.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::candace as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    wave_count: i32,
    burst_src: i32,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const BURST_DMG: ModKey = ModKey::new(g::BURST_DMG_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const INFUSE: ModKey = ModKey::new("candace-q-infuse");
const SHIELD: ModKey = ModKey::new("candace-skill-shield");
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const C2: ModKey = ModKey::new("candace-c2");
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const CB_C2: u16 = 1 << 0;
const CB_PARTICLE_2: u16 = 1 << 1;
const CB_PARTICLE_3: u16 = 1 << 2;

const TASK_BURST_START: u8 = 0;
// arg: the character infused * 10^8 + the burst's frame
const TASK_INFUSE: u8 = 1;

impl Character for Char {
    const KEY: &'static str = "candace";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        for i in 0..s.chars.len() {
            s.chars[i].mods.push(Mod::owned(BURST_DMG, ModKind::Attack, o, st::NONE, false, false, -1));
            if s.chars[ci].ascension >= 4 {
                s.chars[i].mods.push(Mod::owned(A4, ModKind::Attack, o, st::NONE, false, false, -1));
            }
        }
        s.subscribe(EventKind::Swap, o);
        if s.chars[ci].cons >= 6 {
            s.subscribe(EventKind::EnemyDamage, o);
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
            ai.blunt = g::ATTACK_STRIKE_TYPES[n][i] == 2;
            ai.poise = g::ATTACK_POISE_DMG[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_HITLAG_DEF_HALT[n][i];
            let h = g::ATTACK_HITMARKS[n][i];
            s.queue_attack(ai, 0, h, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let mut level = p.get("hold").unwrap_or(0).min(1).max(0) as usize;
        let mut windup = 0;
        if p.get("perfect").unwrap_or(0) != 0 {
            level = 1;
            windup = 55;
        }
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
        ai.abil = ["Sacred Rite: Heron's Sanctum", "Sacred Rite: Heron's Sanctum (Charged)"][level];
        ai.flat_dmg = g::SKILL_DMG[level][lvl] * s.max_hp(ci);
        ai.blunt = true;
        ai.poise = [150.0, 300.0][level];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = 0.05 * 60.0;
        ai.def_halt = true;
        let hitmark = g::SKILL_HITMARKS[level] - windup;
        let c2 = if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
        let particle = if level == 0 { CB_PARTICLE_2 } else { CB_PARTICLE_3 };
        s.queue_attack(ai, c2 | particle, hitmark, hitmark);
        let f = s.f;
        s.shields.retain(|x| !(x.owner as usize == ci && x.key == SHIELD));
        s.shields.push(Shield { owner: ci as u8, key: SHIELD, expiry: f + hitmark });
        let cd = if s.chars[ci].cons >= 4 { g::SKILL_CD[0] } else { g::SKILL_CD[level] };
        s.set_cd_with_delay(ci, Action::Skill, cd, g::SKILL_CD_STARTS[level]);
        let t = &g::SKILL_FRAMES[level];
        ActInfo::new(Frames::Char { id: level as u8, arg: windup as i16 }, t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn frames(level: u8, windup: i16, next: Action) -> i32 {
        g::SKILL_FRAMES[level as usize][next as usize] - windup as i32
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.st_mut::<State>(ci).wave_count = 0;
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
        ai.abil = "Sacred Rite: Wagtail's Tide (Initial)";
        ai.flat_dmg = g::BURST_DMG[lvl] * s.max_hp(ci);
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        let f = s.f;
        s.st_mut::<State>(ci).burst_src = f;
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_START, arg: f }, g::BURST_HITMARK);
        s.consume_energy(ci, 4);
        s.set_cd(ci, Action::Burst, 15 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_BURST_START => {
                let dur = if s.chars[ci].cons >= 1 { 720 } else { 540 };
                s.add_status(ci, BURST, dur, true);
                infuse(s, ci, ci, arg);
            }
            TASK_INFUSE => infuse(s, ci, (arg / 100_000_000) as usize, arg % 100_000_000),
            _ => unreachable!(),
        }
    }

    /// The burst's DMG bonus and A4's on elemental normal attacks.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if !s.mod_active(ci, BURST) || ev.info.tag != AttackTag::Normal || matches!(ev.info.element, Element::Physical | Element::NoElement) {
            return;
        }
        ev.stats[st::DMGP] += if m.key == BURST_DMG { 0.2 } else { 0.005 * s.max_hp(ci) / 1000.0 };
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_C2 != 0 {
            s.add_stat_mod(ci, C2, st::HPP, 0.2, 15 * 60, true);
        }
        if cbs & (CB_PARTICLE_2 | CB_PARTICLE_3) != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 30, true);
            let n = if cbs & CB_PARTICLE_2 != 0 { 2.0 } else { 3.0 };
            s.queue_particle(n, Element::Hydro, s.particle_delay[ci]);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // a wave on each of the first three swaps while the burst lasts
            Event::Swap { next, .. } => {
                if !s.mod_active(ci, BURST) {
                    return;
                }
                let src = s.st::<State>(ci).burst_src;
                infuse(s, ci, next, src);
                if s.st::<State>(ci).wave_count > 2 {
                    return;
                }
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
                ai.abil = "Sacred Rite: Wagtail's Tide (Wave)";
                ai.flat_dmg = g::BURST_WAVE_DMG[lvl] * s.max_hp(ci);
                ai.hitlag_factor = 0.01;
                ai.def_halt = true;
                s.queue_attack(ai, 0, g::WAVE_HITMARK, g::WAVE_HITMARK);
                s.st_mut::<State>(ci).wave_count += 1;
            }
            // C6: an elemental normal attack by the active character
            Event::EnemyDamage(ev) => {
                let actor = ev.info.actor as usize;
                if ev.info.tag != AttackTag::Normal || matches!(ev.info.element, Element::Physical | Element::NoElement) {
                    return;
                }
                if actor != s.player.active as usize || actor == ci || !s.mod_active(ci, BURST) || s.mod_active(ci, C6_ICD) || ev.damage == 0.0 {
                    return;
                }
                s.add_status(ci, C6_ICD, 138, true);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
                ai.abil = "The Overflow (C6)";
                ai.flat_dmg = 0.15 * s.max_hp(ci);
                ai.def_halt = true;
                s.queue_attack(ai, 0, g::WAVE_HITMARK, g::WAVE_HITMARK);
            }
            _ => {}
        }
    }
}

/// burstInfuseFn: hydro on the active character's weapon (sword,
/// claymore or polearm) for a second, renewed every half second while the
/// burst lasts and the character is on field.
fn infuse(s: &mut Sim, ci: usize, on: usize, src: i32) {
    if src != s.st::<State>(ci).burst_src || s.player.active as usize != on || !s.mod_active(ci, BURST) {
        return;
    }
    if matches!(s.chars[on].class, WeaponClass::Claymore | WeaponClass::Spear | WeaponClass::Sword) {
        s.add_infusion(on, INFUSE, Element::Hydro, 60, true, &[AttackTag::Normal, AttackTag::Extra, AttackTag::Plunge]);
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_INFUSE, arg: on as i32 * 100_000_000 + src }, 30);
}
