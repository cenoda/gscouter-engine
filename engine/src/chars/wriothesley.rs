//! Wriothesley, from gcsim's internal/characters/wriothesley: normal
//! attacks (with Chilling Penalty), charged attacks (A1's Rebuke), the
//! skill, the burst with its Surging Blade, A1, A4 and C1 to C6, with
//! Radiance: Stellar-Conduct (stellar.go, and the Radiance halves of C1,
//! C2, C4 and C6): inside a Polestar Field, with the revelation, the third
//! and fifth enhanced normal attacks deal direct Stellar-Conduct DMG, a
//! charged attack below 60% HP is Luster: Vaulting Fist, and he deals 30%
//! more Stellar-Conduct DMG. C4's 25% damage reduction is a TODO in gcsim
//! too. No plunges.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::wriothesley as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    saved_normal: i32,
    a4_stacks: i32,
    burst: Option<AttackEvent>,
    /// The `revelation` param (gcsim's default 1).
    revelation: bool,
    /// C1: a fifth normal attack hit during the skill readied a Rebuke.
    c1_n5: bool,
    /// C1: the skill was extended once already.
    c1_extended: bool,
    /// Luster: Vaulting Fist, snapshot as it is used.
    luster: Option<AttackEvent>,
}

const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const LOSE_HP_ICD: ModKey = ModKey::new(g::SKILL_LOSE_HPICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new(g::A1_STATUS);
const A1_ICD: ModKey = ModKey::new(g::A1_ICD_KEY);
const A4: ModKey = ModKey::new("wriothesley-a4");
const OUSIA_ICD: ModKey = ModKey::new(g::BURST_OUSIA_ICD_KEY);
const C1: ModKey = ModKey::new(g::C1_STATUS);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C4: ModKey = ModKey::new(g::C4_STATUS);
const C4_REVELATION: ModKey = ModKey::new("wriothesley-c4-spd-revelation");
const A1_STELLAR: ModKey = ModKey::new("wriothesley-a1-stellar");
const A1_STELLAR_HEAL_ICD: ModKey = ModKey::new("wriothesley-a1-stellar-icd");
const C1_REV_CA: ModKey = ModKey::new("wriothesley-c1-revelation-ca");
const C1_REV_N5: ModKey = ModKey::new("wriothesley-c1-revelation-n5");

const CB_PARTICLE: u16 = 1 << 0;
const CB_PENALTY: u16 = 1 << 1;
const CB_REBUKE: u16 = 1 << 2;
const CB_C1_N5: u16 = 1 << 3;
/// Luster: Vaulting Fist's callbacks (A1's heal, C1), and its use of C1's
/// bonus.
const CB_LUSTER: u16 = 1 << 4;
/// The fifth enhanced normal attack as Stellar-Conduct DMG uses C1's bonus.
const CB_SSC_N5: u16 = 1 << 5;

// arg: the normal's index * 8 + the hit's
const TASK_NORMAL: u8 = 0;
const TASK_OUSIA: u8 = 1;
const TASK_LUSTER: u8 = 2;
// arg: 0 for C1's bonus on Luster: Vaulting Fist, 1 on the fifth normal
const TASK_DELETE_C1_BONUS: u8 = 3;

const NAMES: [&str; 5] = ["Normal 1", "Normal 2", "Normal 3", "Normal 4", "Normal 5"];
const ENHANCED: [&str; 5] = ["Normal 1 (Enhanced)", "Normal 2 (Enhanced)", "Normal 3 (Enhanced)", "Normal 4 (Enhanced)", "Normal 5 (Enhanced)"];
const ENHANCED_SSC: [&str; 5] = ["", "", "Normal 3 (Enhanced) (Stellar-Conduct)", "", "Normal 5 (Enhanced) (Stellar-Conduct)"];

/// Radiance: Stellar-Conduct: inside a Polestar Field (gcsim's
/// isStellarRadiance).
fn radiance(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, crate::stellar::POLESTAR_FIELD)
}

/// gcsim's a1StellarEnabled: Radiance with the revelation.
fn a1_stellar(s: &Sim, ci: usize) -> bool {
    s.st::<State>(ci).revelation && radiance(s, ci)
}

/// gcsim's a1StellarCAReady: Luster: Vaulting Fist, below 60% HP.
fn luster_ready(s: &Sim, ci: usize) -> bool {
    s.chars[ci].ascension >= 1 && a1_stellar(s, ci) && s.chars[ci].hp_ratio < 0.6
}

/// C2 (with the revelation) at 5 Prosecution Edict stacks during Chilling
/// Penalty's buff, in Radiance: +50% on the Stellar-Conduct hits.
fn c2_revelation_ssc(s: &Sim, ci: usize) -> f64 {
    let c = &s.chars[ci];
    let st = s.st::<State>(ci);
    if c.cons >= 2 && st.revelation && c.ascension >= 4 && st.a4_stacks >= 5 && skill_buff(s, ci) && radiance(s, ci) {
        0.5
    } else {
        0.0
    }
}

/// C6: +10% CRIT Rate and +80% CRIT DMG on a snapshot.
fn add_c6_buff(s: &Sim, ci: usize, ev: &mut AttackEvent) {
    if s.chars[ci].cons >= 6 {
        ev.stats[st::CR] += 0.1;
        ev.stats[st::CD] += 0.8;
    }
}

/// Chilling Penalty's buff: the skill's status, above half HP.
fn skill_buff(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, SKILL) && s.chars[ci].hp_ratio > 0.5
}

/// A1's Gracious Rebuke: below 60% HP, off its cooldown. At C1, also after
/// a fifth normal attack hit during the skill, and on C1's cooldown instead
/// of A1's.
fn rebuke_ready(s: &Sim, ci: usize) -> bool {
    if s.chars[ci].ascension < 1 {
        return false;
    }
    if s.chars[ci].cons >= 1 {
        return s.st::<State>(ci).c1_n5 || (s.chars[ci].hp_ratio < 0.6 && !s.mod_active(ci, C1_ICD));
    }
    s.chars[ci].hp_ratio < 0.6 && !s.mod_active(ci, A1_ICD)
}

/// C2 (with the revelation) at 5 Prosecution Edict stacks during Chilling
/// Penalty's buff, outside Radiance: amt, the factor on normal attacks
/// (0.25) or charged attacks (0.3); else 0.
fn c2_revelation(s: &Sim, ci: usize, amt: f64) -> f64 {
    let c = &s.chars[ci];
    let st = s.st::<State>(ci);
    if c.cons >= 2 && st.revelation && c.ascension >= 4 && st.a4_stacks >= 5 && skill_buff(s, ci) && !radiance(s, ci) {
        amt
    } else {
        0.0
    }
}

/// C1: the skill's status gains 4 s once per skill.
fn c1_extend(s: &mut Sim, ci: usize) {
    if !s.st::<State>(ci).c1_extended && s.mod_active(ci, SKILL) {
        s.extend_mod(ci, SKILL, g::C1_SKILL_EXTENSION);
        s.st_mut::<State>(ci).c1_extended = true;
    }
}

/// Dash and skill keep the normal attack combo only after a normal attack
/// or the skill.
fn keep_combo(s: &mut Sim, ci: usize) {
    if !matches!(s.player.last.action, Action::Attack | Action::Skill) {
        s.st_mut::<State>(ci).saved_normal = 0;
    }
}

impl Character for Char {
    const KEY: &'static str = "wriothesley";
    const MAX_CONS: i32 = 6;
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        let revelation = cs.params.iter().find(|p| p.0 == "revelation").is_none_or(|p| p.1 > 0);
        State { revelation, ..State::default() }
    }

    fn init(s: &mut Sim, ci: usize) {
        s.subscribe(EventKind::Swap, Owner::Char(ci as u8));
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::Drain, Owner::Char(ci as u8));
        }
        if s.chars[ci].ascension >= 4 || s.chars[ci].cons >= 4 {
            s.subscribe(EventKind::Heal, Owner::Char(ci as u8));
        }
        // the revelation's Radiance parts, which need a Polestar Field (only
        // a Stellar character makes one; without one they never apply)
        let revelation = s.st::<State>(ci).revelation;
        if revelation && s.stellar.conduct {
            let o = Owner::Char(ci as u8);
            // stellarInit: Stellar-Conduct DMG +30%
            s.chars[ci].mods.push(Mod::owned(A1_STELLAR, ModKind::ReactBonus, o, st::NONE, false, false, -1));
            // C4 in Radiance: ATK SPD +20% for him, +10% for the others,
            // unless C4's own buff is on
            if s.chars[ci].cons >= 4 {
                for i in 0..s.chars.len() {
                    s.chars[i].mods.push(Mod::owned(C4_REVELATION, ModKind::Stat, o, st::ATKSPD, false, false, -1));
                }
            }
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// He charges only right after a normal attack.
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        (a == Action::Charge && s.player.last.action != Action::Attack).then(|| "need to use attack right before charge".into())
    }

    fn charge_stam(s: &Sim, ci: usize) -> Option<f64> {
        (rebuke_ready(s, ci) || luster_ready(s, ci)).then_some(0.0)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "gracious-rebuke" => Some(Ok(Num::from_bool(rebuke_ready(s, ci)))),
            "gracious-rebuke-unveiling" => Some(Ok(Num::from_bool(luster_ready(s, ci)))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // the combo survives the skill and dashes
        if matches!(s.current_state(), AnimState::Dash | AnimState::Skill) {
            s.chars[ci].normal_counter = s.st::<State>(ci).saved_normal;
        }
        let n = s.chars[ci].normal_counter as usize;
        let hitmarks = g::ATTACK_HITMARKS[n];
        for (i, &h) in hitmarks.iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: (n * 8 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hitmarks[hitmarks.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        let next = s.chars[ci].normal_counter;
        s.st_mut::<State>(ci).saved_normal = next;
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        // Radiance: Luster: Vaulting Fist, direct Stellar-Conduct DMG
        if luster_ready(s, ci) {
            let mut ai = AttackInfo::new(ci, AttackTag::DirectStellarConduct, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 0.0, g::CHARGE[lvl] * (1.0 + c2_revelation_ssc(s, ci)));
            ai.abil = "Luster: Vaulting Fist (Stellar-Conduct)";
            ai.add_tags = add_tag::WRIO_SSC_CA;
            ai.blunt = true;
            ai.poise = 110.0;
            ai.ignore_def = 1.0;
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = 0.12 * 60.0;
            ai.pattern = s.box_on_player(0.0, -0.8, 4.0, 5.0);
            let mut ev = s.snapshot_event(ai, 0);
            add_c6_buff(s, ci, &mut ev);
            s.st_mut::<State>(ci).luster = Some(ev);
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_LUSTER, arg: 0 }, g::CHARGE_HITMARK);
            let t = &g::CHARGE_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.blunt = true;
        ai.poise = 110.0;
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = 0.09 * 60.0;
        let rebuke = rebuke_ready(s, ci);
        ai.pattern = if rebuke { s.box_on_player(0.0, -0.8, 4.0, 5.0) } else { s.box_on_player(0.0, -1.2, 2.8, 3.6) };
        let cbs = if rebuke { CB_REBUKE | CB_PARTICLE } else { 0 };
        let mut ev = s.snapshot_event(ai, cbs);
        let cons = s.chars[ci].cons;
        // A1: Rebuke: Vaulting Fist, +50% DMG (C1: +200%), heals on hit
        if rebuke {
            if cons >= 1 {
                s.st_mut::<State>(ci).c1_n5 = false;
            }
            s.add_status(ci, if cons >= 1 { C1 } else { A1 }, -1, false);
            ev.info.abil = "Rebuke: Vaulting Fist";
            ev.info.hitlag_factor = 0.03;
            ev.info.hitlag_halt = 0.12 * 60.0;
            ev.stats[st::DMGP] += if cons >= 1 { 2.0 } else { 0.5 };
            // C6: +10% crit rate, +80% crit DMG
            if cons >= 6 {
                ev.stats[st::CR] += 0.1;
                ev.stats[st::CD] += 0.8;
            }
        }
        ev.info.mult *= 1.0 + c2_revelation(s, ci, 0.3);
        s.queue_attack_with_snap(ev, cbs, g::CHARGE_HITMARK);
        // C6: an icicle with the Rebuke's damage, on its snapshot and hit
        // pattern
        if rebuke && cons >= 6 {
            ev.info.abil = "Rebuke: Vaulting Fist (C6)";
            ev.info.blunt = false;
            ev.info.poise = 50.0;
            s.queue_attack_with_snap(ev, cbs, g::CHARGE_HITMARK);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn dash_info(s: &mut Sim, ci: usize, info: ActInfo) -> ActInfo {
        keep_combo(s, ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        keep_combo(s, ci);
        let dur = 10 * 60 + g::SKILL_CD_DELAY;
        s.st_mut::<State>(ci).a4_stacks = 0;
        s.st_mut::<State>(ci).c1_extended = false;
        s.add_status(ci, SKILL, dur, true);
        // A4: 6% ATK for each Prosecution Edict stack
        if s.chars[ci].ascension >= 4 {
            s.add_mod(ci, Mod::owned(A4, ModKind::Stat, Owner::Char(ci as u8), st::ATKP, false, true, -1), dur);
        }
        s.set_cd_with_delay(ci, Action::Skill, 16 * 60, g::SKILL_CD_DELAY);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Burst as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST[lvl]);
        ai.abil = "Darkgold Wolfbite";
        ai.pattern = s.box_on_player(0.0, 0.0, 8.0, 16.0);
        let mut ev = s.snapshot_event(ai, 0);
        // C2: 40% DMG for each Prosecution Edict stack during the skill, for
        // the Surging Blade too
        if s.chars[ci].ascension >= 4 && s.chars[ci].cons >= 2 && s.mod_active(ci, SKILL) {
            ev.stats[st::DMGP] += 0.4 * s.st::<State>(ci).a4_stacks as f64;
        }
        for &h in g::BURST_HITMARKS {
            s.queue_attack_with_snap(ev, 0, h);
        }
        s.st_mut::<State>(ci).burst = Some(ev);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_OUSIA, arg: 0 }, g::BURST_OUSIA_HITMARK);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 5);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = ((arg / 8) as usize, (arg % 8) as usize);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NAMES[n];
                ai.hitlag_factor = g::ATTACK_HITLAG_FACTOR[n][i];
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.def_halt = true;
                // Chilling Penalty, decided as the hit lands
                let buff = skill_buff(s, ci);
                // C1's callback goes on the fifth hit, enhanced or not
                let mut cbs = if n == 4 && s.chars[ci].cons >= 1 { CB_C1_N5 } else { 0 };
                if buff {
                    cbs |= CB_PARTICLE | CB_PENALTY;
                    ai.abil = ENHANCED[n];
                    ai.mult *= g::SKILL[s.talent(ci, 1)];
                    ai.mult *= 1.0 + c2_revelation(s, ci, 0.25);
                }
                // Radiance: the third and fifth enhanced hits are direct
                // Stellar-Conduct DMG, at 60% and 80%
                let stellar = buff && a1_stellar(s, ci);
                if stellar && (n == 2 || n == 4) {
                    ai.abil = ENHANCED_SSC[n];
                    ai.tag = AttackTag::DirectStellarConduct;
                    ai.ignore_def = 1.0;
                    ai.durability = 0.0;
                    if n == 2 {
                        ai.mult *= 0.6;
                    } else {
                        ai.add_tags = add_tag::WRIO_SSC_N5;
                        ai.mult *= 0.8;
                        cbs |= CB_SSC_N5;
                    }
                    ai.mult *= 1.0 + c2_revelation_ssc(s, ci);
                }
                // gcsim reads the hitbox at the combo's counter now, not the
                // hit's
                let k = buff as usize;
                let box_ = g::ATTACK_HITBOXES[k][s.chars[ci].normal_counter as usize];
                ai.pattern = s.box_on_player(0.0, g::ATTACK_OFFSETS[k], box_[0], box_[1]);
                let mut ev = s.snapshot_event(ai, cbs);
                if stellar {
                    add_c6_buff(s, ci, &mut ev);
                }
                s.queue_attack_with_snap(ev, cbs, 0);
                // C6 in Radiance: an icicle of 60% of the fifth hit, as
                // Stellar-Conduct DMG
                if n == 4 && stellar && s.chars[ci].cons >= 6 {
                    ev.info.abil = "Normal 5 (Enhanced) (C6) (Stellar-Conduct)";
                    ev.info.mult *= 0.6;
                    ev.info.deployable = true;
                    s.queue_attack_with_snap(ev, CB_PARTICLE, 0);
                }
            }
            TASK_LUSTER => {
                let Some(mut ev) = s.st_mut::<State>(ci).luster.take() else { return };
                s.queue_attack_with_snap(ev, CB_LUSTER, 0);
                // C6 in Radiance: an icicle of 60% of it
                if s.chars[ci].cons >= 6 && skill_buff(s, ci) && a1_stellar(s, ci) {
                    ev.info.abil = "Luster: Vaulting Fist (C6) (Stellar-Conduct)";
                    ev.info.mult *= 0.6;
                    ev.info.deployable = true;
                    s.queue_attack_with_snap(ev, 0, 0);
                }
            }
            TASK_DELETE_C1_BONUS => s.delete_mod(ci, if arg == 0 { C1_REV_CA } else { C1_REV_N5 }),
            // Surging Blade, once every 10 s, on the burst's snapshot
            TASK_OUSIA => {
                if s.mod_active(ci, OUSIA_ICD) {
                    return;
                }
                s.add_status(ci, OUSIA_ICD, 10 * 60, true);
                let Some(mut ev) = s.st::<State>(ci).burst else { return };
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 0.0, g::BURST_OUSIA[lvl]);
                ai.abil = "Surging Blade";
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = 0.03 * 60.0;
                ai.pattern = s.box_on_player(0.0, 0.0, 10.0, 16.0);
                ev.info = ai;
                s.queue_attack_with_snap(ev, 0, 0);
            }
            _ => unreachable!(),
        }
    }

    fn mod_amount(s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C4_REVELATION {
            if s.mod_active(on, C4) || !radiance(s, ci) {
                return false;
            }
            out[st::ATKSPD] += if on == ci { 0.2 } else { 0.1 };
            return true;
        }
        out[st::ATKP] += s.st::<State>(ci).a4_stacks as f64 * 0.06;
        true
    }

    /// Stellar-Conduct DMG +30% (stellarInit); C1's +50% on Luster:
    /// Vaulting Fist or on the fifth enhanced hit.
    fn react_bonus(_: &Sim, _: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if m.key == A1_STELLAR {
            return if info.tag == AttackTag::DirectStellarConduct { 0.3 } else { 0.0 };
        }
        let tag = if m.key == C1_REV_CA { add_tag::WRIO_SSC_CA } else { add_tag::WRIO_SSC_N5 };
        if info.has_tag(tag) { 0.5 } else { 0.0 }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        // C1's bonus deletes itself once it counted (a task at the end of
        // the frame: the C6 icicle right after still has it)
        if cbs & CB_LUSTER != 0 && s.mod_active(ci, C1_REV_CA) {
            s.add_task(Task::Char { c: ci as u8, id: TASK_DELETE_C1_BONUS, arg: 0 }, 0);
        }
        if cbs & CB_SSC_N5 != 0 && s.mod_active(ci, C1_REV_N5) {
            s.add_task(Task::Char { c: ci as u8, id: TASK_DELETE_C1_BONUS, arg: 1 }, 0);
        }
        // C1: a fifth normal attack hit during the skill readies a Rebuke;
        // in Radiance it gives the next Luster: Vaulting Fist 50% more
        // Stellar-Conduct DMG instead
        if cbs & CB_C1_N5 != 0 {
            if a1_stellar(s, ci) && skill_buff(s, ci) {
                s.add_mod(ci, Mod::owned(C1_REV_CA, ModKind::ReactBonus, Owner::Char(ci as u8), st::NONE, false, true, -1), 5 * 60);
            } else if s.mod_active(ci, SKILL) && !s.mod_active(ci, C1_ICD) {
                s.st_mut::<State>(ci).c1_n5 = true;
            }
        }
        // Luster: Vaulting Fist: heals 30% of his max HP once every 2 s;
        // C1: the next fifth enhanced hit in 5 s deals 50% more
        // Stellar-Conduct DMG, and the skill is extended
        if cbs & CB_LUSTER != 0 {
            if !s.mod_active(ci, A1_STELLAR_HEAL_ICD) {
                s.add_status(ci, A1_STELLAR_HEAL_ICD, 2 * 60, true);
                let (hp, bonus) = (0.3 * s.max_hp(ci), s.stat(ci, st::HEAL));
                s.heal(ci, ci, hp, bonus);
            }
            if s.chars[ci].cons >= 1 && a1_stellar(s, ci) {
                s.add_mod(ci, Mod::owned(C1_REV_N5, ModKind::ReactBonus, Owner::Char(ci as u8), st::NONE, false, true, -1), 5 * 60);
                c1_extend(s, ci);
            }
        }
        let cons = s.chars[ci].cons;
        if cbs & CB_REBUKE != 0 && cons < 1 && s.mod_active(ci, A1) {
            s.delete_mod(ci, A1);
            s.add_status(ci, A1_ICD, g::A1_ICD, true);
            let (hp, bonus) = (0.3 * s.max_hp(ci), s.stat(ci, st::HEAL));
            s.heal(ci, ci, hp, bonus);
        }
        // C1's Rebuke: its cooldown, the skill's extension, and the heal
        // (C4: 50% of max HP)
        if cbs & CB_REBUKE != 0 && cons >= 1 && s.mod_active(ci, C1) {
            s.delete_mod(ci, C1);
            s.add_status(ci, C1_ICD, g::C1_ICD as i32, true);
            c1_extend(s, ci);
            let frac = if cons >= 4 { 0.5 } else { 0.3 };
            let (hp, bonus) = (frac * s.max_hp(ci), s.stat(ci, st::HEAL));
            s.heal(ci, ci, hp, bonus);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD, true);
            s.queue_particle(1.0, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_PENALTY != 0 && !s.mod_active(ci, LOSE_HP_ICD) {
            s.add_status(ci, LOSE_HP_ICD, g::SKILL_LOSE_HPICD, true);
            let hp = 0.045 * s.max_hp(ci);
            s.drain(ci, hp);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // leaving the field ends the skill
            Event::Swap { prev, .. } => {
                if prev == ci && s.mod_active(ci, SKILL) {
                    s.delete_mod(ci, SKILL);
                }
            }
            // A4: a stack whenever his HP changes during the skill, on field
            Event::Drain { on, amount, .. } => {
                if s.player.active as usize == ci && on == ci && amount > 0.0 {
                    a4_stack(s, ci);
                }
            }
            Event::Heal { on, amount, overheal, .. } => {
                if s.chars[ci].ascension >= 4 && s.player.active as usize == ci && on == ci && amount > 0.0 && (amount - overheal).abs() > 1e-9 {
                    a4_stack(s, ci);
                }
                // C4: an overflowing heal on him gives him 20% ATK SPD for
                // 4 s on field, or the party 10% for 6 s off field; not in
                // Radiance (with the revelation), which has its own
                if s.chars[ci].cons >= 4 && on == ci && amount > 0.0 && overheal > 0.0 && !a1_stellar(s, ci) {
                    for i in 0..s.chars.len() {
                        s.delete_mod(i, C4);
                    }
                    if s.player.active as usize == ci {
                        s.add_stat_mod(ci, C4, st::ATKSPD, 0.2, 4 * 60, true);
                    } else {
                        for i in 0..s.chars.len() {
                            s.add_stat_mod(i, C4, st::ATKSPD, 0.1, 6 * 60, true);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn a4_stack(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, SKILL) {
        let st = s.st_mut::<State>(ci);
        st.a4_stacks = (st.a4_stacks + 1).min(5);
    }
}
