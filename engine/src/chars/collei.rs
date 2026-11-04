//! Collei, from gcsim's internal/characters/collei: normal attacks, aimed
//! shots, Floral Brush (the boomerang's two hits, A1's Sprout after it
//! returns), Trump-Card Kitty (the explosion and the leaps, A4's
//! extension by dendro reactions of a member in its zone) and every
//! constellation.
//!
//! A4's extensions are counted over the whole fight, never reset, as
//! gcsim counts them (three in all).

use crate::content::{Character, Event};
use crate::gcsimdata::chars::collei as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    burst_pos: (f64, f64),
    burst_extend_count: i32,
    sprout_should_extend: bool,
    sprout_should_proc: bool,
    sprout_src: i32,
    /// The skill's callbacks, once a cast between its two hits.
    c6_done: bool,
    particle_done: bool,
    /// Snapshots: the burst's at the explosion, the Sprout's at its start.
    burst_ev: AttackEvent,
    sprout_ev: AttackEvent,
}

const SKILL: ModKey = ModKey::new(g::SKILL_KEY);
const SPROUT: ModKey = ModKey::new(g::SPROUT_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const C1: ModKey = ModKey::new("collei-c1");
const C4: ModKey = ModKey::new("collei-c4");

const CB_C6: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;

/// The skill's state while the boomerang is out (the fight's queue).
const TASK_SKILL_STATUS: u8 = 0;
/// The boomerang returns: Sprout if a reaction procced it (the fight's
/// queue).
const TASK_RETURN: u8 = 1;
/// A tick of Sprout: arg is its start frame.
const TASK_SPROUT_TICK: u8 = 2;
/// The burst's zone forms (the fight's queue).
const TASK_BURST_START: u8 = 3;
const TASK_BURST_TICK: u8 = 4;
const TASK_A4_TICK: u8 = 5;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];

impl Character for Char {
    const KEY: &'static str = "collei";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        let ev = AttackEvent::new(AttackInfo::new(0, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_COLLEI_BURST, Element::Dendro, 25.0, 0.0), 0);
        State { burst_pos: (0.0, 0.0), burst_extend_count: 0, sprout_should_extend: false, sprout_should_proc: false, sprout_src: 0, c6_done: false, particle_done: false, burst_ev: ev, sprout_ev: ev }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // A1, A4 and C2 (which needs A1) watch dendro reactions
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::Reaction, o);
        }
        // C1: ER +20% off field
        if s.chars[ci].cons >= 1 {
            s.chars[ci].mods.push(Mod::owned(C1, ModKind::Stat, o, st::ER, false, false, -1));
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::Attack => p.0.iter().find(|(k, _)| k != "travel").map(|(k, _)| format!("attack param {k}")),
            Action::Aim => p.0.iter().find(|(k, _)| !matches!(k.as_str(), "hold" | "travel" | "weakspot")).map(|(k, _)| format!("aim param {k}")),
            Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn invalid(s: &Sim, _: usize, a: Action, p: &Params) -> Option<String> {
        if a == Action::Aim {
            if let Some(h) = p.get("hold") {
                if !(0..=1).contains(&h) {
                    return Some(format!("invalid hold param supplied, got {h}"));
                }
            }
        }
        crate::content::template_invalid(s, a)
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let h = g::ATTACK_HITMARKS[n];
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        s.queue_attack(ai, 0, h, h + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    /// Aimed shots: physical (hold 0) or fully charged (hold 1, the
    /// default), their hitlag on weak spot hits only.
    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::FULLAIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        ai.deployable = true;
        if p.get("weakspot") == Some(1) {
            ai.weak_point = true;
            ai.hitlag_halt = 0.12 * 60.0;
            ai.hitlag_factor = 0.01;
        }
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let h = g::AIMED_HITMARKS[hold];
        s.queue_attack(ai, 0, h, h + travel);
        let t = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Aim)
    }

    /// Floral Brush: two hits snapshot at the release; the state while
    /// the boomerang is out; Sprout when it returns.
    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::SKILL[lvl]);
        ai.abil = "Floral Brush";
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = s.circle_on_player(0.0, 0.0, 2.0);
        let mut cbs = CB_PARTICLE;
        if s.chars[ci].cons >= 6 {
            cbs |= CB_C6;
        }
        let st = s.st_mut::<State>(ci);
        st.c6_done = false;
        st.particle_done = false;
        for &h in g::SKILL_HITMARKS {
            s.queue_attack(ai, cbs, g::SKILL_RELEASE, h);
        }
        let c = ci as u8;
        s.add_task(Task::Char { c, id: TASK_SKILL_STATUS, arg: 0 }, g::SKILL_RELEASE);
        let proc = s.chars[ci].cons >= 2 && s.chars[ci].ascension >= 1;
        let st = s.st_mut::<State>(ci);
        st.sprout_should_extend = false;
        st.sprout_should_proc = proc;
        s.add_task(Task::Char { c, id: TASK_RETURN, arg: 0 }, g::SKILL_RETURN);
        s.set_cd_with_delay(ci, Action::Skill, 720, g::SKILL_RELEASE);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
    }

    /// Trump-Card Kitty: the explosion where the player is; the zone 25
    /// frames in (snapshot then), its leaps from 68 frames in, A4's checks.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_COLLEI_BURST, Element::Dendro, 25.0, g::BURST_EXPLOSION[lvl]);
        ai.abil = "Trump-Card Kitty (Explosion)";
        let pos = s.player_pos;
        s.st_mut::<State>(ci).burst_pos = pos;
        ai.pattern = Pattern::Circle { x: pos.0, y: pos.1, r: 5.5 };
        s.queue_attack(ai, 0, g::EXPLOSION_HITMARK, g::EXPLOSION_HITMARK);
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_START, arg: 0 }, g::EXPLOSION_HITMARK);
        // C4: EM +60 to the others for 12 s
        if s.chars[ci].cons >= 4 {
            for k in 0..s.chars.len() {
                if k != ci {
                    s.add_stat_mod(k, C4, st::EM, 60.0, 720, true);
                }
            }
        }
        s.set_cd(ci, Action::Burst, 900);
        s.consume_energy(ci, 7);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        let c = ci as u8;
        match id {
            TASK_SKILL_STATUS => s.add_status(ci, SKILL, g::SKILL_RETURN - g::SKILL_RELEASE, false),
            TASK_RETURN => {
                if !s.st::<State>(ci).sprout_should_proc {
                    return;
                }
                let src = s.f;
                let extend = s.st::<State>(ci).sprout_should_extend;
                s.st_mut::<State>(ci).sprout_src = src;
                s.add_status(ci, SPROUT, if extend { 360 } else { 180 }, true);
                let ev = s.snapshot_event(sprout_info(s, ci), 0);
                s.st_mut::<State>(ci).sprout_ev = ev;
                s.queue_char_task(ci, Task::Char { c, id: TASK_SPROUT_TICK, arg: src }, g::SPROUT_HITMARK);
            }
            TASK_SPROUT_TICK => {
                if !s.mod_active(ci, SPROUT) || arg != s.st::<State>(ci).sprout_src {
                    return;
                }
                let mut ev = s.st::<State>(ci).sprout_ev;
                ev.info.pattern = s.circle_on_player(0.0, 0.0, 2.0);
                s.queue_attack_with_snap(ev, 0, 0);
                s.add_task(Task::Char { c, id: TASK_SPROUT_TICK, arg }, g::SPROUT_TICK_PERIOD);
            }
            TASK_BURST_START => {
                s.add_status(ci, BURST, 378, false);
                let lvl = s.talent(ci, 2);
                let ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_COLLEI_BURST, Element::Dendro, 25.0, g::BURST_EXPLOSION[lvl]);
                let ev = s.snapshot_event(AttackInfo { abil: "Trump-Card Kitty (Explosion)", ..ai }, 0);
                s.st_mut::<State>(ci).burst_ev = ev;
                s.add_task(Task::Char { c, id: TASK_BURST_TICK, arg: 0 }, g::LEAP_HITMARK - g::EXPLOSION_HITMARK);
                a4_tick(s, ci);
            }
            TASK_BURST_TICK => {
                if !s.mod_active(ci, BURST) {
                    return;
                }
                let lvl = s.talent(ci, 2);
                let (x, y) = s.st::<State>(ci).burst_pos;
                let mut ev = s.st::<State>(ci).burst_ev;
                ev.info.abil = "Trump-Card Kitty (Leap)";
                ev.info.mult = g::BURST_LEAP[lvl];
                ev.info.pattern = Pattern::Circle { x, y, r: 4.0 };
                s.queue_attack_with_snap(ev, 0, 0);
                s.add_task(Task::Char { c, id: TASK_BURST_TICK, arg: 0 }, g::LEAP_TICK_PERIOD);
            }
            TASK_A4_TICK => a4_tick(s, ci),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        // C6: once a cast, a dendro area of radius 4 on the enemy hit
        if cbs & CB_C6 != 0 && !s.st::<State>(ci).c6_done {
            s.st_mut::<State>(ci).c6_done = true;
            let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, 2.0);
            ai.abil = "Forest of Falling Arrows (C6)";
            ai.pattern = s.circle_on_target(4.0);
            s.queue_attack(ai, 0, 0, 22);
        }
        if cbs & CB_PARTICLE != 0 && !s.st::<State>(ci).particle_done {
            s.st_mut::<State>(ci).particle_done = true;
            s.queue_particle(3.0, Element::Dendro, s.particle_delay[ci]);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C1 && s.player.active as usize != ci {
            out[st::ER] += 0.2;
            return true;
        }
        false
    }

    /// Burning, Quicken, Aggravate, Spread, Bloom, Hyperbloom, Burgeon and
    /// Lunar-Bloom (on the enemy, but Hyperbloom and Burgeon from
    /// anywhere): A1's proc, A4's extension and C2's, in that order.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        let anywhere = matches!(r, Reaction::Hyperbloom | Reaction::Burgeon);
        if !anywhere && !(matches!(r, Reaction::Burning | Reaction::Quicken | Reaction::Aggravate | Reaction::Spread | Reaction::Bloom | Reaction::LunarBloom) && ev.on_enemy()) {
            return;
        }
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // A1: a reaction while the boomerang is out procs Sprout
        if asc >= 1 && !s.st::<State>(ci).sprout_should_proc && s.mod_active(ci, SKILL) {
            s.st_mut::<State>(ci).sprout_should_proc = true;
        }
        // A4: a member in the zone extends it by 1 s, three times in all
        if asc >= 4 && s.mod_active(ci, BURST) && s.mod_active(ev.info.actor as usize, A4) && s.st::<State>(ci).burst_extend_count < 3 {
            s.extend_mod(ci, BURST, 60);
            s.st_mut::<State>(ci).burst_extend_count += 1;
        }
        // C2: Sprout lasts 3 s more
        if cons >= 2 && asc >= 1 && !s.st::<State>(ci).sprout_should_extend && (s.mod_active(ci, SPROUT) || s.mod_active(ci, SKILL)) {
            s.st_mut::<State>(ci).sprout_should_extend = true;
            if s.mod_active(ci, SPROUT) {
                s.extend_mod(ci, SPROUT, 180);
            }
        }
    }
}

fn sprout_info(s: &Sim, ci: usize) -> AttackInfo {
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_COLLEI_SPROUT, icd::GROUP_DEFAULT, Element::Dendro, 25.0, 0.4);
    ai.abil = "Floral Sidewinder (A1)";
    ai.pattern = s.circle_on_player(0.0, 0.0, 2.0);
    ai
}

/// A4: the active member in the zone (radius 6) is marked for 1 s, every
/// 30 frames while it lasts.
fn a4_tick(s: &mut Sim, ci: usize) {
    if !s.mod_active(ci, BURST) {
        return;
    }
    let (x, y) = s.st::<State>(ci).burst_pos;
    if !s.player_within(Pattern::Circle { x, y, r: 6.0 }) {
        return;
    }
    let active = s.player.active as usize;
    s.add_status(active, A4, 60, true);
    s.add_task(Task::Char { c: ci as u8, id: TASK_A4_TICK, arg: 0 }, 30);
}
