//! The electro Traveler (lumineelectro and aetherelectro), from gcsim's
//! internal/characters/traveler/common/electro: normal and charged
//! attacks, Lightning Blade (its blades, Abundance Amulets and their
//! pickup), Bellowing Thunder (the burst and Falling Thunder on the active
//! character's normal and charged hits), A1, A4 and every constellation.
//! gcsim gives it no plunges.
//!
//! One module holds both: `lumineelectro` and `aetherelectro` are
//! `Char<1>` and `Char<0>` (gcsim's gender).

use super::traveler_common;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::traveler_common_electro as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char<const G: u8>;

pub struct State<const G: u8>(Inner);

pub struct Inner {
    gender: usize,
    /// gcsim's abundanceAmulets (and its tag "generated").
    amulets: i32,
    /// The most amulets the current skill makes.
    max_amulets: i32,
    /// gcsim's burstC6Hits and burstC6WillGiveEnergy.
    c6_hits: i32,
    c6_will_give_energy: bool,
    /// Falling Thunder as the burst snapshot it (gcsim's burstAtk).
    burst_atk: Option<AttackEvent>,
    /// The frame Falling Thunder may come again (gcsim's icd).
    icd: i32,
    story: traveler_common::Story,
}

fn st(s: &Sim, ci: usize) -> &Inner {
    match &s.chars[ci].state {
        CharState::LumineElectro(x) => &x.0,
        CharState::AetherElectro(x) => &x.0,
        _ => unreachable!(),
    }
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut Inner {
    match &mut s.chars[ci].state {
        CharState::LumineElectro(x) => &mut x.0,
        CharState::AetherElectro(x) => &mut x.0,
        _ => unreachable!(),
    }
}

const BURST_STATUS: ModKey = ModKey::new("travelerelectroburst");
const AMULET: ModKey = ModKey::new("abundance-amulet");
const C2: ModKey = ModKey::new("travelerelectro-c2");

const CB_PARTICLE: u16 = 1 << 0;
const CB_AMULET: u16 = 1 << 1;
const CB_THUNDER_ENERGY: u16 = 1 << 2;
const CB_C2: u16 = 1 << 3;
const CB_C6_ENERGY: u16 = 1 << 4;

/// The amulets' pickup by the active character (the fight's queue).
const TASK_COLLECT: u8 = 0;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];
const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];

impl<const G: u8> Character for Char<G> {
    const KEY: &'static str = if G == 1 { "lumineelectro" } else { "aetherelectro" };
    type State = State<G>;

    fn new_state(cs: &CharSetup) -> State<G> {
        State(Inner {
            gender: G as usize,
            amulets: 0,
            max_amulets: 0,
            c6_hits: 0,
            c6_will_give_energy: false,
            burst_atk: None,
            icd: 0,
            story: traveler_common::Story::new(cs),
        })
    }

    fn init(s: &mut Sim, ci: usize) {
        traveler_common::story_buffs(s, ci);
        // burstProc
        s.subscribe(EventKind::EnemyDamage, Owner::Char(ci as u8));
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Skill => p.0.iter().find(|(k, _)| !matches!(k.as_str(), "hits" | "max_amulets" | "amulet_delay")).map(|(k, _)| format!("{name} param {k}")),
            Action::Attack | Action::Charge | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match (fields[0].as_str(), fields.get(1).map(String::as_str)) {
            ("tags", Some("generated")) => Some(Ok(Num::Int(st(s, ci).amulets as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[sex][n] * 60.0;
        ai.def_halt = true;
        let off = g::ATTACK_OFFSETS[sex][n];
        let hb = g::ATTACK_HITBOXES[sex][n];
        ai.pattern = if (sex == 0 && (n == 0 || n == 2)) || (sex == 1 && n == 1) {
            s.box_on_player(0.0, off, hb[0], hb[1])
        } else {
            s.fan_on_player(0.0, off, hb[0], g::ATTACK_FAN_ANGLES[sex][n])
        };
        let h = g::ATTACK_HITMARKS[sex][n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[sex][n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, 0.0);
        let marks = g::CHARGE_HITMARKS[sex];
        for (i, mult) in g::CHARGE[sex].iter().enumerate() {
            ai.mult = mult[lvl];
            ai.abil = CHARGE_NAMES[i];
            ai.pattern = s.circle_on_player(0.0, 0.0, 2.2);
            s.queue_attack(ai, 0, marks[i], marks[i]);
        }
        let t = &g::CHARGE_FRAMES[sex];
        ActInfo::new(Frames::Table(t), t[0], marks[marks.len() - 1], AnimState::Charge)
    }

    /// Lightning Blade: its blades (1 by default, 3 at most), each making
    /// an amulet as it hits, picked up by the active character later.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL[lvl]);
        ai.abil = "Lightning Blade";
        ai.pattern = s.box_on_target(0.0, 0.0, 0.1, 0.6);
        let ev = s.snapshot_event(ai, 0);
        let hits = p.get("hits").map_or(1, |h| h.min(3));
        let mut max_amulets = if s.chars[ci].cons >= 1 { 3 } else { 2 };
        if let Some(m) = p.get("max_amulets") {
            if (m as i32) < max_amulets {
                max_amulets = m as i32;
            }
        }
        // about 1.79 s, and never sooner
        let delay = p.get("amulet_delay").unwrap_or(0).max(107) as i32;
        let stv = st_mut(s, ci);
        stv.amulets = 0;
        stv.max_amulets = max_amulets;
        for _ in 0..hits {
            s.queue_attack_with_snap(ev, CB_PARTICLE | CB_AMULET, g::SKILL_HITMARK);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_COLLECT, arg: 0 }, delay);
        s.set_cd_with_delay(ci, Action::Skill, 810, 20);
        let t = &g::SKILL_FRAMES[sex];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    /// Bellowing Thunder, and Falling Thunder snapshot for 12 s.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::BURST[lvl]);
        ai.abil = "Bellowing Thunder";
        ai.blunt = true;
        ai.poise = 150.0;
        ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
        s.queue_attack(ai, 0, 0, g::BURST_HITMARK);
        s.set_cd_with_delay(ci, Action::Burst, 1200, 35);
        s.consume_energy(ci, 37);
        // not extended by hitlag
        s.add_core_status(BURST_STATUS, 720);
        let mut proc = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::BURST_TICK[lvl]);
        proc.abil = "Falling Thunder";
        let ev = s.snapshot_event(proc, 0);
        st_mut(s, ci).burst_atk = Some(ev);
        let t = &g::BURST_FRAMES[sex];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_COLLECT => {
                let active = s.player.active as usize;
                collect_amulets(s, ci, active);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        // each blade has its own makeParticleCB: a particle each
        if cbs & CB_PARTICLE != 0 {
            s.queue_particle(1.0, Element::Electro, s.particle_delay[ci]);
        }
        if cbs & CB_AMULET != 0 {
            let stv = st_mut(s, ci);
            if stv.amulets < stv.max_amulets {
                stv.amulets += 1;
            }
        }
        if cbs & CB_THUNDER_ENERGY != 0 {
            let active = s.player.active as usize;
            let v = g::BURST_REGEN[s.talent(ci, 2)];
            s.add_energy(active, v);
        }
        if cbs & CB_C2 != 0 {
            s.enemy_add_resist(C2, Element::Electro, -0.15, 480);
        }
        if cbs & CB_C6_ENERGY != 0 && st(s, ci).c6_will_give_energy {
            let active = s.player.active as usize;
            s.add_energy(active, 1.0);
            st_mut(s, ci).c6_will_give_energy = false;
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        traveler_common::story_amount(&st(s, ci).story, m, out)
    }

    /// Falling Thunder, on the active character's normal and charged hits
    /// while the burst lasts, once every 0.5 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ae) = *e else { return };
        if !matches!(ae.info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        if ae.info.actor != s.player.active || s.core_status_key(BURST_STATUS) == 0 {
            return;
        }
        if st(s, ci).icd > s.f {
            return;
        }
        let cons = s.chars[ci].cons;
        let Some(mut atk) = st(s, ci).burst_atk else { return };
        atk.snap_f = s.f;
        let r = if cons >= 6 && st(s, ci).c6_will_give_energy { 2.5 } else { 2.0 };
        atk.info.pattern = s.circle_on_target(r);
        // C6: every third Falling Thunder deals 200%
        if cons >= 6 {
            let stv = st_mut(s, ci);
            stv.c6_hits += 1;
            if stv.c6_hits >= 3 {
                atk.info.mult *= 2.0;
                stv.c6_hits = 0;
                stv.c6_will_give_energy = true;
            }
        }
        let mut cbs = CB_THUNDER_ENERGY | CB_C6_ENERGY;
        if cons >= 2 {
            cbs |= CB_C2;
        }
        s.queue_attack_with_snap(atk, cbs, 1);
        st_mut(s, ci).icd = s.f + 30;
    }
}

/// collectAmulets: the collector takes every amulet at once, for energy
/// and 6 s of ER.
fn collect_amulets(s: &mut Sim, ci: usize, collector: usize) {
    let amulets = st(s, ci).amulets;
    if amulets <= 0 {
        return;
    }
    let mut er = 0.20;
    // A4: 10% of the Traveler's ER without extra mods
    if s.chars[ci].ascension >= 4 {
        er += s.stat_ex(ci, st::ER, true) * 0.1;
    }
    // gcsim indexes skillRegen by the talent level itself
    let mut energy = g::SKILL_REGEN[s.chars[ci].talents[1] as usize] * amulets as f64;
    // C4: twice the energy below 35% of the active character's
    if s.chars[ci].cons >= 4 {
        let a = s.player.active as usize;
        if s.chars[a].energy / s.chars[a].energy_max < 0.35 {
            energy *= 2.0;
        }
    }
    s.add_energy(collector, energy);
    // A1: 1.5 s off the skill per amulet another member takes
    if s.chars[ci].ascension >= 1 && collector != ci {
        s.reduce_cd(ci, Action::Skill, 90 * amulets);
    }
    s.add_mod(collector, Mod::stat(AMULET, st::ER, er, true, true, -1), 360);
    st_mut(s, ci).amulets = 0;
}
