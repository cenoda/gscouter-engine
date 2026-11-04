//! Qiqi, from gcsim's internal/characters/qiqi: normal and charged
//! attacks, the skill (the Herald of Frost: its initial hit, its swipes,
//! its heals, and with the revelation its coordinated attacks and
//! particles), the burst and its talisman with the talisman's heals, A1,
//! A4, C1, C2, C4's extra heal and C6's Glimpse of Mystery, and her
//! revelation (`revelation`, gcsim's default 1): the skill's cooldown cut
//! to 15 s, and in Radiance (a Polestar Field's or Stellar Swirl's status
//! on her, which she reads but does not make) the reaction bonuses, the
//! burst's direct Stellar-Conduct hit, A4's 100% chance and C1's and C2's
//! Radiance parts. gcsim implements neither C4's ATK reduction nor C6's
//! revival. Plunges from Xianyun's airborne state.

use crate::chars::plunge;
use crate::content::{Character, Event, EventKind, Owner};
use crate::gcsimdata::chars::qiqi as g;
use crate::sim::*;
use crate::stellar::{POLESTAR_FIELD, RADIANCE_SWIRL};
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The `revelation` param.
    revelation: bool,
    /// The frame the skill was last used (gcsim's skillLastUsed).
    skill_last_used: i32,
    /// gcsim's skillHealSnapshot: the snapshot's total ATK and Healing
    /// Bonus (zero before the first skill hits).
    heal_atk: f64,
    heal_bonus: f64,
    /// The swipes' snapshot, taken as the skill hits.
    tick: Option<AttackEvent>,
    /// C6's Glimpse of Mystery stacks.
    c6_stacks: i32,
    /// The enemy's qiqi-talisman-icd tag: the frame until which the
    /// talisman does not heal.
    talisman_icd: i32,
}

const SKILL_BUFF: ModKey = ModKey::new(g::SKILL_BUFF_KEY);
const SKILL_ICD: ModKey = ModKey::new(g::SKILL_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::SKILL_PARTICLE_ICD_KEY);
const TALISMAN: ModKey = ModKey::new(g::TALISMAN_KEY);
const A1: ModKey = ModKey::new("qiqi-a1");
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C2_RADIANCE: ModKey = ModKey::new("qiqi-c2-radiance");
const C6: ModKey = ModKey::new(g::C6_KEY);
const REV_SSC: ModKey = ModKey::new("qiqi-revelation-ssc");
const REV_SSW: ModKey = ModKey::new("qiqi-revelation-ssw");

/// skillHealCB.
const CB_HEAL: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;
const CB_C1: u16 = 1 << 2;
const CB_C1_REV: u16 = 1 << 3;
const CB_TALISMAN: u16 = 1 << 4;

// arg: the normal's index * 8 + the hit's
const TASK_NORMAL: u8 = 0;
/// arg: the skill's source frame.
const TASK_SKILL_HIT: u8 = 1;
const TASK_SKILL_CD: u8 = 2;
/// arg: the source frame * 2 + 1 when the last wait was 60 frames.
const TASK_DMG_TICK: u8 = 3;
/// arg: the source frame.
const TASK_HEAL_TICK: u8 = 4;

/// The airborne plunges (Xianyun's), from gcsim's plunge.go.
const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];
const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];

/// gcsim's getRadiance: Stellar-Conduct takes priority; nothing without
/// the revelation.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Radiance {
    None,
    Conduct,
    Swirl,
}

fn radiance(s: &Sim, ci: usize) -> Radiance {
    if !s.st::<State>(ci).revelation {
        Radiance::None
    } else if s.mod_active(ci, POLESTAR_FIELD) {
        Radiance::Conduct
    } else if s.mod_active(ci, RADIANCE_SWIRL) {
        Radiance::Swirl
    } else {
        Radiance::None
    }
}

/// The Herald of Frost's hits: cryo elemental art DMG of her ICD tag.
fn herald_info(ci: usize, abil: &'static str, mult: f64) -> AttackInfo {
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_QIQI_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, mult);
    ai.abil = abil;
    ai.hitlag_factor = 0.05;
    ai.hitlag_halt = 0.05 * 60.0;
    ai.def_halt = true;
    ai
}

/// The callbacks of the swipes and coordinated attacks.
fn herald_cbs(s: &Sim, ci: usize) -> u16 {
    if s.chars[ci].cons >= 1 { CB_PARTICLE | CB_C1 | CB_C1_REV } else { CB_PARTICLE }
}

/// gcsim's healSnapshot with the skill's snapshot.
fn heal_snapshot(s: &Sim, ci: usize, per: &[f64], flat: &[f64]) -> (f64, f64) {
    let lvl = s.talent(ci, 1);
    let st = s.st::<State>(ci);
    (flat[lvl] + st.heal_atk * per[lvl], st.heal_bonus)
}

impl Character for Char {
    const KEY: &'static str = "qiqi";
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        let revelation = cs.params.iter().find(|p| p.0 == "revelation").is_none_or(|p| p.1 > 0);
        State { revelation, ..State::default() }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons, rev) = (s.chars[ci].ascension, s.chars[ci].cons, s.st::<State>(ci).revelation);
        // skillInit (the coordinated attack, with the revelation), A4 and
        // C6 hook OnEnemyHit
        if rev || asc >= 4 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        // burstInit: the talisman's heal
        s.subscribe(EventKind::EnemyDamage, o);
        // revelationInit: the reaction bonuses on every character
        if rev {
            for i in 0..s.chars.len() {
                s.add_mod(i, Mod::owned(REV_SSC, ModKind::ReactBonus, o, st::NONE, false, false, -1), -1);
                s.add_mod(i, Mod::owned(REV_SSW, ModKind::ReactBonus, o, st::NONE, false, false, -1), -1);
            }
        }
        // a1Init: every reaction event
        if asc >= 1 {
            s.subscribe(EventKind::Reaction, o);
        }
        // c2Init
        if cons >= 2 {
            s.add_mod(ci, Mod::owned(C2, ModKind::Attack, o, st::NONE, false, false, -1), -1);
            if rev {
                s.add_mod(ci, Mod::owned(C2_RADIANCE, ModKind::Stat, o, st::ATKP, false, false, -1), -1);
            }
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

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
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
        for i in 0..g::CHARGE.len() {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[i][lvl]);
            ai.abil = CHARGE_NAMES[i];
            ai.pattern = s.circle_on_player(0.0, g::CHARGE_OFFSETS[i], g::CHARGE_RADIUS[i]);
            let d = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, CB_HEAL, d, d);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARKS[g::CHARGE_HITMARKS.len() - 1], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // +1 frame; a deployable after the initial hit, so not hitlag
        s.add_status(ci, SKILL_BUFF, 15 * 60 + 1, false);
        let src = s.f;
        s.st_mut::<State>(ci).skill_last_used = src;
        s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_HIT, arg: src }, g::SKILL_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_CD, arg: 0 }, 3);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // the talisman goes on with a 0 DMG hit well before the damage
        let mut tal = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 0.0, 0.0);
        tal.abil = "Fortune-Preserving Talisman (Talisman application)";
        let pattern = s.circle_on_player(0.0, 0.0, 7.0);
        tal.pattern = pattern;
        s.queue_attack(tal, CB_TALISMAN, 40, 40);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 50.0, g::BURST_DMG[lvl]);
        ai.abil = "Fortune-Preserving Talisman";
        ai.pattern = pattern;
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        if radiance(s, ci) == Radiance::Conduct {
            ai.abil = "Fortune-Preserving Talisman (Stellar-Conduct)";
            ai.mult = g::BURST_SSC[lvl];
            ai.tag = AttackTag::DirectStellarConduct;
            ai.ignore_def = 1.0;
            ai.durability = 0.0;
            ai.icd_tag = icd::TAG_NONE;
            s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        }
        // c6OnBurst
        if s.chars[ci].cons >= 6 && s.st::<State>(ci).revelation {
            s.add_status(ci, C6, 12 * 60, true);
            s.st_mut::<State>(ci).c6_stacks = 4;
        }
        s.set_cd(ci, Action::Burst, 20 * 60);
        s.consume_energy(ci, 8);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = ((arg / 8) as usize, (arg % 8) as usize);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.hitlag_factor = g::ATTACK_HITLAG_FACTOR[n][i];
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.def_halt = true;
                let off = g::ATTACK_OFFSETS[n][i];
                ai.pattern = if n == 1 { s.box_on_player(0.0, off, g::ATTACK_HITBOXES[n][0], g::ATTACK_HITBOXES[n][1]) } else { s.fan_on_player(0.0, off, g::ATTACK_HITBOXES[n][0], g::ATTACK_FAN_ANGLES[n][i]) };
                s.queue_attack(ai, CB_HEAL, 0, 0);
            }
            TASK_SKILL_HIT => {
                let src = arg;
                let lvl = s.talent(ci, 1);
                let mut ai = herald_info(ci, "Herald of Frost: Initial Damage", g::SKILL_INITIAL_DMG[lvl]);
                let mut snap = s.snapshot_event(ai, 0);
                // one heal at once, on the active character
                let (atk, bonus) = (total_atk(&snap.stats), snap.stats[st::HEAL]);
                {
                    let st = s.st_mut::<State>(ci);
                    st.heal_atk = atk;
                    st.heal_bonus = bonus;
                }
                let (heal, b) = heal_snapshot(s, ci, g::SKILL_HEAL_CONT_PER, g::SKILL_HEAL_CONT_FLAT);
                let active = s.player.active as usize;
                s.heal(ci, active, heal, b);
                s.add_task(Task::Char { c: ci as u8, id: TASK_HEAL_TICK, arg: src }, (4.5 * 60.0) as i32);
                // the swipes, snapshot now
                let mut tick = ai;
                tick.abil = "Herald of Frost: Skill Damage";
                tick.mult = g::SKILL_DMG_CONT[lvl];
                tick.deployable = true;
                let tick = s.snapshot_event(tick, 0);
                s.st_mut::<State>(ci).tick = Some(tick);
                // the initial hit's hitlag delays the first swipe
                s.add_task(Task::Char { c: ci as u8, id: TASK_DMG_TICK, arg: src * 2 + 1 }, 57 + 7);
                ai.pattern = s.circle_on_player(0.0, 0.0, 2.5);
                snap.info.pattern = ai.pattern;
                s.queue_attack_with_snap(snap, 0, 0);
            }
            TASK_SKILL_CD => {
                s.set_cd(ci, Action::Skill, 1800);
                // revelationSkillCDReduction
                if s.st::<State>(ci).revelation {
                    s.reduce_cd(ci, Action::Skill, 15 * 60);
                }
            }
            TASK_DMG_TICK => {
                let (src, last60) = (arg >> 1, arg & 1 == 1);
                if !s.mod_active(ci, SKILL_BUFF) || s.st::<State>(ci).skill_last_used > src {
                    return;
                }
                let Some(mut ev) = s.st::<State>(ci).tick else { return };
                // the Herald follows the player
                ev.info.pattern = s.circle_on_player(0.0, 0.0, 2.5);
                let cbs = herald_cbs(s, ci);
                s.queue_attack_with_snap(ev, cbs, 0);
                let next = if last60 { 135 } else { 60 };
                s.add_task(Task::Char { c: ci as u8, id: TASK_DMG_TICK, arg: src * 2 + (next == 60) as i32 }, next);
            }
            TASK_HEAL_TICK => {
                let src = arg;
                if !s.mod_active(ci, SKILL_BUFF) || s.st::<State>(ci).skill_last_used > src {
                    return;
                }
                let (heal, b) = heal_snapshot(s, ci, g::SKILL_HEAL_CONT_PER, g::SKILL_HEAL_CONT_FLAT);
                let active = s.player.active as usize;
                s.heal(ci, active, heal, b);
                s.add_task(Task::Char { c: ci as u8, id: TASK_HEAL_TICK, arg: src }, (4.5 * 60.0) as i32);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        // skillHealCB: her normal and charged hits heal the party during
        // the skill
        if cbs & CB_HEAL != 0 && s.mod_active(ci, SKILL_BUFF) {
            let (heal, b) = heal_snapshot(s, ci, g::SKILL_HEAL_ON_HIT_PER, g::SKILL_HEAL_ON_HIT_FLAT);
            s.heal_info(ci, None, false, heal, b);
        }
        if cbs & CB_PARTICLE != 0 && s.st::<State>(ci).revelation && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 6 * 60, true);
            s.queue_particle(2.0, Element::Cryo, s.particle_delay[ci]);
        }
        // C1: 2 energy on an enemy with the talisman
        if cbs & CB_C1 != 0 && s.enemy_status_active(TALISMAN) {
            s.add_energy(ci, 2.0);
        }
        // C1 in Radiance: 6 energy, once every 6 s
        if cbs & CB_C1_REV != 0 && radiance(s, ci) != Radiance::None && !s.mod_active(ci, C1_ICD) {
            s.add_status(ci, C1_ICD, 6 * 60, true);
            s.add_energy(ci, 6.0);
        }
        if cbs & CB_TALISMAN != 0 {
            s.enemy_add_status(TALISMAN, 15 * 60, true);
        }
    }

    /// C2 in Radiance: ATK +50%.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if radiance(s, ci) == Radiance::None {
            return false;
        }
        out[st::ATKP] += 0.5;
        true
    }

    /// C2: normal and charged attack DMG +15% on an enemy with cryo or
    /// frozen.
    fn attack_mod(s: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        if !s.enemy_aura_contains(Element::Cryo) {
            return;
        }
        ev.stats[st::DMGP] += 0.15;
    }

    /// The revelation: while the Herald is out, in Radiance, +50% on the
    /// reactions of that Radiance.
    fn react_bonus(s: &Sim, ci: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if !s.mod_active(ci, SKILL_BUFF) {
            return 0.0;
        }
        let rad = radiance(s, ci);
        if m.key == REV_SSC {
            if rad == Radiance::Conduct && matches!(info.tag, AttackTag::DirectStellarConduct | AttackTag::SuperconductDamage) {
                return 0.5;
            }
        } else if rad == Radiance::Swirl && matches!(info.tag, AttackTag::DirectStellarSwirl | AttackTag::ReactionStellarSwirl | AttackTag::SwirlCryo) {
            return 0.5;
        }
        0.0
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyHit(ev) => {
                let active = s.player.active as usize;
                let actor = ev.info.actor as usize;
                // skillInit: the coordinated attack, for the active
                // character's hits but EC, burning and hydro swirl, once
                // every 2.2 s per character
                if s.st::<State>(ci).revelation
                    && actor == active
                    && !matches!(ev.info.tag, AttackTag::ECDamage | AttackTag::BurningDamage | AttackTag::SwirlHydro)
                    && s.mod_active(ci, SKILL_BUFF)
                    && !s.mod_active(active, SKILL_ICD)
                {
                    s.add_status(active, SKILL_ICD, (2.2 * 60.0) as i32, true);
                    let lvl = s.talent(ci, 1);
                    let mut ai = herald_info(ci, "Herald of Frost: Coordinated Attack", g::SKILL_COORD[lvl]);
                    ai.deployable = true;
                    ai.pattern = s.circle_on_target(2.0);
                    let cbs = herald_cbs(s, ci);
                    s.queue_attack(ai, cbs, 1, 1);
                }
                // A4: her normal and charged hits put a talisman on the
                // enemy
                if s.chars[ci].ascension >= 4 && actor == ci && matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) && !s.mod_active(ci, A4_ICD) {
                    let (chance, cd) = if radiance(s, ci) == Radiance::None { (0.5, 30 * 60) } else { (1.0, 15 * 60) };
                    let exp = || s.enemy.mods.iter().find(|m| m.key == TALISMAN).map_or(0, |m| m.expiry());
                    // the draw is made either way; only then the expiry
                    if s.rng.float64() <= chance && exp() < s.f + 360 {
                        s.enemy_add_status(TALISMAN, 360, true);
                        s.add_status(ci, A4_ICD, cd, true);
                    }
                }
                // C6: Glimpse of Mystery, on the others' direct Stellar hits
                if s.chars[ci].cons >= 6
                    && s.st::<State>(ci).revelation
                    && s.st::<State>(ci).c6_stacks > 0
                    && actor != ci
                    && actor == active
                    && s.mod_active(ci, C6)
                    && matches!(ev.info.tag, AttackTag::DirectStellarConduct | AttackTag::DirectStellarSwirl)
                {
                    s.st_mut::<State>(ci).c6_stacks -= 1;
                    s.hit_flat += 6.0 * s.total_atk(ci);
                }
            }
            // burstInit: the talisman heals the attacker, once a second
            Event::EnemyDamage(ev) => {
                if !s.enemy_status_active(TALISMAN) || s.st::<State>(ci).talisman_icd >= s.f {
                    return;
                }
                let lvl = s.talent(ci, 2);
                let heal = g::BURST_HEAL_FLAT[lvl] + s.total_atk(ci) * g::BURST_HEAL_PER[lvl];
                let bonus = s.stat(ci, st::HEAL);
                s.heal(ci, ev.info.actor as usize, heal, bonus);
                s.st_mut::<State>(ci).talisman_icd = s.f + 60;
                // C4, with the revelation: the lowest HP ratio heals too
                if s.chars[ci].cons >= 4 && s.st::<State>(ci).revelation {
                    let ratio = |s: &Sim, i: usize| s.current_hp(i) / s.max_hp(i);
                    let (mut low, mut low_r) = (ci, ratio(s, ci));
                    for i in 0..s.chars.len() {
                        let r = ratio(s, i);
                        if r < low_r {
                            low = i;
                            low_r = r;
                        }
                    }
                    let heal = 1.8 * s.total_atk(ci);
                    let bonus = s.stat(ci, st::HEAL);
                    s.heal(ci, low, heal, bonus);
                }
            }
            // A1: a reaction by the active character during the skill
            // raises its incoming healing for 8 s
            Event::Reaction(_, ev) => {
                if !s.mod_active(ci, SKILL_BUFF) {
                    return;
                }
                let active = s.player.active as usize;
                if ev.info.actor as usize != active {
                    return;
                }
                let mut m = Mod::new(A1, ModKind::HealBonus, st::HEAL, false, true, -1);
                m.value = 0.2;
                s.add_mod(active, m, 8 * 60);
            }
            _ => {}
        }
    }
}
