//! Arlecchino, from gcsim's internal/characters/arlecchino: normal attacks
//! (pyro and with flat damage from her Bond of Life while Masque of the Red
//! Death is up, at 30% of max HP), the charged attack and its early cancel,
//! All is Ash (the spike, the cleave and the Blood Debt Directive it leaves
//! on the enemy, which ticks and which A1 upgrades), Balemoon Rising (the
//! hit, the directives it absorbs into her Bond of Life, the skill's
//! cooldown reset and her heal), A1's directive on a kill, her passive's
//! pyro DMG, the airborne plunges and every constellation.
//!
//! As gcsim writes her: she ignores every heal but her burst's own (her
//! ReceiveHeal: other heals neither heal her nor pay her Bond of Life),
//! A4's RES is not implemented, and A1's kill hook is subscribed whatever
//! her ascension.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gamedata::chars::arlecchino as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    skill_debt: f64,
    skill_debt_max: f64,
    initial_directive_level: i32,
    /// The enemy's directive tags (gcsim's tags on it): its level and the
    /// frame it was given, 0 once removed.
    dir_level: i32,
    dir_src: i32,
    /// The last charged attack was cancelled early (gcsim reads the
    /// last action's early_cancel param).
    early: bool,
    /// Her burst's heal is being given (the one heal she receives).
    own_heal: bool,
    /// The burst's hit, its area taken as it is cast.
    burst: AttackInfo,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const DIRECTIVE: ModKey = ModKey::new(g::DIRECTIVE_KEY);
const DIRECTIVE_LIMIT: ModKey = ModKey::new(g::DIRECTIVE_LIMIT_KEY);
const NA_BUFF: ModKey = ModKey::new(g::NA_BUFF_KEY);
const BOND_CONSUME_ICD: ModKey = ModKey::new(g::BOND_CONSUME_ICD_KEY);
const PASSIVE: ModKey = ModKey::new("arlecchino-passive");
const C2_ICD: ModKey = ModKey::new(g::C2_ICD_KEY);
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_DIRECTIVE: u16 = 1 << 1;
const CB_BOND: u16 = 1 << 2;

/// A normal attack's hit at its hitmark, with counter * 4 + hit.
const TASK_NA: u8 = 0;
/// absorbDirectives (the charged attack and the burst).
const TASK_ABSORB: u8 = 1;
/// The charged attack's hit.
const TASK_CHARGE: u8 = 2;
/// debtLimit, after the cleave.
const TASK_DEBT_LIMIT: u8 = 3;
/// C6 as the cleave hits.
const TASK_C6_SKILL: u8 = 4;
/// The directive's tick (the enemy's queue), with src * 4 + count.
const TASK_DIRECTIVE_TICK: u8 = 5;
/// A1's upgrade of the directive (the enemy's queue), with its src.
const TASK_A1_UPGRADE: u8 = 6;
/// The burst resets the skill's cooldown.
const TASK_BURST_RESET: u8 = 7;
/// The burst's hit, with C6's flat damage taken then.
const TASK_BURST_HIT: u8 = 8;
/// balemoonRisingHeal.
const TASK_BURST_HEAL: u8 = 9;

/// The charged attack's frames, less its windup (arg).
const FRAMES_CHARGE: u8 = 0;

const NORMAL_NAMES: [&str; 6] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4", "Normal 5"];

/// gcsim's plunge.go: the physical plunges on Xianyun's airborne state,
/// the low one queueing after its burst frame, the high one after its
/// attack frame.
const PLUNGE: plunge::Spec = plunge::Spec {
    low_queue: Action::Burst,
    high_queue: Action::Attack,
    ..plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION)
};

impl Character for Char {
    const KEY: &'static str = "arlecchino";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            skill_debt: 0.0,
            skill_debt_max: 0.0,
            initial_directive_level: 1,
            dir_level: 0,
            dir_src: 0,
            early: false,
            own_heal: false,
            burst: AttackInfo::new(0, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, 0.0),
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // naBuff
        s.subscribe(EventKind::HpDebt, o);
        // passive: pyro DMG +40%
        s.chars[ci].mods.push(Mod::stat(PASSIVE, st::PYROP, 0.4, false, false, -1));
        // a1OnKill, whatever her ascension
        s.subscribe(EventKind::TargetDied, o);
        // c2
        let level = if s.chars[ci].cons >= 2 && s.chars[ci].ascension >= 1 { 2 } else { 1 };
        s.st_mut::<State>(ci).initial_directive_level = level;
    }

    /// gcsim validates early_cancel on the charged attack and collision on
    /// the plunges, and reads no other params.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            Action::Charge => p.0.iter().find(|(k, _)| k != "early_cancel").map(|(k, _)| format!("{name} param {k}")),
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            _ => Some(name.into()),
        }
    }

    /// gcsim's NextQueueItemIsValid: after an early-cancelled charged
    /// attack only a dash or jump; a charged attack needs no normal attack
    /// before it; otherwise the template's.
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if s.player.last.action == Action::Charge && s.player.last.char as usize == ci && s.st::<State>(ci).early && !matches!(a, Action::Dash | Action::Jump) {
            return Some(format!("arlecchino: Cannot early cancel Charged Attack with {}", crate::gcsl::action_name(a)));
        }
        if let Some(why) = plunge::grounded(s, a) {
            return Some(why);
        }
        if a == Action::Charge {
            return None;
        }
        crate::content::template_invalid(s, a)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let marks = g::ATTACK_HITMARKS[n];
        for (i, &h) in marks.iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NA, arg: (n * 4 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *marks.last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let windup = if s.current_state() == AnimState::Normal { 12 } else { 0 };
        let early = p.get("early_cancel").unwrap_or(0) > 0;
        s.st_mut::<State>(ci).early = early;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ABSORB, arg: 0 }, 12 - windup);
        if early {
            return ActInfo::new(Frames::Const(13 - windup), g::CHARGE_HITMARK - 1, 13 - windup, AnimState::Charge);
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CHARGE, arg: 0 }, g::CHARGE_HITMARK - windup);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: FRAMES_CHARGE, arg: windup as i16 }, t[0] - windup, g::CHARGE_HITMARK - windup, AnimState::Charge)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FRAMES_CHARGE => g::CHARGE_FRAMES[next as usize] - arg as i32,
            _ => unreachable!(),
        }
    }

    /// All is Ash: the spike, then the cleave that leaves the directive.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_ARLECCHINO_ELEMENTAL_ART, Element::Pyro, 25.0, g::SKILL_SPIKE[lvl]);
        ai.abil = "All is Ash (Spike)";
        // NewCircleHitOnTarget(Player, nil, 0.5)
        ai.pattern = s.circle_on_player(0.0, 0.0, 0.5);
        s.queue_attack(ai, 0, g::SPIKE_HITMARK, g::SPIKE_HITMARK);

        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL_FINAL[lvl]);
        ai.abil = "All is Ash (Cleave)";
        ai.def_halt = true;
        // NewCircleHitOnTarget(PrimaryTarget, {Y: 4.5}, 6)
        ai.pattern = s.circle_on_enemy(0.0, 4.5, 6.0);
        s.queue_attack(ai, CB_PARTICLE | CB_DIRECTIVE, g::FINAL_HITMARK, g::FINAL_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_DEBT_LIMIT, arg: 0 }, g::FINAL_HITMARK + 1);
        s.set_cd_with_delay(ci, Action::Skill, 30 * 60, 16);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6_SKILL, arg: 0 }, g::FINAL_HITMARK);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    /// Balemoon Rising.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BURST[lvl]);
        ai.abil = "Balemoon Rising";
        // NewCircleHitOnTarget(Player, {Y: 6}, 6.5), as she casts it
        ai.pattern = s.circle_on_player(0.0, 6.0, 6.5);
        s.st_mut::<State>(ci).burst = ai;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ABSORB, arg: 0 }, 22);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_RESET, arg: 0 }, 107);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_HIT, arg: 0 }, g::BURST_HITMARKS);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_HEAL, arg: 0 }, 123);
        s.set_cd_with_delay(ci, Action::Burst, 15 * 60, 0);
        s.consume_energy(ci, 12);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge_xy(s, ci, false, p)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge_xy(s, ci, true, p)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NA => na_hit(s, ci, (arg / 4) as usize, (arg % 4) as usize),
            TASK_ABSORB => absorb_directives(s, ci),
            TASK_CHARGE => {
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGE[lvl]);
                ai.abil = "Charge";
                ai.hitlag_halt = 0.02;
                ai.hitlag_factor = 0.01;
                ai.def_halt = true;
                if s.mod_active(ci, NA_BUFF) {
                    ai.element = Element::Pyro;
                    ai.ignore_infusion = true;
                }
                // NewCircleHit(Player, PrimaryTarget, {Y: 0.9}, 4)
                ai.pattern = s.circle_toward_target(0.0, 0.9, 4.0);
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_DEBT_LIMIT => {
                s.add_status(ci, DIRECTIVE_LIMIT, 35 * 60, true);
                let max = 1.45 * s.max_hp(ci);
                let st = s.st_mut::<State>(ci);
                st.skill_debt_max = max;
                st.skill_debt = 0.0;
            }
            TASK_C6_SKILL => {
                if s.chars[ci].cons < 6 || s.mod_active(ci, C6_ICD) {
                    return;
                }
                s.add_status(ci, C6_ICD, 15 * 60, true);
                s.add_mod(ci, Mod::owned(C6, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 20 * 60);
            }
            TASK_DIRECTIVE_TICK => {
                let (src, count) = (arg >> 2, arg & 3);
                if s.st::<State>(ci).dir_src != src || !s.enemy_status_active(DIRECTIVE) {
                    return;
                }
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_ARLECCHINO_ELEMENTAL_ART, Element::Pyro, 25.0, g::SKILL_SIGIL[lvl]);
                ai.abil = "Blood Debt Directive";
                ai.pattern = Pattern::Single;
                s.queue_attack(ai, 0, 0, 0);
                if count - 1 > 0 {
                    s.queue_enemy_task(Task::Char { c: ci as u8, id: TASK_DIRECTIVE_TICK, arg: src << 2 | (count - 1) }, 5 * 60);
                }
            }
            TASK_A1_UPGRADE => {
                let st = s.st_mut::<State>(ci);
                if st.dir_level == 0 || st.dir_level >= 2 || st.dir_src != arg {
                    return;
                }
                st.dir_level += 1;
            }
            TASK_BURST_RESET => s.reset_cd(ci, Action::Skill),
            TASK_BURST_HIT => {
                // the Bond of Life can change during the burst
                let mut ai = s.st::<State>(ci).burst;
                ai.flat_dmg += c6_amount(s, ci);
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_BURST_HEAL => {
                let amt = 1.5 * s.hp_debt(ci) + 1.5 * s.total_atk(ci);
                let bonus = s.stat(ci, st::HEAL);
                s.st_mut::<State>(ci).own_heal = true;
                s.heal(ci, ci, amt, bonus);
                s.st_mut::<State>(ci).own_heal = false;
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, false);
            s.queue_particle(5.0, Element::Pyro, s.particle_delay[ci]);
        }
        if cbs & CB_DIRECTIVE != 0 {
            // bloodDebtDirective
            s.enemy_add_status(DIRECTIVE, 30 * 60, true);
            let f = s.f;
            let level = s.st::<State>(ci).initial_directive_level;
            let st = s.st_mut::<State>(ci);
            st.dir_src = f;
            st.dir_level = level;
            s.queue_enemy_task(Task::Char { c: ci as u8, id: TASK_DIRECTIVE_TICK, arg: f << 2 | 2 }, 5 * 60);
            // a1Upgrade
            if s.chars[ci].ascension >= 1 {
                s.queue_enemy_task(Task::Char { c: ci as u8, id: TASK_A1_UPGRADE, arg: f }, 5 * 60);
            }
        }
        if cbs & CB_BOND != 0 {
            // bondConsumeCB
            if !s.mod_active(ci, NA_BUFF) || s.mod_active(ci, BOND_CONSUME_ICD) {
                return;
            }
            s.add_status(ci, BOND_CONSUME_ICD, 2, true);
            let amt = -0.075 * s.hp_debt(ci);
            s.modify_hp_debt(ci, amt);
            s.reduce_cd(ci, Action::Skill, 48);
        }
    }

    /// C6: CRIT Rate +10% and CRIT DMG +70% on normal attacks and the burst.
    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C6 && matches!(ev.info.tag, AttackTag::ElementalBurst | AttackTag::Normal) {
            ev.stats[st::CR] += 0.1;
            ev.stats[st::CD] += 0.7;
        }
    }

    /// Her ReceiveHeal: only her burst's heal.
    fn receives_heal(s: &Sim, ci: usize, caller: usize) -> bool {
        caller == ci && s.st::<State>(ci).own_heal
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // naBuff: Masque at a Bond of Life of 30% of max HP
            Event::HpDebt { on, .. } if on == ci => {
                if s.hp_debt(ci) >= s.max_hp(ci) * 0.3 {
                    s.add_status(ci, NA_BUFF, -1, false);
                } else {
                    s.delete_mod(ci, NA_BUFF);
                }
            }
            // a1OnKill: the directive's enemy dies, max-level debt
            Event::TargetDied(_) => {
                if !s.enemy_status_active(DIRECTIVE) {
                    return;
                }
                let new_debt = g::DIRECTIVE_SCALING[g::DIRECTIVE_SCALING.len() - 1] * s.max_hp(ci);
                add_skill_debt(s, ci, new_debt);
                remove_directive(s, ci);
            }
            _ => {}
        }
    }
}

/// A normal attack's hit: pyro, with the Bond's flat damage, under Masque.
fn na_hit(s: &mut Sim, ci: usize, n: usize, i: usize) {
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
    ai.abil = NORMAL_NAMES[n];
    // StrikeTypeSlash and StrikeTypeSpear: not blunt
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
    ai.def_halt = g::ATTACK_DEF_HALT[n][i];
    // gcsim reads the normal counter as the hit lands
    if s.chars[ci].normal_counter == 3 && i == 0 {
        ai.hitlag_factor = 0.0;
    }
    let mut na_index = 0;
    if s.mod_active(ci, NA_BUFF) {
        na_index = 1;
        ai.element = Element::Pyro;
        ai.ignore_infusion = true;
        ai.flat_dmg += bond_bonus(s, ci);
    }
    let hb = g::ATTACK_HITBOXES[na_index][n][i];
    let (ox, oy) = (g::ATTACK_OFFSETS[n][i][0], g::ATTACK_OFFSETS[n][i][1]);
    ai.pattern = if hb.len() == 1 { s.fan_on_player(ox, oy, hb[0], g::ATTACK_FAN_ANGLES[n][i]) } else { s.box_on_player(ox, oy, hb[0], hb[1]) };
    s.queue_attack(ai, CB_BOND, 0, 0);
}

/// bondBonus: Masque's (and C1's) share of her Bond of Life, as flat
/// damage scaling with ATK.
fn bond_bonus(s: &Sim, ci: usize) -> f64 {
    let c1 = if s.chars[ci].cons >= 1 { 1.0 } else { 0.0 };
    (g::MASQUE[s.talent(ci, 0)] + c1) * s.hp_debt(ci) / s.max_hp(ci) * s.total_atk(ci)
}

/// The skill's debt, capped by its limit while it lasts.
fn add_skill_debt(s: &mut Sim, ci: usize, mut new_debt: f64) {
    if s.mod_active(ci, DIRECTIVE_LIMIT) {
        let st = s.st::<State>(ci);
        new_debt = new_debt.min(st.skill_debt_max - st.skill_debt);
    }
    if new_debt > 0.0 {
        s.st_mut::<State>(ci).skill_debt += new_debt;
        s.modify_hp_debt(ci, new_debt);
    }
}

/// The directive's tags and status removed from the enemy.
fn remove_directive(s: &mut Sim, ci: usize) {
    let st = s.st_mut::<State>(ci);
    st.dir_level = 0;
    st.dir_src = 0;
    s.enemy_delete_status(DIRECTIVE);
}

/// absorbDirectives: the directive of an enemy within 6.5 of a point 3
/// ahead of her becomes Bond of Life by its level; C4 and, at level 2, C2.
fn absorb_directives(s: &mut Sim, ci: usize) {
    if !s.enemy_alive || !s.enemy_within(s.circle_on_player(0.0, 3.0, 6.5)) || !s.enemy_status_active(DIRECTIVE) {
        return;
    }
    let level = s.st::<State>(ci).dir_level;
    let new_debt = g::DIRECTIVE_SCALING[level as usize] * s.max_hp(ci);
    add_skill_debt(s, ci, new_debt);
    remove_directive(s, ci);
    c4_on_absorb(s, ci);
    if level >= 2 {
        c2_on_absorb_due(s, ci);
    }
}

/// C2: absorbing a level 2 directive, once every 10 s, a 900% pyro hit.
fn c2_on_absorb_due(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 2 || s.chars[ci].ascension < 1 || s.mod_active(ci, C2_ICD) {
        return;
    }
    s.add_status(ci, C2_ICD, 10 * 60, true);
    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, 9.0);
    ai.abil = "Balemoon Bloodfire (C2)";
    // NewCircleHit(Player, PrimaryTarget, {Y: 3}, 6.5)
    ai.pattern = s.circle_toward_target(0.0, 3.0, 6.5);
    s.queue_attack(ai, 0, 50, 50);
}

/// C4: absorbing a directive, once every 10 s, the burst's cooldown −2 s
/// and 15 energy.
fn c4_on_absorb(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 4 || s.mod_active(ci, C4_ICD) {
        return;
    }
    s.add_status(ci, C4_ICD, 10 * 60, true);
    s.reduce_cd(ci, Action::Burst, 2 * 60);
    s.add_energy(ci, 15.0);
}

/// C6: the burst's flat damage, 700% of ATK times her Bond of Life's share
/// of max HP.
fn c6_amount(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].cons < 6 {
        return 0.0;
    }
    s.total_atk(ci) * 7.0 * s.hp_debt(ci) / s.max_hp(ci)
}

/// lowPlungeXY and highPlungeXY: physical, or pyro under Masque.
fn plunge_xy(s: &mut Sim, ci: usize, high: bool, p: &Params) -> ActInfo {
    let masque = s.mod_active(ci, NA_BUFF);
    let pyro = |ai: &mut AttackInfo| {
        if masque {
            ai.element = Element::Pyro;
            ai.ignore_infusion = true;
        }
    };
    if plunge::wants_collision(p) {
        let mut ai = plunge::collision_info(s, ci, &PLUNGE);
        pyro(&mut ai);
        let h = PLUNGE.collision_hitmark;
        s.queue_attack(ai, 0, h, h);
    }
    let mut ai = plunge::hit_info(s, ci, high, &PLUNGE);
    pyro(&mut ai);
    let h = plunge::hitmark(&PLUNGE, high);
    s.queue_attack(ai, 0, h, h);
    plunge::act(&PLUNGE, high)
}
