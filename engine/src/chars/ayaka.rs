//! Kamisato Ayaka, from gcsim's internal/characters/ayaka: normal attacks,
//! the charged attack (its hits on the enemy in reach), Kamisato Art:
//! Hyouka, Kamisato Art: Soumetsu (and C2's two smaller ones),
//! Senho (her dash: its cryo hit, the infusion, no dash cooldown), the
//! airborne plunges, A1, A4 and every constellation.
//!
//! The charged attack aims its hits at the enemy in reach and at up to four
//! gadgets of the enemy camp (dendro cores, the Bogglecat Box) within 4 of
//! it, three hits on each, each a circle of radius 1 that also reaches
//! the enemy from a gadget close to it. A dash of
//! 60 frames or more costs gcsim more stamina by its `f` (ActionStam),
//! which the engine does not read: an error too.

use crate::chars::plunge;
use crate::content::Character;
use crate::gcsimdata::chars::ayaka as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// C6: whether the next charged hit starts the buff's cooldown.
    c6_avail: bool,
    /// The normal attacks' hit areas, made when the attack starts, by
    /// attack and hit.
    normal_patterns: [[Pattern; 3]; 5],
    /// The charged attack's targets: the enemy, then gadgets.
    charge_targets: [(f64, f64); 5],
}

const A1: ModKey = ModKey::new("ayaka-a1");
const A4: ModKey = ModKey::new("ayaka-a4");
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C4: ModKey = ModKey::new("ayaka-c4");
const C6: ModKey = ModKey::new("ayaka-c6");
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const DASH_INFUSE: ModKey = ModKey::new("ayaka-dash");

const CB_C1: u16 = 1 << 0;
const CB_C4: u16 = 1 << 1;
const CB_C6: u16 = 1 << 2;
const CB_PARTICLE: u16 = 1 << 3;
const CB_A4: u16 = 1 << 4;

/// A normal attack's hit: arg is attack * 3 + hit (her queue).
const TASK_NORMAL: u8 = 0;
/// The charged attack's search for targets (the fight's queue).
const TASK_CHARGE_CHECK: u8 = 1;
/// A charged hit on a target: arg is its index (the fight's queue).
const TASK_CHARGE_HIT: u8 = 2;
/// The dash's infusion (the fight's queue).
const TASK_INFUSE: u8 = 3;
/// C6: the buff goes 30 frames after a charged hit (her queue).
const TASK_C6_DELETE: u8 = 4;
/// C6: the buff comes back 600 frames later (her queue).
const TASK_C6_RESTORE: u8 = 5;

const FR_DASH: u8 = 0;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];

/// The airborne plunges (Xianyun's), from gcsim's plunge.go.
const PLUNGE: plunge::Spec = plunge::Spec {
    high_queue: Action::Skill,
    ..plunge::Spec::physical(
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
    )
};

impl Character for Char {
    const KEY: &'static str = "kamisatoayaka";
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        State { c6_avail: cs.cons >= 6, normal_patterns: [[Pattern::Single; 3]; 5], charge_targets: [(0.0, 0.0); 5] }
    }

    fn init(s: &mut Sim, ci: usize) {
        // C6: the buff from the start
        if s.chars[ci].cons >= 6 {
            c6_add_buff(s, ci);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// Plunges only while airborne (Xianyun's jump).
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    /// Her dash costs 10 stamina (15 more a second of `f`, not ported).
    fn action_stam(_: &Sim, _: usize, a: Action) -> Option<f64> {
        (a == Action::Dash).then_some(10.0)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let hits = g::ATTACK_HITMARKS[n];
        // gcsim makes each hit's area when the attack starts: on the
        // player, or on the enemy for the fifth (a bullet)
        let oy = g::ATTACK_OFFSETS[n];
        let r = g::ATTACK_RADIUS[n];
        let fan = g::ATTACK_FAN_ANGLES[n];
        let p = if n == 4 { fan_toward_target(s, oy, r, fan) } else { s.fan_on_player(0.0, oy, r, fan) };
        for (i, &h) in hits.iter().enumerate() {
            s.st_mut::<State>(ci).normal_patterns[n][i] = p;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: (n * 3 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hits[hits.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    /// The charged attack: 26 frames in, the hits are aimed at the enemy
    /// when it is within 5 of the player.
    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let check = g::CHARGE_HITMARKS[0] - 1;
        s.add_task(Task::Char { c: ci as u8, id: TASK_CHARGE_CHECK, arg: 0 }, check);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARKS[g::CHARGE_HITMARKS.len() - 1], AnimState::Charge)
    }

    /// Kamisato Art: Hyouka: A1's bonus, and the hit 33 frames in.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.chars[ci].ascension >= 1 {
            s.add_mod(ci, Mod::owned(A1, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 360);
        }
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 50.0, g::SKILL[lvl]);
        ai.abil = "Hyouka";
        ai.pattern = s.circle_on_player(0.0, 0.0, 4.5);
        s.queue_attack(ai, CB_PARTICLE, 0, g::SKILL_HITMARK);
        s.set_cd(ci, Action::Skill, 600);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    /// Kamisato Art: Soumetsu: 19 cuts every 15 frames and the bloom 5 s
    /// after 104 frames in (and C2's two smaller storms), snapshot then.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let c2 = s.chars[ci].cons >= 2;
        let h = g::BURST_HITMARK;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST_BLOOM[lvl]);
        ai.abil = "Soumetsu (Bloom)";
        ai.pattern = s.circle_toward_target(0.0, 0.0, 5.0);
        s.queue_attack(ai, CB_C4, h, h + 300);
        let mut c2 = c2.then(|| {
            let mut a = ai;
            a.mult = g::BURST_BLOOM[lvl] * 0.2;
            a.abil = "Mini-Frostflake Seki no To (Bloom) (C2)";
            a
        });
        if let Some(mut a) = c2 {
            a.pattern = s.circle_toward_target(0.0, 0.0, 3.0);
            for _ in 0..2 {
                s.queue_attack(a, CB_C4, h, h + 300);
            }
        }
        for i in 0..19 {
            ai.mult = g::BURST_CUT[lvl];
            ai.abil = "Soumetsu (Cutting)";
            ai.pattern = s.circle_toward_target(0.0, 0.3, 3.0);
            s.queue_attack(ai, CB_C4, h, h + i * 15);
            if let Some(a) = c2.as_mut() {
                a.mult = g::BURST_CUT[lvl] * 0.2;
                a.abil = "Mini-Frostflake Seki no To (Cutting) (C2)";
                a.pattern = s.circle_toward_target(0.0, 0.3, 1.5);
                for _ in 0..2 {
                    s.queue_attack(*a, CB_C4, h, h + i * 15);
                }
            }
        }
        s.consume_energy(ci, 8);
        s.set_cd(ci, Action::Burst, 20 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Burst)
    }

    /// Senho: no dash cooldown; a cryo hit and the infusion at its end
    /// (A4 on the hit), and the stamina at the template's time.
    fn own_dash(s: &mut Sim, ci: usize, p: &Params) -> Option<ActInfo> {
        let f = p.get("f").unwrap_or(0) as i32;
        if f >= 60 {
            s.unsupported("kamisatoayaka dash[f] of 60 frames or more (its stamina)");
        }
        let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_DASH, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 0.0);
        ai.abil = "Dash";
        ai.pattern = s.circle_on_player(0.0, 0.1, 2.0);
        let h = g::DASH_HITMARK + f;
        let cbs = if s.chars[ci].ascension >= 4 { CB_A4 } else { 0 };
        s.queue_attack(ai, cbs, h, h);
        s.add_task(Task::Char { c: ci as u8, id: TASK_INFUSE, arg: 0 }, h);
        // QueueDashStaminaConsumption
        let len = s.chars[ci].dash_len;
        s.add_task(Task::DashStam { c: ci as u8 }, len - 1);
        let t = &g::DASH_FRAMES;
        Some(ActInfo::new(Frames::Char { id: FR_DASH, arg: f as i16 }, t[0] + f, t[Action::Dash as usize] + f, AnimState::Dash))
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_DASH => g::DASH_FRAMES[next as usize] + arg as i32,
            _ => unreachable!(),
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = (arg as usize / 3, arg as usize % 3);
                let lvl = s.talent(ci, 0);
                // the fifth hit is in another ICD group
                let group = if n == 4 { icd::GROUP_POLE_EXTRA_ATTACK } else { icd::GROUP_DEFAULT };
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, group, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.hitlag_factor = g::ATTACK_HITLAG_FACTOR[n][i];
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.def_halt = g::ATTACK_DEF_HALT[n][i];
                ai.pattern = s.st::<State>(ci).normal_patterns[n][i];
                s.queue_attack(ai, CB_C1, 0, 0);
            }
            TASK_CHARGE_CHECK => {
                if !s.enemy_alive || !s.enemy_within(s.circle_on_player(0.0, 0.0, 5.0)) {
                    return;
                }
                // the enemy, then up to four gadgets of the enemy camp within
                // 4 of it, in the order they came (GadgetsWithinArea)
                let area = s.circle_on_target(4.0);
                let mut targets = [(0.0, 0.0); 5];
                targets[0] = s.enemy_pos;
                let mut n = 1;
                for gd in &s.gadgets {
                    if n == 5 {
                        break;
                    }
                    if gd.enemy_camp && area.contains_point(gd.x, gd.y) == Some(true) {
                        targets[n] = (gd.x, gd.y);
                        n += 1;
                    }
                }
                s.st_mut::<State>(ci).charge_targets = targets;
                let check = g::CHARGE_HITMARKS[0] - 1;
                for t in 0..n {
                    for &h in g::CHARGE_HITMARKS {
                        s.add_task(Task::Char { c: ci as u8, id: TASK_CHARGE_HIT, arg: t as i32 }, h - check);
                    }
                }
            }
            TASK_CHARGE_HIT => {
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_AYAKA_EXTRA_ATTACK, Element::Physical, 25.0, g::CA[lvl]);
                ai.abil = "Charge";
                let (x, y) = s.st::<State>(ci).charge_targets[arg as usize];
                ai.pattern = Pattern::Circle { x, y, r: 1.0 };
                s.queue_attack(ai, CB_C1 | CB_C6, 0, 0);
            }
            TASK_INFUSE => {
                s.add_infusion(ci, DASH_INFUSE, Element::Cryo, 300, true, &[AttackTag::Normal, AttackTag::Extra, AttackTag::Plunge]);
            }
            TASK_C6_DELETE => {
                s.delete_mod(ci, C6);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6_RESTORE, arg: 0 }, 600);
            }
            TASK_C6_RESTORE => {
                s.st_mut::<State>(ci).c6_avail = true;
                c6_add_buff(s, ci);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        // C1: a cryo normal or charged hit cuts the skill's cooldown by
        // 0.3 s half the time, once in 0.1 s
        if cbs & CB_C1 != 0 && s.chars[ci].cons >= 1 && ev.info.element == Element::Cryo && !s.mod_active(ci, C1_ICD) && s.rng.float64() >= 0.5 {
            s.reduce_cd(ci, Action::Skill, 18);
            s.add_status(ci, C1_ICD, 6, true);
        }
        // C4: the burst's damage cuts DEF by 30% for 6 s
        if cbs & CB_C4 != 0 && s.chars[ci].cons >= 4 && ev.damage != 0.0 {
            s.enemy_add_def(C4, -0.3, 360);
        }
        // C6: a charged hit ends the buff 0.5 s later, back 10 s after
        if cbs & CB_C6 != 0 && s.chars[ci].cons >= 6 && s.st::<State>(ci).c6_avail {
            s.st_mut::<State>(ci).c6_avail = false;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6_DELETE, arg: 0 }, 30);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, false);
            let count = if s.rng.float64() < 0.5 { 5.0 } else { 4.0 };
            s.queue_particle(count, Element::Cryo, s.particle_delay[ci]);
        }
        // A4: 10 stamina and 18% Cryo DMG Bonus for 10 s
        if cbs & CB_A4 != 0 {
            s.restore_stam(10.0);
            s.add_stat_mod(ci, A4, st::CRYOP, 0.18, 600, true);
        }
    }

    /// A1: +30% normal and charged attack DMG; C6: +298% charged attack
    /// DMG.
    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A1 && matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            ev.stats[st::DMGP] += 0.3;
        }
        if m.key == C6 && ev.info.tag == AttackTag::Extra {
            ev.stats[st::DMGP] += 2.98;
        }
    }
}

fn c6_add_buff(s: &mut Sim, ci: usize) {
    s.add_mod(ci, Mod::owned(C6, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1), -1);
}

/// gcsim's NewCircleHitFanAngle from the player centered on the enemy:
/// the fan faces the way from the player to it.
fn fan_toward_target(s: &Sim, oy: f64, r: f64, fan: f64) -> Pattern {
    debug_assert!(oy == 0.0 && fan == 360.0);
    s.circle_toward_target(0.0, oy, r)
}
