//! Razor, from gcsim's internal/characters/razor: normal attacks with the
//! burst's Wolf Within, the press and hold skill with its Electro Sigils,
//! the burst, A1, A4, every constellation, his Hexerei parts (the Wolf
//! Within's flat damage, C6's three sigils and the CRIT buff, which need
//! only his own Hexerei; Surge of Lightning, which needs a second Hexerei
//! character), and the plunges from Xianyun's airborne state.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::razor as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    sigils: i32,
    /// The enemy's HP at the start, for C2.
    enemy_max_hp: f64,
}

const BURST: ModKey = ModKey::new(g::BURST_BUFF_KEY);
const SIGIL: ModKey = ModKey::new(g::SKILL_SIGIL_KEY);
const PRESS_PARTICLE_ICD: ModKey = ModKey::new(g::PRESS_PARTICLE_ICD_KEY);
const HOLD_PARTICLE_ICD: ModKey = ModKey::new(g::HOLD_PARTICLE_ICD_KEY);
const HEX_ICD: ModKey = ModKey::new(g::HEXEREI_ICD_KEY);
const HEX_C6: ModKey = ModKey::new(g::HEXEREI_C6_BUFF_KEY);
const A4: ModKey = ModKey::new("razor-a4");
const C1: ModKey = ModKey::new("razor-c1");
const C2: ModKey = ModKey::new("razor-c2");
const C4: ModKey = ModKey::new("razor-c4");
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

// callbacks, run in the order gcsim gives them to each attack
const CB_PRESS_PARTICLE: u16 = 1 << 0;
const CB_HOLD_PARTICLE: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;
const CB_SIGIL: u16 = 1 << 3;
/// The Wolf Within after normal attack n: bit 4 + n.
const CB_WOLF: u16 = 1 << 4;
const CB_C6: u16 = 1 << 8;
/// C6's lightning: a sigil if the burst is not up as it hits.
const CB_C6_SIGIL: u16 = 1 << 9;

const TASK_BURST_BUFF: u8 = 0;
const TASK_CLEAR_SIGIL: u8 = 1;
const TASK_SURGE_ENERGY: u8 = 2;

const WOLF_NAMES: [&str; 4] = ["The Wolf Within 0", "The Wolf Within 1", "The Wolf Within 2", "The Wolf Within 3"];

const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

/// A1: the skill's cooldown 18% shorter.
fn a1_cd(s: &Sim, ci: usize, cd: i32) -> i32 {
    if s.chars[ci].ascension < 1 {
        return cd;
    }
    (cd as f64 * 0.82) as i32
}

/// gcsim's c6HexereiMod: CRIT Rate and CRIT DMG for 15 s, for a Hexerei
/// Razor at any constellation.
fn hexerei_crit(s: &mut Sim, ci: usize) {
    if !s.chars[ci].hexerei {
        return;
    }
    s.add_mod(ci, Mod::owned(HEX_C6, ModKind::Stat, Owner::Char(ci as u8), st::NONE, false, false, -1), 15 * 60);
}

/// gcsim's addSigil, once for the attack that hit.
fn add_sigil(s: &mut Sim, ci: usize) {
    let n = if s.chars[ci].hexerei && s.chars[ci].cons >= 6 { 3 } else { 1 };
    if !s.mod_active(ci, SIGIL) {
        s.st_mut::<State>(ci).sigils = 0;
    }
    let mut sigils = s.st::<State>(ci).sigils + n;
    if sigils > 3 {
        sigils = 3;
        s.st_mut::<State>(ci).sigils = sigils;
        thunderfall(s, ci);
    }
    s.st_mut::<State>(ci).sigils = sigils;
    s.add_mod(ci, Mod::stat(SIGIL, st::ER, sigils as f64 * 0.2, false, false, -1), 18 * 60);
}

/// gcsim's thunderFallCB: Surge of Lightning, with a second Hexerei
/// character, on a sigil beyond three.
fn thunderfall(s: &mut Sim, ci: usize) {
    if !s.chars[ci].hexerei || s.hexerei_count() < 2 || s.mod_active(ci, HEX_ICD) {
        return;
    }
    hexerei_crit(s, ci);
    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, 1.5);
    ai.abil = "Surge of Lightning";
    ai.pattern = s.circle_on_player(0.0, 2.0, 5.0);
    s.add_status(ci, HEX_ICD, 60 + 74, false);
    s.queue_attack(ai, 0, 0, 37);
    s.add_task(Task::Char { c: ci as u8, id: TASK_SURGE_ENERGY, arg: 0 }, 74);
}

/// gcsim's clearSigil: the sigils turn into energy.
fn clear_sigil(s: &mut Sim, ci: usize) {
    if !s.mod_active(ci, SIGIL) {
        s.st_mut::<State>(ci).sigils = 0;
        return;
    }
    let sigils = s.st::<State>(ci).sigils;
    if sigils > 0 {
        s.add_energy(ci, sigils as f64 * 5.0);
        s.st_mut::<State>(ci).sigils = 0;
        hexerei_crit(s, ci);
        s.delete_mod(ci, SIGIL);
    }
}

fn enemy_low(s: &Sim, ci: usize) -> bool {
    s.enemy_hp / s.st::<State>(ci).enemy_max_hp < 0.3
}

impl Character for Char {
    const KEY: &'static str = "razor";
    const HEXEREI: bool = true;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Swap, Owner::Char(ci as u8));
        if s.chars[ci].ascension >= 4 {
            s.add_mod(ci, Mod::owned(A4, ModKind::Stat, Owner::Char(ci as u8), st::ER, false, false, -1), -1);
        }
        // gcsim's "utility-dash" stamina mod, one key for everyone
        s.utility_dash = Some(ci as u8);
        let cons = s.chars[ci].cons;
        if cons >= 1 {
            s.subscribe(EventKind::ParticleReceived, Owner::Char(ci as u8));
        }
        if cons >= 2 && s.damage_mode {
            s.st_mut::<State>(ci).enemy_max_hp = s.enemy_hp;
            s.add_mod(ci, Mod::owned(C2, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1), -1);
            s.subscribe(EventKind::SpecialReactionAttack, Owner::Char(ci as u8));
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Skill => p.0.iter().find(|(k, _)| k != "hold").map(|(k, _)| format!("{name} param {k}")),
            Action::Attack | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.blunt = true;
        ai.poise = g::ATTACK_POISE_DMG[n];
        ai.hitlag_factor = g::ATTACK_HITLAG_FACTOR[n];
        // gcsim passes the table as frames, not seconds (0.1 frames, then
        // the defense halt's 3.6)
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n];
        ai.def_halt = true;
        let (off, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
        ai.pattern = if n == 1 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
        let mut cbs = 0;
        if s.mod_active(ci, BURST) {
            cbs |= CB_WOLF << n;
        }
        if s.chars[ci].cons >= 6 {
            cbs |= CB_C6;
        }
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, cbs, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let burst = s.mod_active(ci, BURST);
        let b = burst as usize;
        let lvl = s.talent(ci, 1);
        if p.get("hold").unwrap_or(0) > 0 {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_HOLD[lvl]);
            ai.abil = "Claw and Thunder (Hold)";
            ai.blunt = true;
            ai.poise = 300.0;
            ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
            let cbs = if burst { 0 } else { CB_HOLD_PARTICLE };
            let h = g::SKILL_HOLD_HITMARKS[b];
            s.queue_attack(ai, cbs, h, h);
            s.add_task(Task::Char { c: ci as u8, id: TASK_CLEAR_SIGIL, arg: 0 }, h);
            let cd = a1_cd(s, ci, 10 * 60);
            s.set_cd_with_delay(ci, Action::Skill, cd, g::SKILL_HOLD_CD_STARTS[b]);
            let t = &g::SKILL_HOLD_FRAMES[b];
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_PRESS[lvl]);
        ai.abil = "Claw and Thunder (Press)";
        ai.hitlag_halt = 0.1 * 60.0;
        ai.hitlag_factor = 0.03;
        ai.def_halt = true;
        let radius = if burst { 3.0 } else { 2.4 };
        ai.pattern = s.fan_on_player(0.0, 1.0, radius, 240.0);
        let mut cbs = CB_SIGIL;
        if !burst {
            cbs |= CB_PRESS_PARTICLE;
        }
        if s.chars[ci].cons >= 4 {
            cbs |= CB_C4;
        }
        let h = g::SKILL_PRESS_HITMARKS[b];
        s.queue_attack(ai, cbs, h, h);
        let cd = a1_cd(s, ci, 6 * 60);
        s.set_cd_with_delay(ci, Action::Skill, cd, g::SKILL_PRESS_CD_STARTS[b]);
        let t = &g::SKILL_PRESS_FRAMES[b];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_BUFF, arg: 0 }, g::BURST_HITMARK);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 50.0, g::BURST_DMG[lvl]);
        ai.abil = "Lightning Fang";
        ai.blunt = true;
        ai.poise = 51.75;
        ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.set_cd(ci, Action::Burst, 1200);
        s.consume_energy(ci, 6);
        s.add_task(Task::Char { c: ci as u8, id: TASK_CLEAR_SIGIL, arg: 0 }, 7);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::BURST_HITMARK, AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_BURST_BUFF => {
                // A1: the burst resets the skill
                if s.chars[ci].ascension >= 1 {
                    s.reset_cd(ci, Action::Skill);
                }
                let v = g::BURST_ATK_SPEED[s.talent(ci, 2)];
                s.add_stat_mod(ci, BURST, st::ATKSPD, v, 15 * 60, true);
            }
            TASK_CLEAR_SIGIL => clear_sigil(s, ci),
            TASK_SURGE_ENERGY => s.add_energy(ci, 7.0),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PRESS_PARTICLE != 0 && !s.mod_active(ci, PRESS_PARTICLE_ICD) {
            s.add_status(ci, PRESS_PARTICLE_ICD, 60, true);
            s.queue_particle(3.0, Element::Electro, s.particle_delay[ci]);
        }
        if cbs & CB_HOLD_PARTICLE != 0 && !s.mod_active(ci, HOLD_PARTICLE_ICD) {
            s.add_status(ci, HOLD_PARTICLE_ICD, 60, true);
            s.queue_particle(4.0, Element::Electro, s.particle_delay[ci]);
        }
        if cbs & CB_C4 != 0 {
            s.enemy_add_def(C4, -0.15, 7 * 60);
        }
        if cbs & CB_SIGIL != 0 {
            add_sigil(s, ci);
        }
        if cbs & (0xf * CB_WOLF) != 0 {
            let n = ((cbs / CB_WOLF) & 0xf).trailing_zeros() as usize;
            let lvl = s.talent(ci, 2);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::WOLF_DMG[lvl] * ev.info.mult);
            ai.abil = WOLF_NAMES[n];
            if s.chars[ci].hexerei {
                ai.flat_dmg += 0.7 * s.total_atk(ci);
            }
            let (off, hb) = (g::BURST_ATTACK_OFFSETS[n], g::BURST_ATTACK_HITBOXES[n]);
            ai.pattern = if n == 1 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
            s.queue_attack(ai, 0, 1, 1);
        }
        if cbs & CB_C6 != 0 && !s.mod_active(ci, C6_ICD) {
            s.add_status(ci, C6_ICD, 600, true);
            let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, 1.0);
            ai.abil = "Lupus Fulguris";
            ai.pattern = s.circle_toward_target(0.0, 0.7, 1.5);
            s.queue_attack(ai, CB_C6_SIGIL, 1, 1);
        }
        if cbs & CB_C6_SIGIL != 0 && !s.mod_active(ci, BURST) {
            add_sigil(s, ci);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == A4 {
            let c = &s.chars[ci];
            if c.energy / c.energy_max >= 0.5 {
                return false;
            }
            out[st::ER] += 0.3;
        } else {
            // the Hexerei CRIT buff
            out[st::CR] += 0.1;
            out[st::CD] += 0.5;
        }
        true
    }

    /// C2: CRIT Rate on an enemy below 30% HP.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if enemy_low(s, ci) {
            ev.stats[st::CR] += 0.1;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { prev, .. } => {
                if prev == ci && s.mod_active(ci, BURST) {
                    s.delete_mod(ci, BURST);
                }
            }
            // C1: DMG while on field, on a particle
            Event::ParticleReceived => {
                if s.player.active as usize == ci {
                    s.add_mod(ci, Mod::stat(C1, st::DMGP, 0.1, false, true, -1), 8 * 60);
                }
            }
            // C2 on lunar reactions
            Event::SpecialReactionAttack(_) => {
                if enemy_low(s, ci) {
                    s.special_atk.stats[st::CR] += 0.1;
                }
            }
            _ => {}
        }
    }
}
