//! Sayu, from gcsim's internal/characters/sayu: normal attacks, the skill
//! pressed, short-held or held (with its absorbed element on the roll and
//! the kick), the burst's hit, heal and Muji-Muji Daruma, A1's heal on her
//! swirls, A4's wider Daruma, and every constellation (C1's Daruma that
//! heals and attacks at once, C2's kick bonus, C4's energy, C6's EM
//! scaling). gcsim gives her no charged attack and no plunges.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::sayu as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The frame up to which the absorbed element adds hits (gcsim's
    /// eDuration).
    e_duration: i32,
    /// The absorbed element (NoElement before one is) and its ICD tag.
    absorb: Element,
    absorb_tag: IcdTag,
    /// Where the hold checks for an element: a circle of 1.2 on the player
    /// as the skill was used.
    absorb_area: Pattern,
    /// C2's DMG bonus for the kick.
    c2_bonus: f64,
    /// The hold's DoT, snapshot as the skill is used.
    roll: AttackEvent,
    /// The burst's Daruma: its attack, snapshot as the burst is used, its
    /// heal, and its area (a circle of 10, 1.5 in front of the player).
    daruma: AttackEvent,
    daruma_heal: f64,
    burst_area: Pattern,
}

const A1_ICD: ModKey = ModKey::new(g::A1_ICD_KEY);
const KICK_PARTICLE_ICD: ModKey = ModKey::new(g::KICK_PARTICLE_ICD_KEY);
const ROLL_PARTICLE_ICD: ModKey = ModKey::new(g::ROLL_PARTICLE_ICD_KEY);
const C2: ModKey = ModKey::new("sayu-c2");

const CB_KICK_PARTICLE: u16 = 1 << 0;
const CB_ROLL_PARTICLE: u16 = 1 << 1;

// arg: the checks done so far, and (above 16 bits) how many to do
const TASK_ABSORB: u8 = 0;
const TASK_ROLL_TICK: u8 = 1;
const TASK_BURST_TICKS: u8 = 2;
const TASK_DARUMA: u8 = 3;

fn blank() -> AttackEvent {
    AttackEvent::new(AttackInfo::new(0, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 0.0, 0.0), 0)
}

fn kick(s: &mut Sim, ci: usize, abil: &'static str, mult: f64, r: f64, delay: i32) {
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, mult);
    ai.abil = abil;
    ai.hitlag_halt = 0.02 * 60.0;
    ai.hitlag_factor = 0.05;
    ai.pattern = s.circle_on_player(0.0, 0.5, r);
    let ev = s.snapshot_event(ai, 0);
    s.queue_attack_with_snap(ev, CB_KICK_PARTICLE, delay);
}

/// The start of a short hold or a hold: the absorb check's state and the
/// DoT's snapshot.
fn hold_start(s: &mut Sim, ci: usize, e_duration: i32, checks: i32) {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArtHold, icd::TAG_ELEMENTAL_ART_ANEMO, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL_PRESS[lvl]);
    ai.abil = "Fuufuu Windwheel (DoT Hold)";
    ai.hitlag_halt = 0.01 * 60.0;
    ai.hitlag_factor = 0.05;
    ai.deployable = true;
    let roll = s.snapshot_event(ai, CB_ROLL_PARTICLE);
    let area = s.circle_on_player(0.0, 0.0, 1.2);
    let st = s.st_mut::<State>(ci);
    st.e_duration = e_duration;
    st.c2_bonus = 0.0;
    st.absorb = Element::NoElement;
    st.absorb_tag = icd::TAG_NONE;
    st.absorb_area = area;
    st.roll = roll;
    s.add_task(Task::Char { c: ci as u8, id: TASK_ABSORB, arg: checks << 16 }, 18 + 12);
}

impl Character for Char {
    const KEY: &'static str = "sayu";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { e_duration: -1, absorb: Element::NoElement, absorb_tag: icd::TAG_NONE, absorb_area: Pattern::Unknown, c2_bonus: 0.0, roll: blank(), daruma: blank(), daruma_heal: 0.0, burst_area: Pattern::Unknown }
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::Reaction, Owner::Char(ci as u8));
        }
        s.subscribe(EventKind::EnemyHit, Owner::Char(ci as u8));
        if s.chars[ci].cons >= 2 {
            s.add_mod(ci, Mod::owned(C2, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1), -1);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Skill => p.0.iter().find(|(k, _)| k != "hold" && k != "short_hold").map(|(k, _)| format!("{name} param {k}")),
            Action::Attack | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let hits = g::ATTACK_HITMARKS[n];
        for (i, &h) in hits.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.blunt = true;
            ai.poise = g::ATTACK_POISE_DMG[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let (off, hb) = (g::ATTACK_OFFSETS[n][i], g::ATTACK_HITBOXES[n][i]);
            ai.pattern = if n == 0 || (n == 2 && i == 0) || n == 3 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
            s.queue_attack_at(ci, ai, 0, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hits[hits.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn frames(_: u8, duration: i16, next: Action) -> i32 {
        g::SKILL_HOLD_FRAMES[next as usize] - 600 + duration as i32
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        if p.get("short_hold").unwrap_or(0) != 0 {
            let f = s.f;
            hold_start(s, ci, f + g::SKILL_SHORT_HOLD_KICK_HITMARK, 1);
            s.add_task(Task::Char { c: ci as u8, id: TASK_ROLL_TICK, arg: 0 }, 18);
            kick(s, ci, "Fuufuu Whirlwind (Kick Hold)", g::SKILL_HOLD_END[lvl], 3.0, g::SKILL_SHORT_HOLD_KICK_HITMARK);
            s.set_cd_with_delay(ci, Action::Skill, 372, g::SKILL_SHORT_HOLD_CD_START);
            let t = &g::SKILL_SHORT_HOLD_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill);
        }
        let hold = p.get("hold").unwrap_or(0);
        if hold > 0 {
            let d = hold.min(600) as i32;
            let f = s.f;
            hold_start(s, ci, f + (g::SKILL_HOLD_KICK_HITMARK - 600) + d, d / 12);
            let mut i = 0;
            while i <= d {
                s.add_task(Task::Char { c: ci as u8, id: TASK_ROLL_TICK, arg: 0 }, 18 + i);
                i += 30;
            }
            kick(s, ci, "Fuufuu Whirlwind (Kick Hold)", g::SKILL_HOLD_END[lvl], 3.0, (g::SKILL_HOLD_KICK_HITMARK - 600) + d);
            // +2 frames so that the absorbed hit does not proc Sacrificial
            s.set_cd_with_delay(ci, Action::Skill, (360.0 + d as f64 * 0.5) as i32, (g::SKILL_HOLD_CD_START - 600) + d + 2);
            let t = &g::SKILL_HOLD_FRAMES;
            return ActInfo::new(Frames::Char { id: 0, arg: d as i16 }, t[0] - 600 + d, t[Action::Swap as usize] - 600 + d, AnimState::Skill);
        }
        // press
        s.st_mut::<State>(ci).c2_bonus = 0.033;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArtHold, icd::TAG_ELEMENTAL_ART_ANEMO, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL_PRESS[lvl]);
        ai.abil = "Fuufuu Windwheel (DoT Press)";
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
        let ev = s.snapshot_event(ai, 0);
        s.queue_attack_with_snap(ev, 0, g::SKILL_PRESS_DO_T_HITMARK);
        kick(s, ci, "Fuufuu Whirlwind (Kick Press)", g::SKILL_PRESS_END[lvl], 2.5, g::SKILL_PRESS_KICK_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 360, g::SKILL_PRESS_CD_START);
        let t = &g::SKILL_PRESS_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST[lvl]);
        ai.abil = "Yoohoo Art: Mujina Flurry";
        ai.hitlag_factor = 0.05;
        ai.hitlag_halt = 0.02 * 60.0;
        let area = s.circle_on_player(0.0, 1.5, 10.0);
        let Pattern::Circle { x, y, .. } = area else { unreachable!() };
        ai.pattern = Pattern::Circle { x, y, r: 4.5 };
        let ev = s.snapshot_event(ai, 0);
        s.queue_attack_with_snap(ev, 0, g::BURST_HITMARK);
        let heal = g::INIT_HEAL_FLAT[lvl] + total_atk(&ev.stats) * g::INIT_HEAL_PP[lvl];
        s.heal_info(ci, None, false, heal, ev.stats[st::HEAL]);
        // the Daruma, snapshot now
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST_SKILL[lvl]);
        ai.abil = "Muji-Muji Daruma";
        let mut d = s.snapshot_event(ai, 0);
        let atk = total_atk(&d.stats);
        let mut heal = g::BURST_HEAL_FLAT[lvl] + atk * g::BURST_HEAL_PP[lvl];
        // C6: DMG and healing from her EM
        if s.chars[ci].cons >= 6 {
            d.info.flat_dmg += atk * (d.stats[st::EM] * 0.002).min(4.0);
            heal += (d.stats[st::EM] * 3.0).min(6000.0);
        }
        let st = s.st_mut::<State>(ci);
        st.daruma = d;
        st.daruma_heal = heal;
        st.burst_area = area;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_TICKS, arg: 0 }, g::TICK_TASK_DELAY);
        s.set_cd(ci, Action::Burst, 1200);
        s.consume_energy(ci, 7);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_ABSORB => {
                let (count, max) = (arg & 0xffff, arg >> 16);
                if count == max {
                    return;
                }
                let area = s.st::<State>(ci).absorb_area;
                let e = s.absorb_check_in(area, &[Element::Pyro, Element::Hydro, Element::Electro, Element::Cryo]);
                s.st_mut::<State>(ci).absorb = e;
                if e != Element::NoElement {
                    s.st_mut::<State>(ci).absorb_tag = match e {
                        Element::Pyro => icd::TAG_ELEMENTAL_ART_PYRO,
                        Element::Hydro => icd::TAG_ELEMENTAL_ART_HYDRO,
                        Element::Electro => icd::TAG_ELEMENTAL_ART_ELECTRO,
                        _ => icd::TAG_ELEMENTAL_ART_CRYO,
                    };
                    return;
                }
                s.add_task(Task::Char { c: ci as u8, id: TASK_ABSORB, arg: arg + 1 }, 12);
            }
            TASK_ROLL_TICK => {
                // the DoT follows the player
                let mut d = s.st::<State>(ci).roll;
                d.info.pattern = s.circle_on_player(0.0, 0.0, 3.0);
                s.queue_attack_with_snap(d, CB_ROLL_PARTICLE, 0);
                let c2 = s.chars[ci].cons >= 2;
                let st = s.st_mut::<State>(ci);
                if c2 && st.c2_bonus < 0.66 {
                    st.c2_bonus += 0.033;
                }
            }
            TASK_BURST_TICKS => {
                // the first tick at 145
                let mut i = 145 - g::TICK_TASK_DELAY;
                while i < 145 + 540 - g::TICK_TASK_DELAY {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_DARUMA, arg: 0 }, i);
                    i += 90;
                }
            }
            TASK_DARUMA => {
                let active = s.player.active as usize;
                let c1 = s.chars[ci].cons >= 1;
                let area = s.st::<State>(ci).burst_area;
                // C1 ignores the HP limit, and attacks and heals at once
                let need_heal = s.player_within(area) && (s.chars[active].hp_ratio <= 0.7 || c1);
                let enemy = s.enemy_alive && s.enemy_within(area);
                if enemy && (!need_heal || c1) {
                    let r = if s.chars[ci].ascension >= 4 { 3.5 } else { 1.0 };
                    let mut d = s.st::<State>(ci).daruma;
                    d.info.pattern = s.circle_on_target(r);
                    s.queue_attack_with_snap(d, 0, 0);
                }
                if need_heal {
                    let st = s.st::<State>(ci);
                    let (heal, bonus) = (st.daruma_heal, st.daruma.stats[st::HEAL]);
                    s.heal(ci, active, heal, bonus);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_KICK_PARTICLE != 0 && !s.mod_active(ci, KICK_PARTICLE_ICD) {
            s.add_status(ci, KICK_PARTICLE_ICD, 30, true);
            s.queue_particle(2.0, Element::Anemo, s.particle_delay[ci]);
        }
        if cbs & CB_ROLL_PARTICLE != 0 && !s.mod_active(ci, ROLL_PARTICLE_ICD) {
            s.add_status(ci, ROLL_PARTICLE_ICD, 180, true);
            s.queue_particle(1.0, Element::Anemo, s.particle_delay[ci]);
        }
    }

    /// C2: the kick's DMG bonus.
    fn attack_mod(s: &Sim, ci: usize, on: usize, _: &Mod, ev: &mut AttackEvent) {
        if on == ci && ev.info.tag == AttackTag::ElementalArt {
            ev.stats[st::DMGP] += s.st::<State>(ci).c2_bonus;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // the absorbed element's hits, on the roll's and the kick's hits
            Event::EnemyHit(ev) => {
                if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
                    return;
                }
                let st = s.st::<State>(ci);
                if ev.info.element != Element::Anemo || st.absorb == Element::NoElement || s.f > st.e_duration {
                    return;
                }
                let lvl = s.talent(ci, 1);
                let mut ai = if ev.info.tag == AttackTag::ElementalArtHold {
                    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArtHold, st.absorb_tag, icd::GROUP_DEFAULT, st.absorb, 25.0, g::SKILL_ABSORB[lvl]);
                    ai.abil = "Fuufuu Windwheel Elemental (Elemental DoT Hold)";
                    ai
                } else {
                    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, st.absorb, 25.0, g::SKILL_ABSORB_END[lvl]);
                    ai.abil = "Fuufuu Whirlwind Elemental (Elemental Kick Hold)";
                    ai
                };
                ai.pattern = Pattern::Single;
                s.queue_attack(ai, 0, 1, 1);
            }
            // A1: a heal for the party on her swirl while on the field,
            // once every 2 s (C4: and energy)
            Event::Reaction(r, ev) => {
                if !matches!(r, Reaction::SwirlCryo | Reaction::StellarSwirl | Reaction::SwirlElectro | Reaction::SwirlHydro | Reaction::SwirlPyro) {
                    return;
                }
                if !ev.on_enemy() || ev.info.actor as usize != ci || s.player.active as usize != ci || s.mod_active(ci, A1_ICD) {
                    return;
                }
                s.add_status(ci, A1_ICD, 120, true);
                if s.chars[ci].cons >= 4 {
                    s.add_energy(ci, 1.2);
                }
                let heal = 300.0 + s.stat(ci, st::EM) * 1.2;
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, None, false, heal, bonus);
            }
            _ => {}
        }
    }
}
