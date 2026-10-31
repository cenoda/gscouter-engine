//! Kuki Shinobu, from gcsim's internal/characters/kuki: normal and charged
//! attacks, Sanctifying Ring (its HP cost, the ring and its heal) and
//! Gyoei Narukami Kariyama Rite, A1, A4 and every constellation.
//!
//! C4's Thundergrass Mark follows normal, charged and plunging attacks,
//! the tags gcsim lists. Ordinary plunges are not actions she has; the
//! plunge tag is kept so a plunge the engine does produce (Gaming's skill
//! plunge, Kazuha's) still marks. Xingqiu's N1 start delay while she is
//! on field is the generated `anim_delay`, not code here.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::kuki as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The frame the current ring was activated. An older tick stops.
    ring_src: i32,
}

const RING: ModKey = ModKey::new(g::RING_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new("kuki-a1");
const C4_ICD: ModKey = ModKey::new("kuki-c4-icd");
const C6: ModKey = ModKey::new("kuki-c6");
const C6_ICD: ModKey = ModKey::new("kuki-c6-icd");

const CB_PARTICLE: u16 = 1 << 0;

const TASK_RING: u8 = 0;
const TASK_BELL: u8 = 1;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];
const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];

impl Character for Char {
    const KEY: &'static str = "kukishinobu";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].ascension >= 1 {
            let o = Owner::Char(ci as u8);
            s.chars[ci]
                .mods
                .push(Mod::owned(A1, ModKind::Stat, o, st::HEAL, false, false, -1));
        }
        if s.chars[ci].cons >= 4 {
            s.subscribe(EventKind::EnemyDamage, Owner::Char(ci as u8));
        }
        if s.chars[ci].cons >= 6 {
            s.subscribe(EventKind::Drain, Owner::Char(ci as u8));
        }
    }

    /// gcsim's package reads no params.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => {
                p.0.first().map(|(k, _)| format!("{name} param {k}"))
            }
            _ => Some(name.into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(
            ci,
            AttackTag::Normal,
            icd::TAG_NORMAL_ATTACK,
            icd::GROUP_DEFAULT,
            Element::Physical,
            25.0,
            g::AUTO[n][lvl],
        );
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        let off = g::ATTACK_OFFSETS[n];
        let box_ = &g::ATTACK_HITBOXES[n];
        ai.pattern = if n == 2 {
            s.box_on_player(0.0, off, box_[0], box_[1])
        } else {
            s.circle_on_player(0.0, off, box_[0])
        };
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(
            Frames::Attack { table, atkspd },
            table[0],
            h,
            AnimState::Normal,
        );
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        for i in 0..g::CHARGE.len() {
            let mut ai = AttackInfo::new(
                ci,
                AttackTag::Extra,
                icd::TAG_NORMAL_ATTACK,
                icd::GROUP_DEFAULT,
                Element::Physical,
                25.0,
                g::CHARGE[i][lvl],
            );
            ai.abil = CHARGE_NAMES[i];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::CHARGE_HITLAG_HALT_FRAME[i] * 60.0;
            ai.def_halt = g::CHARGE_DEF_HALT[i];
            ai.pattern = s.circle_on_player(0.0, 0.0, 2.8);
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(
            Frames::Table(t),
            t[0],
            g::CHARGE_HITMARKS[g::CHARGE_HITMARKS.len() - 1],
            AnimState::Charge,
        )
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // only above 20% HP, and never below it
        if s.chars[ci].hp_ratio > g::HP_DRAIN_THRESHOLD {
            let max = s.max_hp(ci);
            let current = s.current_hp(ci);
            let mut drain = 0.3 * current;
            if (current - drain) / max <= g::HP_DRAIN_THRESHOLD {
                drain = current - g::HP_DRAIN_THRESHOLD * max;
            }
            s.drain(ci, drain);
        }
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(
            ci,
            AttackTag::ElementalArt,
            icd::TAG_ELEMENTAL_ART,
            icd::GROUP_DEFAULT,
            Element::Electro,
            25.0,
            g::SKILL[lvl],
        );
        ai.abil = "Sanctifying Ring";
        ai.blunt = true;
        ai.poise = 30.0;
        // gcsim adds 25% of EM here with no ascension check; the ticks use A4
        ai.flat_dmg = s.stat(ci, st::EM) * 0.25;
        ai.pattern = s.circle_on_player(0.0, 0.0, 4.0);
        let h = g::SKILL_HITMARK;
        s.queue_attack(ai, 0, h, h);
        // the ring starts after the hit, and hitlag does not move it
        s.add_task(
            Task::Char {
                c: ci as u8,
                id: TASK_RING,
                arg: 0,
            },
            23,
        );
        s.set_cd_with_delay(ci, Action::Skill, 15 * 60, 7);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(
            Frames::Table(t),
            t[0],
            t[Action::Jump as usize],
            AnimState::Skill,
        )
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(
            ci,
            AttackTag::ElementalBurst,
            icd::TAG_ELEMENTAL_BURST,
            icd::GROUP_DEFAULT,
            Element::Electro,
            25.0,
            0.0,
        );
        ai.abil = "Gyoei Narukami Kariyama Rite";
        ai.flat_dmg = s.max_hp(ci) * g::BURST[lvl];
        // C1: the rite's radius is 50% larger
        let r = if s.chars[ci].cons >= 1 { 6.0 } else { 4.0 };
        ai.pattern = s.circle_on_target(r);
        let ev = s.snapshot_event(ai, 0);
        // 7 hits, 12 at half HP or less; every 2/7 s from frame 50
        let count = if s.chars[ci].hp_ratio <= 0.5 { 12 } else { 7 };
        let interval = 2 * 60 / 7;
        let mut i = g::BURST_START;
        let end = count * interval + g::BURST_START;
        while i < end {
            s.queue_attack_with_snap(ev, 0, i);
            i += interval;
        }
        s.consume_energy(ci, 4);
        s.set_cd(ci, Action::Burst, 15 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(
            Frames::Table(t),
            t[0],
            t[Action::Attack as usize],
            AnimState::Burst,
        )
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_RING => {
                // C2: the ring lasts 15 s instead of 12
                let dur = if s.chars[ci].cons >= 2 { 900 } else { 720 };
                s.add_core_status(RING, dur);
                let src = s.f;
                s.st_mut::<State>(ci).ring_src = src;
                s.add_task(
                    Task::Char {
                        c: ci as u8,
                        id: TASK_BELL,
                        arg: src,
                    },
                    90,
                );
            }
            TASK_BELL => bell_tick(s, ci, arg),
            _ => unreachable!(),
        }
    }

    /// A1: Healing Bonus +15% at half HP or less.
    fn mod_amount(s: &Sim, ci: usize, _: usize, _: &Mod, out: &mut Stats) -> bool {
        if s.chars[ci].hp_ratio > 0.5 {
            return false;
        }
        out[st::HEAL] += 0.15;
        true
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 {
            particle_cb(s, ci);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyDamage(ev) => c4(s, ci, ev.info.tag, ev.info.actor),
            // gcsim's handler does not look at who was drained
            Event::Drain { amount, .. } if amount > 0.0 => c6(s, ci),
            _ => {}
        }
    }
}

/// A ring tick: damage two frames later, the heal now, then the next tick
/// while the ring lasts. Hitlag does not move either.
fn bell_tick(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).ring_src != src {
        return;
    }
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(
        ci,
        AttackTag::ElementalArt,
        icd::TAG_ELEMENTAL_ART,
        icd::GROUP_DEFAULT,
        Element::Electro,
        25.0,
        g::SKILLDOT[lvl],
    );
    ai.abil = "Grass Ring of Sanctification";
    ai.flat_dmg = a4_damage(s, ci);
    ai.pattern = s.circle_on_player(0.0, 0.0, 4.0);
    s.queue_attack(ai, CB_PARTICLE, 2, 2);
    let src_hp = g::SKILLHEALPP[lvl] * s.max_hp(ci) + g::SKILLHEALFLAT[lvl] + a4_healing(s, ci);
    let bonus = s.stat(ci, st::HEAL);
    s.heal(ci, s.player.active as usize, src_hp, bonus);
    if s.core_status(g::RING_KEY) == 0 {
        return;
    }
    s.add_task(
        Task::Char {
            c: ci as u8,
            id: TASK_BELL,
            arg: src,
        },
        90,
    );
}

/// C4: once every 5 s, while the ring is up, the active character's
/// normal, charged or plunging hit drops a mark on the enemy.
fn c4(s: &mut Sim, ci: usize, tag: AttackTag, actor: u8) {
    if s.mod_active(ci, C4_ICD) {
        return;
    }
    if !matches!(
        tag,
        AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge
    ) {
        return;
    }
    if actor as usize != s.player.active as usize {
        return;
    }
    if s.core_status(g::RING_KEY) == 0 {
        return;
    }
    s.add_status(ci, C4_ICD, 5 * 60, true);
    let mut ai = AttackInfo::new(
        ci,
        AttackTag::ElementalArt,
        icd::TAG_NONE,
        icd::GROUP_DEFAULT,
        Element::Electro,
        25.0,
        0.0,
    );
    ai.abil = "Thundergrass Mark (C4)";
    ai.flat_dmg = s.max_hp(ci) * 0.097;
    ai.pattern = s.circle_on_target(2.0);
    s.queue_attack(ai, CB_PARTICLE, 5, 5);
}

/// C6: at 25% HP or less, EM +150 for 15 s, once a minute. At 0 HP she is
/// set back to 1. Any drain qualifies, as gcsim wrote it.
fn c6(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, C6_ICD) || s.chars[ci].hp_ratio > 0.25 {
        return;
    }
    if s.chars[ci].hp_ratio <= 0.0 {
        s.set_hp(ci, 1.0);
    }
    s.add_status(ci, C6_ICD, 60 * 60, false);
    s.add_stat_mod(ci, C6, st::EM, 150.0, 15 * 60, true);
}

fn a4_damage(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 4 {
        return 0.0;
    }
    s.stat(ci, st::EM) * 0.25
}

fn a4_healing(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 4 {
        return 0.0;
    }
    s.stat(ci, st::EM) * 0.75
}

/// One electro particle, 45% of the time, once every 0.2 s. The callback
/// runs only for the enemy hit, which is gcsim's enemy-target check.
fn particle_cb(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, PARTICLE_ICD) {
        return;
    }
    s.add_status(ci, PARTICLE_ICD, 12, false);
    if s.rng.float64() < 0.45 {
        s.queue_particle(1.0, Element::Electro, s.particle_delay[ci]);
    }
}
