//! The dendro traveler (aetherdendro, luminedendro), from gcsim's
//! internal/characters/traveler/common/dendro: normal and charged attacks,
//! the skill with C1's energy, the burst's Lea Lotus Lamp (its ticks, its
//! transfiguration by electro, hydro or pyro on it, and its explosion),
//! A1's Overflowing Lotuslight, A4's DMG bonus from EM, C2's duration, C4's
//! stacks, C6's DMG bonus, and the story-quest buffs.
//!
//! The lamp is a gadget of the enemy camp with a dendro aura of its own
//! (gcsim's LeaLotus, a Reactable): the attacks that reach it go through
//! its own ICD, and electro (quicken), pyro (burning) or hydro (bloom, which
//! makes a dendro core around it) transfigure it, emitting the reaction on
//! the gadget, after which it is gone. Without cryo on it, the other
//! reactions its React tries (aggravate, melt, superconduct, freeze,
//! swirls, crystallize, spread) find nothing: a cryo hit on the lamp, which
//! gcsim attaches to it, is an error that names it.

use crate::chars::traveler::{self, Normals};
use crate::chars::CharState;
use crate::content::{Character, Event, Owner};
use crate::gamedata::chars::traveler_common_dendro as g;
use crate::sim::*;
use crate::types::*;

pub struct Traveler<const G: usize>;

/// Aether's and Lumine's states are distinct types holding the same.
pub struct State<const G: usize>(Inner);

pub struct Inner {
    story: crate::chars::traveler_common::Story,
    /// gcsim's burstPos, burstRadius, burstOverflowingLotuslight,
    /// burstTransfig and skillC1.
    burst_pos: (f64, f64),
    burst_radius: f64,
    stacks: i32,
    transfig: Option<Element>,
    skill_c1: bool,
    /// The lamp: its hitbox radius, its ticks' attack (snapshot as it
    /// appears; the explosion changes its info), the frame it expires,
    /// the gadget tick its think started at (-1 before), and its own ICD
    /// by actor, tag and group (the frame the counter resets, the count).
    hitbox_radius: f64,
    lamp_atk: AttackEvent,
    lamp_end: i32,
    think_from: i32,
    icd: Vec<(u8, IcdTag, IcdGroup, i32, usize)>,
}

const N: Normals = crate::traveler_normals!(g);

const LAMP: u8 = 0;
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const A4: ModKey = ModKey::new("dmc-a4");
const C6: ModKey = ModKey::new("dmc-c6");

const CB_PARTICLE: u16 = 1 << 0;
const CB_C1: u16 = 1 << 1;

const TASK_LAMP: u8 = 0;
const TASK_FIRST_TICK: u8 = 1;
const TASK_A1_STACK: u8 = 2;
const TASK_A1_BUFF: u8 = 3;
const TASK_C6_BUFF: u8 = 4;
// arg: the attack slot of the bloom
const TASK_CORE: u8 = 5;
const TASK_EXPLODE: u8 = 6;
// arg: the delay of the tick (0 for the lamp's think)
const TASK_TICK: u8 = 7;

fn st(s: &Sim, ci: usize) -> &Inner {
    match &s.chars[ci].state {
        CharState::Aetherdendro(x) => &x.0,
        CharState::Luminedendro(x) => &x.0,
        _ => unreachable!(),
    }
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut Inner {
    match &mut s.chars[ci].state {
        CharState::Aetherdendro(x) => &mut x.0,
        CharState::Luminedendro(x) => &mut x.0,
        _ => unreachable!(),
    }
}

fn task(s: &mut Sim, ci: usize, id: u8, arg: i32, delay: i32) {
    s.add_task(Task::Char { c: ci as u8, id, arg }, delay);
}

impl<const G: usize> Character for Traveler<G> {
    const KEY: &'static str = if G == 0 { "aetherdendro" } else { "luminedendro" };
    type State = State<G>;

    fn new_state(cs: &CharSetup) -> State<G> {
        let lamp_atk = AttackEvent::new(AttackInfo::new(0, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 0.0, 0.0), 0);
        State(Inner { story: crate::chars::traveler_common::Story::new(cs), burst_pos: (0.0, 0.0), burst_radius: 8.0, stacks: 0, transfig: None, skill_c1: false, hitbox_radius: 2.0, lamp_atk, lamp_end: 0, think_from: -1, icd: Vec::new() })
    }

    fn init(s: &mut Sim, ci: usize) {
        crate::chars::traveler_common::story_buffs(s, ci);
        let o = Owner::Char(ci as u8);
        // A1 and C6 lose their buffs on the character swapped out
        if s.chars[ci].ascension >= 1 || s.chars[ci].cons >= 6 {
            s.subscribe(EventKind::Swap, o);
        }
        if s.chars[ci].ascension >= 4 {
            s.add_mod(ci, Mod::owned(A4, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        traveler::unsupported(a)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        traveler::attack(s, ci, G, &N, |_| {})
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        traveler::charge(s, ci, G, &N)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::SKILL[lvl]);
        ai.abil = "Razorgrass Blade";
        ai.pattern = s.fan_on_player(0.0, -0.3, 6.5, 130.0);
        let mut cbs = CB_PARTICLE;
        if s.chars[ci].cons >= 1 {
            st_mut(s, ci).skill_c1 = true;
            cbs |= CB_C1;
        }
        s.queue_attack(ai, cbs, g::SKILL_HITMARK, g::SKILL_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 8 * 60, g::CD_START);
        let t = &g::SKILL_FRAMES[G];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.set_cd(ci, Action::Burst, 1200);
        s.consume_energy(ci, 2);
        task(s, ci, TASK_LAMP, 0, g::LEA_LOTUS_APPEAR);
        let t = &g::BURST_FRAMES[G];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_LAMP => new_lamp(s, ci),
            // 37 frames after it appears: its first tick, then one every
            // 90 frames (gcsim's OnThinkInterval, set here)
            TASK_FIRST_TICK => {
                let Some(k) = s.gadget_index(ci, LAMP) else { return };
                if s.gadgets[k].src != arg {
                    return;
                }
                lamp_tick(s, ci, 0);
                st_mut(s, ci).think_from = s.gadgets[k].timer;
            }
            TASK_TICK => lamp_tick(s, ci, 0),
            // A1: a stack each second the burst lasts, up to 10
            TASK_A1_STACK => {
                if s.core_status_key(BURST) > 0 && st(s, ci).stacks < 10 {
                    st_mut(s, ci).stacks += 1;
                }
            }
            // A1 and C6: every 0.3 s, the active character inside the
            // lamp's area gets them for 1 s
            TASK_A1_BUFF | TASK_C6_BUFF => {
                if s.core_status_key(BURST) <= 0 {
                    return;
                }
                let t = st(s, ci);
                let area = Pattern::Circle { x: t.burst_pos.0, y: t.burst_pos.1, r: t.burst_radius };
                if !s.player_within(area) {
                    return;
                }
                let active = s.player.active as usize;
                if id == TASK_A1_BUFF {
                    let em = (6 * st(s, ci).stacks) as f64;
                    s.add_stat_mod(active, A1, st::EM, em, 60, true);
                } else {
                    s.add_mod(active, Mod::owned(C6, ModKind::Stat, Owner::Char(ci as u8), st::NONE, false, true, -1), 60);
                }
            }
            TASK_CORE => {
                let (x, y) = st(s, ci).burst_pos;
                s.spawn_core_around(arg as u16, (x, y), 1.0);
            }
            // pyro's explosion, a circle of 6.5 on the lamp, a second after
            // the burning; the burst's status ends
            TASK_EXPLODE => {
                let (x, y) = st(s, ci).burst_pos;
                let mut ev = st(s, ci).lamp_atk;
                ev.info.pattern = Pattern::Circle { x, y, r: 6.5 };
                s.queue_attack_with_snap(ev, 0, 0);
                s.delete_core_status(BURST);
            }
            _ => unreachable!(),
        }
    }

    fn gadget_tick(s: &mut Sim, ci: usize, _: u8, timer: i32) {
        let from = st(s, ci).think_from;
        if from >= 0 && timer > from && (timer - from) % 90 == 0 && s.gadget_index(ci, LAMP).is_some() {
            lamp_tick(s, ci, 0);
        }
    }

    fn gadget_hit(s: &mut Sim, ci: usize, _: u8, ev: &AttackEvent, lands: Option<bool>) {
        match lands {
            Some(true) => {}
            Some(false) => return,
            None => {
                let who = crate::gcsl::char_name(s.chars[ev.info.actor as usize].key);
                s.unsupported(&format!("hit pattern {who} {}", ev.info.abil));
                return;
            }
        }
        let mut ev = *ev;
        if ev.info.durability <= 0.0 {
            return;
        }
        ev.info.durability *= lamp_will_apply(s, ci, &ev.info);
        if ev.info.durability <= 0.0 || ev.info.element == Element::Physical {
            return;
        }
        ev.on_gadget = true;
        match ev.info.element {
            Element::Electro => {
                // quicken: the lamp's ticks every 54 frames from 15 for
                // the time it has left
                ev.reacted = true;
                s.emit(Event::Reaction(Reaction::Quicken, &ev));
                let left = st(s, ci).lamp_end - s.f;
                let mut t = 15;
                while t <= left {
                    lamp_tick(s, ci, t);
                    t += 54;
                }
                transfig(s, ci, Element::Electro);
            }
            Element::Pyro => {
                ev.reacted = true;
                s.emit(Event::Reaction(Reaction::Burning, &ev));
                let lvl = s.talent(ci, 2);
                let a = &mut st_mut(s, ci).lamp_atk.info;
                a.abil = g::BURST_EXPLOSION_ABIL;
                a.durability = 50.0;
                a.icd_tag = icd::TAG_NONE;
                a.mult = g::BURST_EXPLODE[lvl];
                task(s, ci, TASK_EXPLODE, 0, 60);
                transfig(s, ci, Element::Pyro);
            }
            Element::Hydro => {
                // bloom: a dendro core around the lamp, and wider ticks
                // every 90 frames from 15 for the time it has left
                ev.reacted = true;
                let kind = s.bloom_kind();
                let slot = s.alloc(ev);
                task(s, ci, TASK_CORE, slot as i32, 30);
                s.emit(Event::Reaction(kind, &ev));
                let t = st_mut(s, ci);
                t.burst_radius = 12.0;
                t.hitbox_radius = 4.0;
                let left = t.lamp_end - s.f;
                let mut t = 15;
                while t <= left {
                    lamp_tick(s, ci, t);
                    t += 90;
                }
                transfig(s, ci, Element::Hydro);
            }
            Element::Cryo => {
                let who = crate::gcsl::char_name(s.chars[ci].key);
                s.unsupported(&format!("cryo on {who}'s Lea Lotus Lamp"));
            }
            // dendro, anemo and geo find nothing on a dendro aura alone
            _ => {}
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let t = st(s, ci);
        if crate::chars::traveler_common::story_amount(&t.story, m, out) {
            return true;
        }
        if m.key != C6 {
            return false;
        }
        // C6: 12% dendro DMG, and 12% of the element the lamp took
        out[st::DENDROP] += 0.12;
        match t.transfig {
            Some(Element::Hydro) => out[st::HYDROP] += 0.12,
            Some(Element::Electro) => out[st::ELECTROP] += 0.12,
            Some(Element::Pyro) => out[st::PYROP] += 0.12,
            _ => {}
        }
        true
    }

    /// A4: 0.15% skill DMG and 0.1% burst DMG per point of EM.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        match ev.info.tag {
            AttackTag::ElementalArt => ev.stats[st::DMGP] += s.stat(ci, st::EM) * 0.0015,
            AttackTag::ElementalBurst => ev.stats[st::DMGP] += s.stat(ci, st::EM) * 0.001,
            _ => {}
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if !ev.on_enemy() {
            return;
        }
        if cbs & CB_C1 != 0 && st(s, ci).skill_c1 {
            s.add_energy(ci, 3.5);
            st_mut(s, ci).skill_c1 = false;
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, true);
            let n = if s.rng.float64() < 0.5 { 3.0 } else { 2.0 };
            s.queue_particle(n, Element::Dendro, s.particle_delay[ci]);
        }
    }

    fn event(s: &mut Sim, _: usize, e: &Event) {
        let Event::Swap { prev, .. } = *e else { return };
        s.delete_mod(prev, A1);
        s.delete_mod(prev, C6);
    }
}

/// gcsim's newLeaLotusLamp and the rest of the burst's task: the lamp 1
/// ahead of the player, its status, its first tick 37 frames on, A1's and
/// C6's tasks for its whole duration.
fn new_lamp(s: &mut Sim, ci: usize) {
    let pos = s.point_from_player(0.0, 1.0);
    let mut dur = 12 * 60;
    if s.chars[ci].cons >= 2 {
        dur += 3 * 60;
    }
    s.add_core_status(BURST, dur);
    let f = s.f;
    task(s, ci, TASK_FIRST_TICK, f, g::BURST_HITMARK - g::LEA_LOTUS_APPEAR);
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::BURST_TICK[lvl]);
    ai.abil = g::BURST_ABIL;
    let lamp_atk = s.snapshot_event(ai, 0);
    let t = st_mut(s, ci);
    t.burst_pos = pos;
    t.transfig = None;
    t.burst_radius = 8.0;
    t.hitbox_radius = 2.0;
    t.stacks = 0;
    t.lamp_atk = lamp_atk;
    t.lamp_end = f + dur;
    t.think_from = -1;
    t.icd.clear();
    if s.chars[ci].ascension >= 1 {
        let mut d = 0;
        while d <= dur {
            task(s, ci, TASK_A1_STACK, 0, d);
            d += 60;
        }
        let mut d = 0;
        while d <= dur {
            task(s, ci, TASK_A1_BUFF, 0, d);
            d += 18;
        }
    }
    if s.chars[ci].cons >= 6 {
        let mut d = 0;
        while d <= dur {
            task(s, ci, TASK_C6_BUFF, 0, d);
            d += 18;
        }
    }
    // one lamp at a time: a new one removes the old
    s.kill_gadget(ci, LAMP);
    s.add_hittable_gadget(ci, LAMP, dur, pos.0, pos.1, 1.0, true);
}

/// gcsim's LeaLotus.QueueAttack: a hit on a random enemy within the
/// lamp's area, `delay` frames later.
fn lamp_tick(s: &mut Sim, ci: usize, delay: i32) {
    let t = st(s, ci);
    let area = Pattern::Circle { x: t.burst_pos.0, y: t.burst_pos.1, r: t.burst_radius };
    if !s.enemy_within(area) {
        return;
    }
    // RandomEnemyWithinArea: the one enemy
    let _ = s.rng.intn(1);
    let mut ev = st(s, ci).lamp_atk;
    ev.info.pattern = s.circle_on_target(st(s, ci).hitbox_radius);
    s.queue_attack_with_snap(ev, 0, delay);
}

/// The lamp takes an element: C4's stacks, and it is gone.
fn transfig(s: &mut Sim, ci: usize, e: Element) {
    let t = st_mut(s, ci);
    t.transfig = Some(e);
    if s.chars[ci].cons >= 4 {
        let t = st_mut(s, ci);
        t.stacks = (t.stacks + 5).min(10);
    }
    s.kill_gadget(ci, LAMP);
}

/// The lamp's own WillApplyEle (gcsim's Target.WillApplyEle on it): its
/// counter by actor, tag and group, reset ResetTimer - 1 frames after the
/// first hit.
fn lamp_will_apply(s: &mut Sim, ci: usize, info: &AttackInfo) -> f64 {
    if info.icd_tag == icd::TAG_NONE {
        return 1.0;
    }
    let f = s.f;
    let grp = info.icd_group;
    let t = st_mut(s, ci);
    let k = match t.icd.iter().position(|e| e.0 == info.actor && e.1 == info.icd_tag && e.2 == grp) {
        Some(k) => k,
        None => {
            t.icd.push((info.actor, info.icd_tag, grp, -1, 0));
            t.icd.len() - 1
        }
    };
    let e = &mut t.icd[k];
    if e.3 >= 0 && f >= e.3 {
        e.3 = -1;
        e.4 = 0;
    }
    if e.3 < 0 {
        e.3 = f + grp.reset_timer() - 1;
    }
    let val = e.4;
    e.4 += 1;
    let seq = grp.ele_seq();
    if val < seq.len() { seq[val] } else { seq[seq.len() - 1] }
}
