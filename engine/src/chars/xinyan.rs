//! Xinyan, from gcsim's internal/characters/xinyan: normal attacks, the
//! plunges from Xianyun's airborne state, the skill's swing and its shield
//! of three levels (by the enemies hit; level 3 pulses Pyro DoT), the
//! burst's Physical hit and Pyro DoTs, A1 (fewer hits per level), A4
//! (Physical DMG for the shielded active character), C1, C2 and C4, and her
//! start delay for Xingqiu's N0. gcsim has no charged attack for her (and
//! so no C6, which only changes it): `charge` is an error.
//!
//! The skill's level counts every opponent its callbacks reach; gcsim calls
//! callbacks for enemies only, so against the one enemy a fight has, the
//! swing reaches level 2 with A1 and level 3 only through C2's burst.

use crate::chars::plunge;
use crate::content::Character;
use crate::gamedata::chars::xinyan as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    shield_level: i32,
    level2_req: i32,
    level3_req: i32,
    shield_tick_src: i32,
    /// The current cast's opponents hit and its snapshot's DEF (gcsim's
    /// closure over hitOpponents and defFactor).
    hits: i32,
    skill_def: f64,
    /// C2: the burst's DEF for its level 3 shield.
    burst_def: f64,
}

const SHIELD: ModKey = ModKey::new("xinyan-skill-shield");
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C1: ModKey = ModKey::new("xinyan-c1");
const C2: ModKey = ModKey::new("xinyan-c2");
const C4: ModKey = ModKey::new("xinyan-c4");
const A4: ModKey = ModKey::new("xinyan-a4");

// the skill's callbacks, in gcsim's order
const CB_SKILL: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;
const CB_C1: u16 = 1 << 2;
const CB_C4: u16 = 1 << 3;

// arg: the level
const TASK_SHIELD: u8 = 0;
// the skill's level check after a hit
const TASK_SKILL_CHECK: u8 = 1;
// arg: the shield's tick source
const TASK_SHIELD_DOT: u8 = 2;
const TASK_BURST_DOT1: u8 = 3;
const TASK_BURST_DOTS: u8 = 4;
// C2's level 3 shield
const TASK_BURST_SHIELD: u8 = 5;

const PLUNGE: plunge::Spec = plunge::Spec::physical(
    g::LOW_PLUNGE_HITMARK,
    g::HIGH_PLUNGE_HITMARK,
    g::COLLISION_HITMARK,
    &g::LOW_PLUNGE_FRAMES,
    &g::HIGH_PLUNGE_FRAMES,
    g::LOW_PLUNGE_POISE_DMG,
    g::HIGH_PLUNGE_POISE_DMG,
    g::LOW_PLUNGE_RADIUS,
    g::HIGH_PLUNGE_RADIUS,
    g::LOW_PLUNGE,
    g::HIGH_PLUNGE,
    g::COLLISION,
);

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn has_shield(s: &Sim) -> bool {
    s.shields.iter().any(|x| x.typ == ShieldType::XinyanSkill)
}

fn c1_cb(s: &Sim, ci: usize) -> u16 {
    if s.chars[ci].cons >= 1 { CB_C1 } else { 0 }
}

impl Character for Char {
    const KEY: &'static str = "xinyan";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { shield_level: 1, level2_req: 2, level3_req: 3, shield_tick_src: 0, hits: 0, skill_def: 0.0, burst_def: 0.0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // A1: one opponent fewer for each level
        if s.chars[ci].ascension >= 1 {
            let stt = st_mut(s, ci);
            stt.level2_req -= 1;
            stt.level3_req -= 1;
        }
        // A4: Physical DMG for every character while shielded and active
        if s.chars[ci].ascension >= 4 {
            for i in 0..s.chars.len() {
                s.add_mod(i, Mod::owned(A4, ModKind::Attack, o, st::NONE, false, false, -1), -1);
            }
        }
        if s.chars[ci].cons >= 2 {
            s.add_mod(ci, Mod::owned(C2, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    /// AnimationStartDelay: Xingqiu's N0.
    fn start_delay(_: &Sim, _: usize, key: usize) -> Option<i32> {
        (key == 2).then_some(28)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.blunt = true;
        ai.poise = g::ATTACK_POISE_DMG[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        let hb = g::ATTACK_HITBOXES[n];
        let off = g::ATTACK_OFFSETS[n];
        ai.pattern = if n == 1 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.fan_on_player(0.0, off, hb[0], g::ATTACK_FAN_ANGLES[n]) };
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, c1_cb(s, ci), h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL[lvl]);
        ai.abil = "Sweeping Fervor (Initial)";
        ai.blunt = true;
        ai.poise = 200.0;
        ai.hitlag_halt = 0.09 * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        // the snapshot's DEF for the shield
        let (snap, _) = s.make_snapshot(&mut ai);
        let stt = st_mut(s, ci);
        stt.skill_def = total_def(&snap);
        stt.hits = 0;
        if !has_shield(s) {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SHIELD, arg: 1 }, g::SKILL_SHIELD_START);
        }
        ai.pattern = s.circle_on_player(0.0, 1.0, 3.0);
        let mut cbs = CB_SKILL | CB_PARTICLE | c1_cb(s, ci);
        if s.chars[ci].cons >= 4 {
            cbs |= CB_C4;
        }
        s.queue_attack(ai, cbs, g::SKILL_HITMARK, g::SKILL_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 18 * 60, 13);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 100.0, g::BURST_DMG[lvl]);
        ai.abil = "Riff Revolution (Initial)";
        ai.def_halt = true;
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
        s.queue_attack(ai, c1_cb(s, ci), g::BURST_INITIAL_HITMARK, g::BURST_INITIAL_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_DOT1, arg: 0 }, g::BURST_DO_T1_HITMARK);
        if s.chars[ci].cons >= 2 {
            st_mut(s, ci).burst_def = total_def(&s.select_stats(ci, [st::BASEDEF, st::DEFP, st::DEF], false));
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_SHIELD, arg: 0 }, g::BURST_SHIELD_START);
            // C2's extra pulses of the level 3 shield's DoT
            for &h in g::C2_PULSE_HITMARKS {
                let ai = shield_dot_info(s, ci);
                s.queue_attack(ai, c1_cb(s, ci), 1, h);
            }
        }
        s.consume_energy(ci, 5);
        s.set_cd_with_delay(ci, Action::Burst, 15 * 60, 1);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Burst)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SHIELD => {
                let def = st(s, ci).skill_def;
                update_shield(s, ci, arg, def);
            }
            TASK_SKILL_CHECK => {
                let stt = st(s, ci);
                let def = stt.skill_def;
                if stt.hits >= stt.level3_req && stt.shield_level < 3 {
                    update_shield(s, ci, 3, def);
                } else if stt.hits >= stt.level2_req && stt.shield_level < 2 {
                    update_shield(s, ci, 2, def);
                }
            }
            TASK_SHIELD_DOT => {
                if !has_shield(s) || st(s, ci).shield_level != 3 || st(s, ci).shield_tick_src != arg {
                    return;
                }
                let ai = shield_dot_info(s, ci);
                s.queue_attack(ai, c1_cb(s, ci), 1, 1);
                s.add_task(Task::Char { c: ci as u8, id: TASK_SHIELD_DOT, arg }, 2 * 60);
            }
            TASK_BURST_DOT1 | TASK_BURST_DOTS => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST_PYRO, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BURST_DOT[lvl]);
                ai.abil = "Riff Revolution (DoT)";
                ai.pattern = s.circle_on_player(0.0, 2.0, 4.0);
                if id == TASK_BURST_DOT1 {
                    // only the first DoT has hitlag
                    ai.def_halt = true;
                    s.queue_attack(ai, c1_cb(s, ci), 0, 0);
                    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_DOTS, arg: 0 }, 17);
                } else {
                    for i in 0..6 {
                        s.queue_attack(ai, c1_cb(s, ci), i * 17, i * 17);
                    }
                }
            }
            TASK_BURST_SHIELD => {
                let def = st(s, ci).burst_def;
                update_shield(s, ci, 3, def);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        // the skill's opponents hit, checked for a level 13 frames on
        if cbs & CB_SKILL != 0 {
            st_mut(s, ci).hits += 1;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_CHECK, arg: 0 }, g::SKILL_SHIELD_START - g::SKILL_HITMARK);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 12, true);
            s.queue_particle(4.0, Element::Pyro, s.particle_delay[ci]);
        }
        // C1: a crit while she is active, every 5 s: ATK SPD for her normal
        // and charged attacks
        if cbs & CB_C1 != 0 && s.player.active as usize == ci && ev.crit && !s.mod_active(ci, C1_ICD) {
            s.add_status(ci, C1_ICD, 300, true);
            let o = Owner::Char(ci as u8);
            s.add_mod(ci, Mod::owned(C1, ModKind::Stat, o, st::ATKSPD, false, true, -1), 300);
        }
        // C4: the swing shreds Physical RES
        if cbs & CB_C4 != 0 {
            s.enemy_add_resist(C4, Element::Physical, -0.15, 12 * 60);
        }
    }

    /// C1's ATK SPD, only in a normal or charged attack.
    fn mod_amount(s: &Sim, _: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C1 {
            if matches!(s.current_state(), AnimState::Normal | AnimState::Charge) {
                out[st::ATKSPD] += 0.12;
            }
            return true;
        }
        false
    }

    /// A4 (for the shielded active character) and C2 (burst's Physical
    /// DMG crits).
    fn attack_mod(s: &Sim, _: usize, on: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A4 {
            if has_shield(s) && s.player.active as usize == on {
                ev.stats[st::PHYP] += 0.15;
            }
        } else if m.key == C2 && ev.info.tag == AttackTag::ElementalBurst && ev.info.element == Element::Physical {
            ev.stats[st::CR] += 1.0;
        }
    }

    /// A broken shield drops to level 1.
    fn shield_damaged(s: &mut Sim, ci: usize, key: ModKey, _: f64, broken: bool) {
        if key == SHIELD && broken {
            st_mut(s, ci).shield_level = 1;
        }
    }

    fn shield_expired(s: &mut Sim, ci: usize, key: ModKey) {
        if key == SHIELD {
            st_mut(s, ci).shield_level = 1;
        }
    }
}

fn shield_dot_info(s: &Sim, ci: usize) -> AttackInfo {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_DOT[lvl]);
    ai.abil = "Sweeping Fervor (DoT)";
    ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
    ai
}

/// gcsim's updateShield: the shield of a level, 12 s; level 3 starts its
/// DoT.
fn update_shield(s: &mut Sim, ci: usize, level: i32, def: f64) {
    let lvl = s.talent(ci, 1);
    st_mut(s, ci).shield_level = level;
    let f = s.f;
    let hp = match level {
        1 => g::SHIELD_LEVEL1_FLAT[lvl] + g::SHIELD_LEVEL1[lvl] * def,
        2 => g::SHIELD_LEVEL2_FLAT[lvl] + g::SHIELD_LEVEL2[lvl] * def,
        _ => {
            st_mut(s, ci).shield_tick_src = f;
            s.add_task(Task::Char { c: ci as u8, id: TASK_SHIELD_DOT, arg: f }, 2 * 60);
            g::SHIELD_LEVEL3_FLAT[lvl] + g::SHIELD_LEVEL3[lvl] * def
        }
    };
    s.add_shield(Shield { owner: ci as u8, key: SHIELD, typ: ShieldType::XinyanSkill, target: -1, ele: Element::Pyro, hp, expiry: f + 12 * 60, em_bonus: 0.0 });
}
