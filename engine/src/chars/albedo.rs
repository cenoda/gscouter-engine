//! Albedo, from gcsim's internal/characters/albedo: normal and charged
//! attacks, the skill's Solar Isotoma (a construct whose Transient
//! Blossoms follow damage to enemies inside it), the burst and its Fatal
//! Blossoms, A1 (an enemy below 50% HP, in damage mode), A4, every
//! constellation, the conditions .albedo.elevator and .albedo.c2stacks,
//! and the plunges from Xianyun's airborne state.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::albedo as g;
use crate::sim::*;
use crate::types::*;
use crate::gcsl::Num;

pub struct Char;

pub struct State {
    last_construct: i32,
    /// The skill's initial hit's snapshot, which the blossoms use.
    bloom_snap: AttackEvent,
    skill_active: bool,
    skill_area: Pattern,
    /// The blossoms' attack and snapshot.
    tick_snap: AttackEvent,
    c2_stacks: i32,
    /// The enemy's HP at the start, for A1.
    enemy_max_hp: f64,
}

const CONSTRUCT: ModKey = ModKey::new("albedo-skill");
const SKILL_ICD: ModKey = ModKey::new(g::SKILL_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const C2: ModKey = ModKey::new(g::C2KEY);
const A1: ModKey = ModKey::new("albedo-a1");
const A4: ModKey = ModKey::new("albedo-a4");
const C4: ModKey = ModKey::new("albedo-c4");
const C6: ModKey = ModKey::new("albedo-c6");

const CB_PARTICLE: u16 = 1 << 0;

const TASK_CONSTRUCT: u8 = 0;
const TASK_C4: u8 = 1;
const TASK_C6: u8 = 2;
const TASK_BLOSSOMS: u8 = 3;

const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];

const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn burst_info(s: &Sim, ci: usize) -> AttackInfo {
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::BURST[s.talent(ci, 2)]);
    ai.abil = "Rite of Progeniture: Tectonic Tide";
    ai.blunt = true;
    ai.poise = 100.0;
    ai
}

impl Character for Char {
    const KEY: &'static str = "albedo";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        let ai = AttackInfo::new(0, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 0.0, 0.0);
        State {
            last_construct: 0,
            bloom_snap: AttackEvent::new(ai, 0),
            skill_active: false,
            skill_area: Pattern::Circle { x: 0.0, y: 0.0, r: 0.0 },
            tick_snap: AttackEvent::new(ai, 0),
            c2_stacks: 0,
            enemy_max_hp: 0.0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        s.subscribe(EventKind::EnemyDamage, o);
        if s.damage_mode && s.chars[ci].ascension >= 1 {
            st_mut(s, ci).enemy_max_hp = s.enemy_hp;
            s.add_mod(ci, Mod::owned(A1, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "elevator" => Some(Ok(Num::Int(st(s, ci).skill_active as i64))),
            "c2stacks" => Some(Ok(Num::Int(st(s, ci).c2_stacks as i64))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        let (off, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
        ai.pattern = if n == 2 || n == 3 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.fan_on_player(0.0, off, hb[0], g::ATTACK_FAN_ANGLES[n]) };
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    /// gcsim tags his charged attack's two hits as normal attacks.
    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::CHARGE.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = CHARGE_NAMES[i];
            ai.pattern = s.fan_on_player(0.0, 0.3, 2.2, 270.0);
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARKS[g::CHARGE_HITMARKS.len() - 1], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL[lvl]);
        ai.abil = g::SKILL_ABIL_INITIAL;
        ai.blunt = true;
        ai.poise = 50.0;
        let mut bloom = s.snapshot_event(ai, 0);
        let pos = s.point_from_player(0.0, 3.0);
        st_mut(s, ci).bloom_snap = bloom;
        st_mut(s, ci).skill_area = Pattern::Circle { x: pos.0, y: pos.1, r: 10.0 };
        bloom.info.pattern = Pattern::Circle { x: pos.0, y: pos.1, r: 5.0 };
        s.queue_attack_with_snap(bloom, 0, g::SKILL_HITMARK);
        // the blossoms' attack, snapshot now
        ai.abil = g::SKILL_ABIL_TICK;
        ai.icd_tag = icd::TAG_ELEMENTAL_ART;
        ai.mult = g::SKILL_TICK[lvl];
        ai.use_def = true;
        let tick = s.snapshot_event(ai, 0);
        st_mut(s, ci).tick_snap = tick;
        // the construct forms after the hit lands
        s.add_task(Task::Char { c: ci as u8, id: TASK_CONSTRUCT, arg: 0 }, g::SKILL_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 240, 23);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::SKILL_HITMARK, AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mut ai = burst_info(s, ci);
        let has_c2 = s.chars[ci].cons >= 2 && s.mod_active(ci, C2);
        // C2's stacks are spent as the burst starts
        let mut c2_count = 0;
        if has_c2 {
            c2_count = st(s, ci).c2_stacks;
            st_mut(s, ci).c2_stacks = 0;
            ai.flat_dmg = total_def(&s.stats(ci)) * c2_count as f64 * 0.3;
        }
        ai.pattern = s.fan_on_player(0.0, 0.0, 8.0, 120.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        // A4 and the blossoms at the hit; arg: C2's stacks, or -1
        let arg = if has_c2 { c2_count } else { -1 };
        s.add_task(Task::Char { c: ci as u8, id: TASK_BLOSSOMS, arg }, g::BURST_HITMARK);
        s.set_cd_with_delay(ci, Action::Burst, 720, 74);
        s.consume_energy(ci, 77);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_CONSTRUCT => {
                let f = s.f;
                s.new_construct(Construct { owner: ci as u8, key: CONSTRUCT, src: f, expiry: f + 1800, pos: None, limited: true }, true);
                let x = st_mut(s, ci);
                x.last_construct = f;
                x.skill_active = true;
                s.delete_mod(ci, SKILL_ICD);
                if s.chars[ci].cons >= 4 {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_C4, arg: f }, 18);
                }
                if s.chars[ci].cons >= 6 {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_C6, arg: f }, 18);
                }
            }
            // C4: plunging DMG for the active character in the field
            TASK_C4 => {
                if st(s, ci).last_construct != arg || !st(s, ci).skill_active {
                    return;
                }
                if s.player_within(st(s, ci).skill_area) {
                    let active = s.player.active as usize;
                    s.add_mod(active, Mod::owned(C4, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 60);
                }
                s.add_task(Task::Char { c: ci as u8, id: TASK_C4, arg }, 18);
            }
            // C6: DMG for the active character in the field with a
            // crystallize shield
            TASK_C6 => {
                if st(s, ci).last_construct != arg || !st(s, ci).skill_active {
                    return;
                }
                let crystallize = s.shields.iter().any(|x| x.typ == ShieldType::Crystallize);
                if crystallize && s.player_within(st(s, ci).skill_area) {
                    let active = s.player.active as usize;
                    s.add_stat_mod(active, C6, st::DMGP, 0.17, 60, false);
                }
                s.add_task(Task::Char { c: ci as u8, id: TASK_C6, arg }, 18);
            }
            TASK_BLOSSOMS => {
                // A4: EM to the party
                if s.chars[ci].ascension >= 4 {
                    for i in 0..s.chars.len() {
                        s.add_stat_mod(i, A4, st::EM, 125.0, 600, true);
                    }
                }
                if !st(s, ci).skill_active {
                    return;
                }
                let mut ai = burst_info(s, ci);
                ai.abil = "Rite of Progeniture: Tectonic Tide (Blossom)";
                ai.poise = 30.0;
                ai.mult = g::BURST_PER_BLOOM[s.talent(ci, 2)];
                if arg >= 0 {
                    ai.flat_dmg = total_def(&s.stats(ci)) * arg as f64 * 0.3;
                }
                let area = st(s, ci).skill_area;
                // RandomEnemiesWithinArea: a permutation of the one enemy
                let tracking = if s.enemy_alive && s.enemy_within(area) {
                    let _ = s.rng.intn(1);
                    1
                } else {
                    0
                };
                let Pattern::Circle { x: cx, y: cy, .. } = area else { unreachable!() };
                let mut ev = st(s, ci).bloom_snap;
                ev.info = ai;
                for i in 0..7 {
                    let (x, y) = if i < tracking { s.enemy_pos } else { s.random_point(cx, cy, 0.5, 9.5) };
                    ev.info.pattern = Pattern::Circle { x, y, r: 3.0 };
                    s.queue_attack_with_snap(ev, 0, g::FATAL_BLOSSOM_HITMARK + i * 5);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 60, false);
            if s.rng.float64() < 0.67 {
                s.queue_particle(1.0, Element::Geo, s.particle_delay[ci]);
            }
        }
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C4 {
            if ev.info.tag == AttackTag::Plunge {
                ev.stats[st::DMGP] += 0.3;
            }
        } else if ev.info.tag == AttackTag::ElementalArt && ev.info.abil != g::SKILL_ABIL_INITIAL && s.enemy_hp / st(s, ci).enemy_max_hp < 0.5 {
            // A1
            ev.stats[st::DMGP] += 0.25;
        }
    }

    fn construct_expired(s: &mut Sim, ci: usize, _: ModKey) {
        st_mut(s, ci).skill_active = false;
    }

    /// The Transient Blossoms: damage to an enemy inside the field.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if !st(s, ci).skill_active || s.mod_active(ci, SKILL_ICD) || ev.info.abil == g::SKILL_ABIL_INITIAL || ev.damage == 0.0 || !s.enemy_within(st(s, ci).skill_area) {
            return;
        }
        // tied to the construct, so not hitlag extended
        s.add_status(ci, SKILL_ICD, 120, false);
        let mut tick = st(s, ci).tick_snap;
        tick.info.pattern = s.circle_on_target(3.4);
        s.queue_attack_with_snap(tick, CB_PARTICLE, 1);
        // C1: energy on each blossom
        let cons = s.chars[ci].cons;
        if cons >= 1 {
            s.add_energy(ci, 1.2);
        }
        // C2: Fatal Reckoning stacks
        if cons >= 2 {
            if !s.mod_active(ci, C2) {
                st_mut(s, ci).c2_stacks = 0;
            }
            s.add_status(ci, C2, 1800, true);
            let x = st_mut(s, ci);
            x.c2_stacks = (x.c2_stacks + 1).min(4);
        }
    }
}
