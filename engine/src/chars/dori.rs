//! Dori, from gcsim's internal/characters/dori: normal attacks,
//! Troubleshooter Shot and its After-Sales Service Rounds, Alcazarzaray's
//! Exactitude (the lamp's connector hits on the line to the player, its
//! drain and electro on the player, the heals and energy), the airborne
//! plunges, A1, A4 and every constellation.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::dori as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    after_count: usize,
    /// The normal attacks' hit areas, made when the attack starts.
    normal_patterns: [[Pattern; 2]; 3],
    /// The burst's area center (5 ahead of the player at the cast), its
    /// snapshot, and the frame each member was last infused by the lamp.
    burst_pos: (f64, f64),
    burst_ev: AttackEvent,
    icd_src: [i32; 4],
    /// C2's travel, from the burst's c2_travel.
    c2_travel: i32,
}

const A1_ICD: ModKey = ModKey::new("dori-a1");
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const C4_HEAL: ModKey = ModKey::new("dori-c4-healbonus");
const C4_ER: ModKey = ModKey::new("dori-c4-er-bonus");
const C6: ModKey = ModKey::new(g::C6_KEY);
const C6_ICD: ModKey = ModKey::new(g::C6_ICD);

const CB_AFTER: u16 = 1 << 0;
const CB_A4: u16 = 1 << 1;
const CB_PARTICLE: u16 = 1 << 2;
const CB_C6: u16 = 1 << 3;

/// A normal attack's hit: arg is attack * 2 + hit (her queue).
const TASK_NORMAL: u8 = 0;
/// A connector tick (the fight's queue).
const TASK_CONNECTOR: u8 = 1;
/// A heal tick (the fight's queue).
const TASK_HEAL: u8 = 2;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];
const CONNECTOR_ABIL: &str = "Alcazarzaray's Exactitude: Connector DMG";

/// The airborne plunges (Xianyun's), from gcsim's plunge.go.
const PLUNGE: plunge::Spec = plunge::Spec::physical(
    g::LOW_PLUNGE_HITMARK,
    g::HIGH_PLUNGE_HITMARK,
    g::COLLISION_HITMARK,
    &g::LOW_PLUNGE_FRAMES,
    &g::HIGH_PLUNGE_FRAMES,
    g::LOW_PLUNGE_POISE_DMG,
    g::HIGH_PLUNGE_POISE_DMG,
    g::LOW_PLUNGE_RADIUS,
    g::HIGH_PLUNGE_RADIUS,
    g::LOW_PLUNGE,
    g::HIGH_PLUNGE,
    g::COLLISION,
);

impl Character for Char {
    const KEY: &'static str = "dori";
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        let ev = AttackEvent::new(AttackInfo::new(0, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DORI_BURST, Element::Electro, 25.0, 0.0), 0);
        State {
            after_count: if cs.cons >= 1 { 3 } else { 2 },
            normal_patterns: [[Pattern::Single; 2]; 3],
            burst_pos: (0.0, 0.0),
            burst_ev: ev,
            icd_src: [i32::MIN; 4],
            c2_travel: 10,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::Reaction, Owner::Char(ci as u8));
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Skill => p.0.iter().find(|(k, _)| k != "travel").map(|(k, _)| format!("skill param {k}")),
            Action::Burst => p.0.iter().find(|(k, _)| k != "c2_travel").map(|(k, _)| format!("burst param {k}")),
            Action::Attack => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// Plunges only while airborne (Xianyun's jump).
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let hits = g::ATTACK_HITMARKS[n];
        let p = s.circle_on_player(0.0, g::ATTACK_OFFSETS[n], 2.0);
        // makeC6CB: only while C6's infusion lasts
        let c6 = s.chars[ci].cons >= 6 && c6_infused(s, ci);
        for (i, &h) in hits.iter().enumerate() {
            s.st_mut::<State>(ci).normal_patterns[n][i] = p;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: ((n * 2 + i) as i32) << 1 | c6 as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hits[hits.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    /// Troubleshooter Shot: the shot `travel` frames after its release
    /// (its After-Sales Service Rounds, A4's energy and the particles); C6's
    /// infusion.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL[lvl]);
        ai.abil = "Troubleshooter Shot";
        if s.chars[ci].cons >= 6 {
            s.add_infusion(ci, C6, Element::Electro, 228, true, &[AttackTag::Normal, AttackTag::Extra, AttackTag::Plunge]);
        }
        ai.pattern = s.circle_toward_target(0.0, 0.0, 1.0);
        let mut cbs = CB_AFTER | CB_PARTICLE;
        if s.chars[ci].ascension >= 4 {
            cbs |= CB_A4;
        }
        s.queue_attack(ai, cbs, 0, g::SKILL_RELEASE + travel);
        s.set_cd_with_delay(ci, Action::Skill, 9 * 60, 16);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    /// Alcazarzaray's Exactitude: the lamp 5 ahead of the player; a
    /// connector tick every 24 frames from 28 frames in (32 of them) and a
    /// heal every 120 frames from 11 (6), while the player is within 10 of
    /// it; one snapshot at the cast.
    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DORI_BURST, Element::Electro, 25.0, g::BURST[lvl]);
        ai.abil = CONNECTOR_ABIL;
        let ev = s.snapshot_event(ai, 0);
        let pos = s.point_from_player(0.0, 5.0);
        let st = s.st_mut::<State>(ci);
        st.burst_ev = ev;
        st.burst_pos = pos;
        st.icd_src = [i32::MIN; 4];
        st.c2_travel = p.get("c2_travel").map_or(10, |t| t as i32);
        let c = ci as u8;
        for i in 0..32 {
            s.add_task(Task::Char { c, id: TASK_CONNECTOR, arg: 0 }, 24 * i + g::BURST_HITMARK);
        }
        for i in 0..6 {
            s.add_task(Task::Char { c, id: TASK_HEAL, arg: 0 }, 120 * i + 11);
        }
        s.consume_energy(ci, 4);
        s.set_cd_with_delay(ci, Action::Burst, 1200, 1);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let c6 = arg & 1 == 1;
                let k = (arg >> 1) as usize;
                let (n, i) = (k / 2, k % 2);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::AUTO[n][i][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.blunt = true;
                ai.poise = g::ATTACK_POISE_DMG[n];
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.def_halt = g::ATTACK_HITLAG_DEF_HALT[n][i];
                ai.pattern = s.st::<State>(ci).normal_patterns[n][i];
                s.queue_attack(ai, if c6 { CB_C6 } else { 0 }, 0, 0);
            }
            TASK_CONNECTOR => connector(s, ci),
            TASK_HEAL => heal_tick(s, ci),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        // the After-Sales Service Rounds: two (three from C1)
        if cbs & CB_AFTER != 0 {
            let lvl = s.talent(ci, 1);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_AFTER[lvl]);
            ai.abil = "After-Sales Service Round";
            ai.pattern = s.circle_toward_target(0.0, 0.0, 1.0);
            for i in 0..s.st::<State>(ci).after_count {
                s.queue_attack(ai, 0, 0, g::SKILL_SALES_HITMARKS[i]);
            }
        }
        // A4: energy of 5 times the shot's ER, up to 15
        if cbs & CB_A4 != 0 {
            s.add_energy(ci, (ev.stats[st::ER] * 5.0).min(15.0));
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 90, true);
            s.queue_particle(2.0, Element::Electro, s.particle_delay[ci]);
        }
        // C6: her normal hits on field in the infusion heal the party for
        // 4% of her max HP, once in 0.1 s
        if cbs & CB_C6 != 0 && s.player.active as usize == ci && c6_infused(s, ci) && !s.mod_active(ci, C6_ICD) {
            s.add_status(ci, C6_ICD, 6, true);
            let src = 0.04 * s.max_hp(ci);
            let bonus = s.stat(ci, st::HEAL);
            s.heal_info(ci, None, false, src, bonus);
        }
    }

    fn mod_amount(_: &Sim, _: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C4_HEAL {
            out[st::HEAL] += 0.5;
            return true;
        }
        false
    }

    /// A1: the active member's electro reactions (on the enemy, Hyperbloom
    /// from anywhere) cut her skill's cooldown by 1 s, once in 3 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        let ok = match r {
            Reaction::Hyperbloom => true,
            Reaction::Overload | Reaction::ElectroCharged | Reaction::LunarCharged | Reaction::Superconduct | Reaction::StellarConduct | Reaction::Quicken | Reaction::Aggravate | Reaction::CrystallizeElectro | Reaction::SwirlElectro => ev.on_enemy(),
            _ => false,
        };
        if !ok || s.player.active != ev.info.actor || s.mod_active(ci, A1_ICD) {
            return;
        }
        s.add_status(ci, A1_ICD, 180, true);
        s.reduce_cd(ci, Action::Skill, 60);
    }
}

/// gcsim's WeaponInfuseIsActive for C6's infusion.
fn c6_infused(s: &Sim, ci: usize) -> bool {
    let f = s.f as f64;
    s.infusion[ci].is_some_and(|i| i.key == C6 && !(i.expiry < f && i.expiry > -1.0))
}

/// A connector tick while the player is within 10 of the lamp: a hit on
/// the line from the lamp to the player, a drain of nothing on the active
/// member, electro on the player once in the group's ICD for each member,
/// and C4.
fn connector(s: &mut Sim, ci: usize) {
    let (x, y) = s.st::<State>(ci).burst_pos;
    if !s.player_within(Pattern::Circle { x, y, r: 10.0 }) {
        return;
    }
    let (px, py) = s.player_pos;
    let distance = ((px - x).powi(2) + (py - y).powi(2)).sqrt();
    let mut ev = s.st::<State>(ci).burst_ev;
    ev.info.pattern = s.box_from_player((x, y), 0.0, -distance, 1.0, distance);
    s.queue_attack_with_snap(ev, 0, 0);
    let idx = s.player.active as usize;
    s.drain_info(idx, 0.0, true, CONNECTOR_ABIL);
    if s.f > s.st::<State>(ci).icd_src[idx].saturating_add(icd::GROUP_DORI_BURST.reset_timer()) {
        let dur = if s.player_aura_count() == 0 { 20.0 } else { 25.0 };
        s.apply_self_infusion(AURA_ELECTRO, dur, (9.5 * 60.0) as i32);
        let f = s.f;
        s.st_mut::<State>(ci).icd_src[idx] = f;
    }
    if s.chars[ci].cons >= 4 {
        c4(s, ci);
    }
}

/// A heal tick while the player is within 10 of the lamp: C2's shot, the
/// active member's heal and energy.
fn heal_tick(s: &mut Sim, ci: usize) {
    let (x, y) = s.st::<State>(ci).burst_pos;
    if !s.player_within(Pattern::Circle { x, y, r: 10.0 }) {
        return;
    }
    if s.chars[ci].cons >= 2 {
        let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_DORI_C2, icd::GROUP_DEFAULT, Element::Electro, 25.0, 0.5);
        ai.abil = "Special Franchise (C2)";
        ai.pattern = s.circle_toward_target(0.0, 0.0, 1.0);
        let travel = s.st::<State>(ci).c2_travel;
        s.queue_attack(ai, 0, 0, travel);
    }
    let lvl = s.talent(ci, 2);
    let active = s.player.active as usize;
    let src = g::BURSTHEALPP[lvl] * s.max_hp(ci) + g::BURSTHEALFLAT[lvl];
    let bonus = s.st::<State>(ci).burst_ev.stats[st::HEAL];
    s.heal(ci, active, src, bonus);
    s.add_energy(active, g::BURSTENERGY[lvl]);
}

/// C4: the active member below half HP heals 50% more, below half energy
/// gains 30% ER, for 0.8 s.
fn c4(s: &mut Sim, ci: usize) {
    let active = s.player.active as usize;
    if s.chars[active].hp_ratio < 0.5 {
        s.add_mod(active, Mod::owned(C4_HEAL, ModKind::HealBonus, Owner::Char(ci as u8), st::NONE, false, true, -1), 48);
    }
    if s.chars[active].energy / s.chars[active].energy_max < 0.5 {
        s.add_stat_mod(active, C4_ER, st::ER, 0.3, 48, true);
    }
}
