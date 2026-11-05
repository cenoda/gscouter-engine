//! Mika, from gcsim's internal/characters/mika, all of it: normal attacks,
//! the charged attack, Starfrost Swirl pressed (Flowfrost Arrow) or held
//! (Rimestar Flare), Soulwind's ATK SPD for the party, Skyfeather Song's
//! heal and Eagleplume's heals on normal attacks, A1's Detector stacks
//! (Physical DMG for the active character), A4 and C1 to C6. gcsim's
//! package has no plunge.
//!
//! The engine fights one enemy: Flowfrost Arrow's A1 stacks come from
//! every enemy after the first, and Rimestar Flare's shards fall on up to
//! three enemies other than the one hit, so neither ever happens (as in
//! gcsim against one enemy).

use crate::content::{Character, Event};
use crate::gamedata::chars::mika as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    max_stacks: i32,
    stacks: i32,
    heal_icd: i32,
    a4_stack: bool,
    c4_count: i32,
}

const SKILL_BUFF: ModKey = ModKey::new(g::SKILL_BUFF_KEY);
const A1_BUFF: ModKey = ModKey::new(g::A1_BUFF);
const C6: ModKey = ModKey::new("mika-c6");
const HEAL: ModKey = ModKey::new(g::HEAL_KEY);
const HEAL_ICD: ModKey = ModKey::new(g::HEAL_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_C2: u16 = 1 << 1;

const TASK_BUFFS: u8 = 0;
const TASK_BURST_HEAL: u8 = 1;

impl Character for Char {
    const KEY: &'static str = "mika";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { max_stacks: 3, stacks: 0, heal_icd: 150, a4_stack: false, c4_count: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let lvl = s.talent(ci, 1);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        let st = s.st_mut::<State>(ci);
        if asc >= 4 {
            st.max_stacks += 1;
        }
        if cons >= 1 {
            st.heal_icd = (150.0 * (1.0 - g::ATK_SPD_BUFF[lvl])) as i32;
        }
        if cons >= 6 {
            st.max_stacks += 1;
        }
        // onBurstHeal, and A4 after it, on OnEnemyDamage
        s.subscribe(EventKind::EnemyDamage, Owner::Char(ci as u8));
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match (fields[0].as_str(), fields.get(1).map(String::as_str)) {
            ("tags", Some(g::A1_STACKS)) => Some(Ok(Num::Int(s.st::<State>(ci).stacks as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.hitlag_factor = 0.01;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let hb = g::ATTACK_HITBOXES[n][i];
            let oy = g::ATTACK_OFFSETS[n];
            ai.pattern = match n {
                1 | 3 => s.fan_on_player(0.0, oy, hb[0], g::ATTACK_FAN_ANGLES[n]),
                _ => s.box_on_player(0.0, oy, hb[0], hb[1]),
            };
            s.queue_attack_at(ci, ai, 0, g::ATTACK_HITMARKS[n][i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = s.circle_on_target(0.8);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let c2 = if s.chars[ci].cons >= 2 && s.chars[ci].ascension >= 1 { CB_C2 } else { 0 };
        if p.get("hold").unwrap_or(0) != 0 {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL_HOLD[lvl]);
            ai.abil = "Rimestar Flare";
            ai.pattern = Pattern::Single;
            s.queue_attack(ai, CB_PARTICLE | c2, g::SKILL_HOLD_HITMARK, g::SKILL_HOLD_HITMARK + g::SKILL_HOLD_TRAVEL);
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BUFFS, arg: 0 }, g::SKILL_HOLD_CD_START + 1);
            s.set_cd_with_delay(ci, Action::Skill, 15 * 60, g::SKILL_HOLD_CD_START + 1);
            let t = &g::SKILL_HOLD_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL_PRESS[lvl]);
        ai.abil = "Flowfrost Arrow";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.pattern = s.box_on_player(0.0, -0.4, 2.5, 10.0);
        s.queue_attack(ai, CB_PARTICLE | c2, g::SKILL_PRESS_HITMARK, g::SKILL_PRESS_HITMARK + g::SKILL_PRESS_TRAVEL);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BUFFS, arg: 0 }, g::SKILL_PRESS_CD_START);
        s.set_cd_with_delay(ci, Action::Skill, 15 * 60, g::SKILL_PRESS_CD_START);
        let t = &g::SKILL_PRESS_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Walk as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_HEAL, arg: 0 }, g::INITIAL_HEAL);
        s.set_cd(ci, Action::Burst, 18 * 60);
        s.consume_energy(ci, 6);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_BUFFS => {
                s.st_mut::<State>(ci).stacks = 0;
                let o = Owner::Char(ci as u8);
                let lvl = s.talent(ci, 1);
                let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
                for i in 0..s.chars.len() {
                    s.add_mod(i, Mod::stat(SKILL_BUFF, st::ATKSPD, g::ATK_SPD_BUFF[lvl], false, true, -1), g::SKILL_BUFF_DURATION);
                    if asc >= 1 {
                        s.add_mod(i, Mod::owned(A1_BUFF, ModKind::Stat, o, st::PHYP, false, true, -1), g::SKILL_BUFF_DURATION);
                    }
                    if cons >= 6 {
                        s.add_mod(i, Mod::owned(C6, ModKind::Attack, o, st::NONE, false, true, -1), g::SKILL_BUFF_DURATION);
                    }
                }
                if asc >= 4 {
                    s.st_mut::<State>(ci).a4_stack = false;
                }
            }
            TASK_BURST_HEAL => {
                let lvl = s.talent(ci, 2);
                let src = g::BURST_HEAL_FIRST_F[lvl] + g::BURST_HEAL_FIRST_P[lvl] * s.max_hp(ci);
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, None, false, src, bonus);
                if s.chars[ci].cons >= 4 {
                    s.st_mut::<State>(ci).c4_count = 5;
                }
                s.add_status(ci, HEAL, 15 * 60, false);
            }
            _ => unreachable!(),
        }
    }

    /// A1's Physical DMG, for the active character only.
    fn mod_amount(s: &Sim, ci: usize, on: usize, _: &Mod, out: &mut Stats) -> bool {
        if s.player.active as usize != on {
            return false;
        }
        out[st::PHYP] += 0.1 * s.st::<State>(ci).stacks as f64;
        true
    }

    /// C6's CRIT DMG on the active character's physical attacks.
    fn attack_mod(s: &Sim, _: usize, on: usize, _: &Mod, ev: &mut AttackEvent) {
        if s.player.active as usize != on || ev.info.element != Element::Physical {
            return;
        }
        ev.stats[st::CD] += 0.6;
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 {
            s.queue_particle(4.0, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_C2 != 0 {
            add_stack(s, ci);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        let actor = ev.info.actor as usize;
        // onBurstHeal: Eagleplume on a normal attack, per attacker's ICD
        if s.mod_active(ci, HEAL) && ev.info.tag == AttackTag::Normal && !s.mod_active(actor, HEAL_ICD) {
            let icd = s.st::<State>(ci).heal_icd;
            s.add_status(actor, HEAL_ICD, icd, true);
            let lvl = s.talent(ci, 2);
            let src = g::BURST_HEAL_F[lvl] + g::BURST_HEAL_P[lvl] * s.max_hp(ci);
            let bonus = s.stat(ci, st::HEAL);
            s.heal_info(ci, Some(actor), false, src, bonus);
            if s.chars[ci].cons >= 4 && s.st::<State>(ci).c4_count > 0 {
                s.add_energy(ci, 3.0);
                s.st_mut::<State>(ci).c4_count -= 1;
            }
        }
        // A4: one stack on a crit of the active character under both buffs
        if s.chars[ci].ascension >= 4 && !s.st::<State>(ci).a4_stack && actor == s.player.active as usize && s.mod_active(actor, SKILL_BUFF) && s.mod_active(ci, HEAL) && ev.crit {
            add_stack(s, ci);
            s.st_mut::<State>(ci).a4_stack = true;
        }
    }
}

fn add_stack(s: &mut Sim, ci: usize) {
    let st = s.st_mut::<State>(ci);
    if st.stacks < st.max_stacks {
        st.stacks += 1;
    }
}
