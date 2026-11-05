//! Prune, from gcsim's internal/characters/prune: normal and charged
//! attacks, the skill and its recast (Witch-tribution, of the element the
//! first hit swirled), the burst and its Witchlure Bell ticks, A1's hammer
//! on the bell's swirls, A4's Tolling Rally DMG bonus for the others, the
//! Hexerei bonuses (her ATK on a Hexerei character's reaction under Tolling
//! Rally, and that character's on its swirls), and C1 to C6. gcsim gives
//! her no plunges; they stay errors.
//!
//! C4's ricochet prefers another enemy in reach (RandomEnemyWithinArea
//! filtered to other enemies): with the one enemy there is none, no draw
//! is made, and it falls on the enemy the hammer hit.

use crate::content::{Character, Event};
use crate::gamedata::chars::prune as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The element the recast converts to (gcsim's skillConvertEle).
    convert_ele: Element,
    burst_src: i32,
    a4_dmg: f64,
    c2_atkp: f64,
    c6_tick_src: i32,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const RECAST: ModKey = ModKey::new(g::SKILL_RECAST_WINDOW_KEY);
const SKILL_SWIRL_ICD: ModKey = ModKey::new(g::SKILL_SWIRL_CHECK_ICD_KEY);
const A1_ICD: ModKey = ModKey::new(g::A1_SWIRL_CHECK_ICD_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const HEX_SELF: ModKey = ModKey::new("prune-hex-self-buff");
const HEX_TEAM: ModKey = ModKey::new("prune-hex-team-buff");
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C2: ModKey = ModKey::new("prune-c2");
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const C6_SELF: ModKey = ModKey::new("prune-c6-self-buff");
const C6_WINDOW: ModKey = ModKey::new(g::C6_WINDOW_KEY);
const C6_ACTIVE: ModKey = ModKey::new(g::C6_ACTIVE_BUFF_KEY);

const SKILL_ABIL: &str = "Ring-A-Ding-Ding! Hexhunter Chime DMG";
const BELL_ABIL: &str = "Witchlure Bell";

const CB_PARTICLE: u16 = 1 << 0;
const CB_A4: u16 = 1 << 1;
const CB_C1: u16 = 1 << 2;
const CB_C2: u16 = 1 << 3;
const CB_C4: u16 = 1 << 4;

const TASK_BURST_STATUS: u8 = 0;
const TASK_BURST_TICK: u8 = 1;
const TASK_C6_TICK: u8 = 2;

const FR_CHARGE: u8 = 0;
const FR_SKILL: u8 = 1;

const ATTACK_OFFSETS: [f64; 3] = [0.5, 0.5, 0.85];

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

/// The burst's duration with C6's 4 s.
fn burst_duration(s: &Sim, ci: usize) -> i32 {
    813 + if s.chars[ci].cons >= 6 { 4 * 60 } else { 0 }
}

impl Character for Char {
    const KEY: &'static str = "prune";
    const HEXEREI: bool = true;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { convert_ele: Element::NoElement, burst_src: -1, a4_dmg: 0.0, c2_atkp: 0.0, c6_tick_src: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        // the skill's swirl check, A1, the Hexerei bonuses and C6
        s.subscribe(EventKind::Reaction, Owner::Char(ci as u8));
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    /// NextQueueItemIsValid: a charged attack needs a normal attack before.
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if a == Action::Charge && s.chars[ci].normal_counter == 0 {
            return Some("need to use attack right before charge".into());
        }
        crate::content::template_invalid(s, a)
    }

    /// The recast is ready while its window lasts, whatever the cooldown.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && s.mod_active(ci, RECAST)).then_some(None)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.blunt = true;
        ai.poise = g::ATTACK_POISE_DMG[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = 0.03 * 60.0;
        ai.def_halt = true;
        ai.pattern = s.circle_toward_target(0.0, ATTACK_OFFSETS[n], g::ATTACK_RADIUS[n]);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let windup = if matches!(s.current_state(), AnimState::Idle | AnimState::Swap) { 0 } else { 10 };
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.blunt = true;
        ai.poise = 120.0;
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = 0.03 * 60.0;
        ai.def_halt = true;
        ai.pattern = s.box_on_player(0.0, 0.0, 3.0, 3.5);
        let h = g::CHARGE_HITMARK - windup;
        s.queue_attack(ai, 0, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: FR_CHARGE, arg: windup as i16 }, t[0], h, AnimState::Charge)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_CHARGE => g::CHARGE_FRAMES[next as usize] - arg as i32,
            _ => unreachable!(),
        }
    }

    /// The first press: a recast in its window cancels sooner.
    fn frames_live(s: &Sim, ci: usize, id: u8, _: i16, next: Action) -> i32 {
        match id {
            FR_SKILL if next == Action::Skill && s.mod_active(ci, RECAST) => 28,
            FR_SKILL => g::SKILL_FRAMES[next as usize],
            _ => unreachable!(),
        }
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, RECAST) {
            return skill_convert(s, ci);
        }
        st_mut(s, ci).convert_ele = Element::NoElement;
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL[lvl]);
        ai.abil = SKILL_ABIL;
        ai.hitlag_factor = 0.05;
        ai.hitlag_halt = 0.02 * 60.0;
        ai.pattern = s.circle_on_player(0.0, 1.4, 2.5);
        s.queue_attack(ai, CB_PARTICLE, 26, 26);
        s.set_cd_with_delay(ci, Action::Skill, 15 * 60, 25);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::CharLive { id: FR_SKILL, arg: 0 }, t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST[lvl]);
        ai.abil = "The Bell Tolls! The Hunt Is On!";
        ai.pattern = s.circle_on_player(0.0, 0.0, 6.5);
        s.queue_attack(ai, 0, 34, 34);
        // the status from 57 frames on, hitlag affected
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_STATUS, arg: 0 }, 57);
        let f = s.f;
        st_mut(s, ci).burst_src = f;
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_TICK, arg: f }, 137);
        s.set_cd(ci, Action::Burst, 18 * 60);
        s.consume_energy(ci, 18);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_BURST_STATUS => {
                let dur = burst_duration(s, ci) - 57;
                s.add_status(ci, BURST, dur, false);
                // C2: ATK +10%, +5% per hit of hers in the burst, to 40%
                if s.chars[ci].cons >= 2 {
                    st_mut(s, ci).c2_atkp = 0.10;
                    s.add_mod(ci, Mod::owned(C2, ModKind::Stat, Owner::Char(ci as u8), st::ATKP, false, true, -1), dur);
                }
            }
            TASK_BURST_TICK => {
                if arg != st(s, ci).burst_src || !s.mod_active(ci, BURST) {
                    return;
                }
                // the closest enemy within 10 of the player
                let area = s.circle_on_player(0.0, 0.0, 10.0);
                if !s.enemy_alive || !s.enemy_within(area) {
                    return;
                }
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST_DOT[lvl]);
                ai.abil = BELL_ABIL;
                ai.pattern = s.circle_on_target(1.6);
                s.queue_attack(ai, CB_C2, 0, 0);
                s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_TICK, arg }, 117);
            }
            TASK_C6_TICK => c6_tick(s, ci, arg),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && ev.on_enemy() && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, false);
            s.queue_particle(5.0, Element::Anemo, s.particle_delay[ci]);
        }
        // A4: DMG bonus for the others from her ATK above 2,000
        if cbs & CB_A4 != 0 && s.chars[ci].ascension >= 4 && ev.on_enemy() {
            let atk = total_atk(&s.select_stats(ci, [st::BASEATK, st::ATKP, st::ATK], true));
            st_mut(s, ci).a4_dmg = ((atk - 2000.0).max(0.0) * 0.00025).min(0.5);
            for i in 0..s.chars.len() {
                if i != ci {
                    s.add_mod(i, Mod::owned(A4, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 5 * 60);
                }
            }
        }
        // C1: energy
        if cbs & CB_C1 != 0 && s.chars[ci].cons >= 1 && ev.on_enemy() && !s.mod_active(ci, C1_ICD) {
            s.add_status(ci, C1_ICD, 108, false);
            s.add_energy(ci, 2.0);
        }
        // C2: the stack, in the burst
        if cbs & CB_C2 != 0 && s.chars[ci].cons >= 2 && ev.on_enemy() && s.mod_active(ci, BURST) {
            let x = st_mut(s, ci);
            x.c2_atkp = (x.c2_atkp + 0.05).min(0.40);
        }
        // C4: the ricochet, of the hit's element and tag, on its snapshot
        if cbs & CB_C4 != 0 && s.chars[ci].cons >= 4 && ev.on_enemy() && !s.mod_active(ci, C4_ICD) {
            s.add_status(ci, C4_ICD, 6, false);
            let mut ai = AttackInfo::new(ci, ev.info.tag, icd::TAG_NONE, icd::GROUP_DEFAULT, ev.info.element, 0.0, 0.8);
            ai.abil = "Banehunter Oathhammer Ricochet (C4)";
            ai.blunt = true;
            ai.poise = 15.0;
            ai.pattern = s.circle_on_target(1.5);
            let mut atk = AttackEvent::new(ai, 0);
            atk.stats = ev.stats;
            atk.char_lvl = ev.char_lvl;
            atk.snap_f = ev.snap_f;
            s.queue_attack_with_snap(atk, CB_A4 | CB_C1 | CB_C2, 63);
        }
    }

    /// A4: DMG of bursts, skills, normal, charged and plunging attacks.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A4 && matches!(ev.info.tag, AttackTag::ElementalBurst | AttackTag::ElementalArt | AttackTag::ElementalArtHold | AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            ev.stats[st::DMGP] += st(s, ci).a4_dmg;
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C2 {
            out[st::ATKP] += st(s, ci).c2_atkp;
            return true;
        }
        false
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        // every handler wants the reaction on the enemy: hyperbloom's and
        // burgeon's are on the dendro core
        if matches!(r, Reaction::Hyperbloom | Reaction::Burgeon) || !ev.on_enemy() {
            return;
        }
        let swirl = match r {
            Reaction::SwirlCryo | Reaction::StellarSwirl => Some(Element::Cryo),
            Reaction::SwirlElectro => Some(Element::Electro),
            Reaction::SwirlHydro => Some(Element::Hydro),
            Reaction::SwirlPyro => Some(Element::Pyro),
            _ => None,
        };
        let actor = ev.info.actor as usize;
        if let Some(ele) = swirl {
            if actor == ci {
                // the first press's swirl opens the recast
                if ev.info.abil == SKILL_ABIL && !s.mod_active(ci, SKILL_SWIRL_ICD) {
                    s.add_status(ci, SKILL_SWIRL_ICD, 12, false);
                    st_mut(s, ci).convert_ele = ele;
                    s.add_status(ci, RECAST, 364, true);
                }
                // A1: the bell's swirls bring down the hammer
                if s.chars[ci].ascension >= 1 && ev.info.abil == BELL_ABIL && !s.mod_active(ci, A1_ICD) {
                    s.add_status(ci, A1_ICD, 72, false);
                    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, ele, 0.0, 1.5);
                    ai.abil = "Banehunter Oathhammer (A1)";
                    ai.blunt = true;
                    ai.poise = 20.0;
                    ai.pattern = s.circle_on_target(2.3);
                    s.queue_attack(ai, CB_A4 | CB_C1 | CB_C4 | CB_C2, 45, 45);
                }
            }
        }
        let rally = s.mod_active(actor, A4);
        // Hexerei: a Hexerei character's reaction under Tolling Rally
        if s.chars[ci].hexerei && s.hexerei_count() >= 2 && s.chars[actor].hexerei && rally {
            s.add_stat_mod(ci, HEX_SELF, st::ATKP, 0.60, 5 * 60, true);
            if matches!(r, Reaction::SwirlCryo | Reaction::SwirlElectro | Reaction::SwirlHydro | Reaction::SwirlPyro) {
                s.add_stat_mod(actor, HEX_TEAM, st::ATKP, 0.30, 5 * 60, true);
            }
        }
        // C6: anyone's reaction under Tolling Rally
        if s.chars[ci].cons >= 6 && rally {
            s.add_stat_mod(ci, C6_SELF, st::ATK, 350.0, 330, true);
            s.add_status(ci, C6_WINDOW, 330, true);
            let x = st_mut(s, ci);
            x.c6_tick_src += 1;
            let src = x.c6_tick_src;
            c6_tick(s, ci, src);
        }
    }
}

/// The recast: the swirled element's hammer.
fn skill_convert(s: &mut Sim, ci: usize) -> ActInfo {
    let lvl = s.talent(ci, 1);
    let ele = st(s, ci).convert_ele;
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, ele, 25.0, g::SKILL_CONVERT[lvl]);
    ai.abil = "Clang Clang! Witch-tribution Comes! DMG";
    ai.blunt = true;
    ai.poise = 75.0;
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = 0.05 * 60.0;
    ai.def_halt = true;
    ai.pattern = s.circle_on_player(0.0, 1.5, 2.5);
    s.queue_attack(ai, CB_A4 | CB_C1 | CB_C2 | CB_C4, 30, 30);
    s.delete_mod(ci, RECAST);
    let t = &g::SKILL_CONVERT_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
}

/// C6: every 0.5 s of the window, ATK +350 on the active character under
/// Tolling Rally (not her).
fn c6_tick(s: &mut Sim, ci: usize, src: i32) {
    if src != st(s, ci).c6_tick_src {
        return;
    }
    let active = s.player.active as usize;
    let window = s.mod_active(ci, C6_WINDOW);
    for i in 0..s.chars.len() {
        if i != ci && i == active && window && s.mod_active(i, A4) {
            s.add_stat_mod(i, C6_ACTIVE, st::ATK, 350.0, -1, false);
        } else {
            s.delete_mod(i, C6_ACTIVE);
        }
    }
    if !window {
        return;
    }
    s.add_task(Task::Char { c: ci as u8, id: TASK_C6_TICK, arg: src }, 30);
}
