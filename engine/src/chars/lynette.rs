//! Lynette, from gcsim's internal/characters/lynette: normal and charged
//! attacks, the pressed skill with its Surging Blade, heal and drains, the
//! burst's Bogglecat Box (a gadget that absorbs cryo, pyro, hydro or
//! electro from an attack that reaches it or, every 0.3 s, from what is
//! around it, then fires Vivid Shots of that element), A1, A4 and every
//! constellation (gcsim leaves C1 out: it pulls enemies). No plunges or
//! held skill.

use crate::content::Character;
use crate::gcsimdata::chars::lynette as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The Vivid Shots' travel, from the burst's vivid_travel.
    vivid_travel: i32,
    /// The element the Box absorbed.
    absorbed: Option<Element>,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const ALIGNED_ICD: ModKey = ModKey::new(g::SKILL_ALIGNED_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const A1: ModKey = ModKey::new("lynette-a1");
const A4: ModKey = ModKey::new("lynette-a4");
const C6_INFUSION: ModKey = ModKey::new("lynette-c6-infusion");
const C6_BUFF: ModKey = ModKey::new("lynette-c6-buff");

const CB_PARTICLE: u16 = 1 << 0;
const CB_HEAL: u16 = 1 << 1;

// arg: the normal attack's counter times 2 plus its hit
const TASK_NORMAL: u8 = 0;
const TASK_SKILL: u8 = 1;
const TASK_C6: u8 = 2;
// arg: drains so far
const TASK_DRAIN: u8 = 3;
const TASK_SPAWN: u8 = 4;
const TASK_VIVID: u8 = 5;

const BOX: u8 = 0;
/// The Box, 1.8 in front of the player, of radius 1.
const BOX_Y: f64 = 1.8;

const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];

impl Character for Char {
    const KEY: &'static str = "lynette";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].cons >= 4 {
            let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
            cd.available = 2;
            cd.max_extra = 1;
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::Skill if p.get("hold").is_some_and(|h| h > 0) => Some("skill hold".into()),
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
        for (i, &h) in g::CHARGE_HITMARKS.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[i][lvl]);
            ai.abil = CHARGE_NAMES[i];
            ai.pattern = s.circle_on_player(0.0, 0.8, 2.2);
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARKS[g::CHARGE_HITMARKS.len() - 1], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL, arg: 0 }, g::SKILL_PRESS_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6, arg: 0 }, g::SKILL_PRESS_C6_START);
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD, g::SKILL_PRESS_CD_START);
        let t = &g::SKILL_PRESS_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("vivid_travel").map_or(15, |t| t as i32);
        s.add_task(Task::Char { c: ci as u8, id: TASK_SPAWN, arg: travel }, g::BURST_SPAWN);
        s.set_cd(ci, Action::Burst, g::BURST_CD);
        // A1, as the cooldown starts: ATK to the party for 10 s, by the
        // number of elements in it
        if s.chars[ci].ascension >= 1 {
            let mut eles: Vec<Element> = s.chars.iter().map(|c| c.element).collect();
            eles.sort_by_key(|e| *e as u8);
            eles.dedup();
            let v = 0.08 + (eles.len() - 1) as f64 * 0.04;
            for i in 0..s.chars.len() {
                s.add_stat_mod(i, A1, st::ATKP, v, 600, true);
            }
        }
        s.consume_energy(ci, g::BURST_ENERGY_DELAY);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
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
                let hb = g::ATTACK_HITBOXES[n];
                let oy = g::ATTACK_OFFSETS[n];
                ai.pattern = if n >= 1 { s.box_on_player(0.0, oy, hb[0], hb[1]) } else { s.circle_on_player(0.0, oy, hb[0]) };
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_SKILL => {
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL[lvl]);
                ai.abil = g::SKILL_ABIL;
                ai.hitlag_factor = 0.01;
                ai.def_halt = true;
                ai.pattern = s.box_on_player(0.0, -0.5, 1.8, 4.5);
                s.queue_attack(ai, CB_PARTICLE | CB_HEAL, 0, 0);
                // Surging Blade, once in 10 s
                if !s.mod_active(ci, ALIGNED_ICD) {
                    s.add_status(ci, ALIGNED_ICD, g::SKILL_ALIGNED_ICD, true);
                    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 0.0, g::SKILL_ALIGNED[lvl]);
                    ai.abil = "Surging Blade (lynette)";
                    ai.hitlag_factor = 0.01;
                    ai.def_halt = true;
                    ai.pattern = s.box_on_player(0.0, -0.3, 1.2, 4.5);
                    let h = g::SKILL_PRESS_ALIGNED_HITMARK - g::SKILL_PRESS_HITMARK;
                    s.queue_attack(ai, 0, h, h);
                }
            }
            // C6: anemo infusion and 20% anemo DMG for 6.4 s
            TASK_C6 => {
                if s.chars[ci].cons < 6 {
                    return;
                }
                let dur = ((6.0 + 0.4) * 60.0) as i32;
                s.add_infusion(ci, C6_INFUSION, Element::Anemo, dur, true, &[AttackTag::Normal, AttackTag::Extra, AttackTag::Plunge]);
                s.add_stat_mod(ci, C6_BUFF, st::ANEMOP, 0.2, dur, true);
            }
            TASK_DRAIN => {
                let count = arg + 1;
                if count == 4 {
                    return;
                }
                let amount = 0.06 * s.current_hp(ci);
                s.drain(ci, amount);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_DRAIN, arg: count }, 60);
            }
            TASK_SPAWN => spawn_box(s, ci, arg),
            TASK_VIVID => {
                let Some(ele) = s.st::<State>(ci).absorbed else { return };
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST_ANEMO, icd::GROUP_DEFAULT, ele, 25.0, g::VIVID[lvl]);
                ai.abil = "Vivid Shot";
                ai.pattern = s.circle_on_target(1.0);
                let n = if s.chars[ci].cons >= 2 { 2 } else { 1 };
                let travel = s.st::<State>(ci).vivid_travel;
                for _ in 0..n {
                    s.queue_attack(ai, 0, 0, travel);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD as i32, true);
            s.queue_particle(g::PARTICLE_COUNT as f64, Element::Anemo, s.particle_delay[ci]);
        }
        // the skill's hit heals her 25% of her max HP, then drains 6% of
        // her HP each second, three times
        if cbs & CB_HEAL != 0 {
            let src = 0.25 * s.max_hp(ci);
            let bonus = s.stat(ci, st::HEAL);
            s.heal(ci, ci, src, bonus);
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_DRAIN, arg: 0 }, 60);
        }
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        // A4: 15% burst DMG while the Box's conversion lasts
        if m.key == A4 && ev.info.tag == AttackTag::ElementalBurst {
            ev.stats[st::DMGP] += 0.15;
        }
    }

    /// Every 0.3 s the Box looks for cryo, pyro, hydro or electro around it.
    fn gadget_tick(s: &mut Sim, ci: usize, _: u8, timer: i32) {
        if timer % 18 != 0 || s.gadget_index(ci, BOX).is_none() {
            return;
        }
        let area = s.circle_on_player(0.0, BOX_Y, 0.48);
        let e = s.absorb_check_in(area, &[Element::Cryo, Element::Pyro, Element::Hydro, Element::Electro]);
        if e != Element::NoElement {
            // gcsim's gadget thinks before its duration counts this tick
            absorb(s, ci, e, 1);
        }
    }

    /// An attack of one of those elements with gauge that reaches the Box
    /// converts it. A fresh Box has no ICD of its own, and the first
    /// conversion removes it, so its ICD never holds an element back.
    fn gadget_hit(s: &mut Sim, ci: usize, _: u8, ev: &AttackEvent, lands: Option<bool>) {
        if ev.info.durability <= 0.0 || !matches!(ev.info.element, Element::Cryo | Element::Pyro | Element::Hydro | Element::Electro) {
            return;
        }
        match lands {
            Some(true) => absorb(s, ci, ev.info.element, 0),
            Some(false) => {}
            None => {
                let who = crate::gcsl::char_name(s.chars[ev.info.actor as usize].key);
                s.unsupported(&format!("hit pattern {who} {}", ev.info.abil));
            }
        }
    }
}

/// The Box appears: its first hit, its ticks for its whole duration, and
/// the burst's status.
fn spawn_box(s: &mut Sim, ci: usize, travel: i32) {
    s.add_hittable_gadget(ci, BOX, g::BURST_DURATION, 0.0, BOX_Y, 1.0);
    s.add_status(ci, BURST, g::BURST_DURATION, false);
    let st = s.st_mut::<State>(ci);
    st.vivid_travel = travel;
    st.absorbed = None;
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST[lvl]);
    ai.abil = "Magic Trick: Astonishing Shift";
    ai.pattern = s.circle_on_player(0.0, 1.5, 4.5);
    let h = g::BURST_HITMARK - g::BURST_SPAWN;
    s.queue_attack(ai, 0, h, h);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BOGGLECAT[lvl]);
    ai.abil = "Bogglecat Box";
    ai.pattern = s.circle_on_player(0.0, BOX_Y, 6.0);
    let mut t = g::BURST_FIRST_TICK - g::BURST_SPAWN;
    while t <= g::BURST_DURATION {
        s.queue_attack(ai, 0, t, t);
        t += g::BURST_TICK_INTERVAL;
    }
}

/// The Box converts: Vivid Shots every 134 frames for what remains of its
/// duration, A4 as long, and the Box is gone.
fn absorb(s: &mut Sim, ci: usize, ele: Element, extra: i32) {
    let Some(k) = s.gadget_index(ci, BOX) else { return };
    let left = s.gadgets[k].duration + extra;
    s.st_mut::<State>(ci).absorbed = Some(ele);
    let mut t = g::BURST_VIVID_INTERVAL;
    while t <= left {
        s.add_task(Task::Char { c: ci as u8, id: TASK_VIVID, arg: 0 }, t);
        t += g::BURST_VIVID_INTERVAL;
    }
    if s.chars[ci].ascension >= 4 {
        s.add_mod(ci, Mod::owned(A4, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1), left);
    }
    s.kill_gadget(ci, BOX);
}
