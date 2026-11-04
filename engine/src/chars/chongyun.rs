//! Chongyun, from gcsim's internal/characters/chongyun: normal attacks
//! (C1's blades after the fourth), the skill's frost field (its cryo
//! infusion on the active character while the player is in it, and on
//! swaps while it lasts), the burst's blades, A1, A4's blade when the field
//! ends, every constellation (C2 as a cooldown mod, C6 in damage mode) and
//! the plunges from Xianyun's airborne state.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::chongyun as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    skill_area: Pattern,
    field_src: i32,
    /// gcsim's a4Snap.
    a4_snap: AttackEvent,
    /// The A4 blades queued: their snapshot and the field's area when
    /// queued (gcsim's closure), by slot.
    a4_pending: [(AttackEvent, Pattern); 4],
    a4_next: usize,
    /// The enemy's HP at the start, for C6.
    enemy_max_hp: f64,
}

const FIELD: ModKey = ModKey::new(g::SKILL_FIELD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const INFUSION: ModKey = ModKey::new("chongyun-ice-weapon");
const A1: ModKey = ModKey::new("chongyun-field");
const A4_SHRED: ModKey = ModKey::new("chongyun-a4");
const C2: ModKey = ModKey::new("chongyun-c2");
const C6: ModKey = ModKey::new("chongyun-c6");

const CB_PARTICLE: u16 = 1 << 0;
const CB_C4: u16 = 1 << 1;
const CB_A4: u16 = 1 << 2;

const TASK_FIELD: u8 = 0;
/// arg: the skill's src.
const TASK_TICK: u8 = 1;
/// arg: src * 4 + slot.
const TASK_A4: u8 = 2;

const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn c4_cb(s: &Sim, ci: usize) -> u16 {
    if s.chars[ci].cons >= 4 {
        CB_C4
    } else {
        0
    }
}

/// gcsim's a4: the blade when the field ends, `delay` frames on.
fn a4(s: &mut Sim, ci: usize, delay: i32, src: i32, use_old: bool) {
    if s.chars[ci].ascension < 4 {
        return;
    }
    let snap = if use_old {
        st(s, ci).a4_snap
    } else {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL[s.talent(ci, 1)]);
        ai.abil = "Spirit Blade: Chonghua's Layered Frost (A4)";
        ai.blunt = true;
        ai.poise = 100.0;
        ai.pattern = s.circle_on_target(3.5);
        let snap = s.snapshot_event(ai, 0);
        st_mut(s, ci).a4_snap = snap;
        snap
    };
    let x = st_mut(s, ci);
    let slot = x.a4_next;
    x.a4_next = (x.a4_next + 1) % 4;
    x.a4_pending[slot] = (snap, x.skill_area);
    s.add_task(Task::Char { c: ci as u8, id: TASK_A4, arg: src * 4 + slot as i32 }, delay);
}

/// gcsim's infuse: on the active character, for the skill level's
/// duration.
fn infuse(s: &mut Sim, ci: usize) {
    let active = s.player.active as usize;
    let dur = (g::INFUSE_DUR[s.talent(ci, 1)] * 60.0) as i32;
    // C2: skill and burst cooldowns 15% shorter
    if s.chars[ci].cons >= 2 {
        let mut m = Mod::new(C2, ModKind::Cooldown, st::NONE, false, true, -1);
        m.value = -0.15;
        s.add_mod(active, m, dur);
    }
    if matches!(s.chars[active].class, WeaponClass::Claymore | WeaponClass::Spear | WeaponClass::Sword) {
        s.add_infusion(active, INFUSION, Element::Cryo, dur, true, &[AttackTag::Normal, AttackTag::Extra, AttackTag::Plunge]);
        // A1: ATK SPD
        if s.chars[ci].ascension >= 1 {
            s.add_stat_mod(active, A1, st::ATKSPD, 0.08, dur, true);
        }
    }
}

impl Character for Char {
    const KEY: &'static str = "chongyun";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        let ev = AttackEvent::new(AttackInfo::new(0, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 0.0, 0.0), 0);
        let p = Pattern::Circle { x: 0.0, y: 0.0, r: 0.0 };
        State { skill_area: p, field_src: -601, a4_snap: ev, a4_pending: [(ev, p); 4], a4_next: 0, enemy_max_hp: 0.0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        s.subscribe(EventKind::Swap, o);
        if s.chars[ci].cons >= 6 && s.damage_mode {
            st_mut(s, ci).enemy_max_hp = s.enemy_hp;
            s.add_mod(ci, Mod::owned(C6, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
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

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.blunt = true;
        ai.poise = g::ATTACK_POISE_DMG[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        let (off, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
        ai.pattern = if n == 3 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.fan_on_player(0.0, off, hb[0], g::ATTACK_FAN_ANGLES[n]) };
        let h = g::ATTACK_HITMARKS[n];
        let cb = c4_cb(s, ci);
        s.queue_attack(ai, cb, h, h);
        // C1: three blades after the fourth hit
        if s.chars[ci].cons >= 1 && n == 3 {
            let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, 0.5);
            ai.abil = "Chongyun C1";
            ai.pattern = s.circle_on_target(1.0);
            for i in 0..3 {
                s.queue_attack(ai, cb, h + i * 5, h + i * 5);
            }
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 50.0, g::SKILL[s.talent(ci, 1)]);
        ai.abil = "Spirit Blade: Chonghua's Layered Frost";
        ai.blunt = true;
        ai.poise = 150.0;
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = 0.09 * 60.0;
        let src = s.f;
        st_mut(s, ci).field_src = src;
        // a field still up ends with its old blade, before the new area
        if s.core_status_key(FIELD) > 0 {
            a4(s, ci, g::SKILL_HITMARK + 45, src, true);
        }
        let area = s.circle_on_player(0.0, 1.5, 8.0);
        st_mut(s, ci).skill_area = area;
        let Pattern::Circle { x, y, .. } = area else { unreachable!() };
        ai.pattern = Pattern::Circle { x, y, r: 2.5 };
        let cbs = CB_PARTICLE | c4_cb(s, ci);
        s.queue_attack(ai, cbs, 0, g::SKILL_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_FIELD, arg: 0 }, g::SKILL_HITMARK);
        let mut i = 0;
        while i <= 600 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_TICK, arg: src }, i + g::SKILL_HITMARK);
            i += 60;
        }
        a4(s, ci, 655, src, false);
        s.set_cd_with_delay(ci, Action::Skill, 900, 34);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST[s.talent(ci, 2)]);
        ai.abil = "Spirit Blade: Cloud-Parting Star";
        ai.blunt = true;
        ai.poise = 100.0;
        ai.pattern = s.circle_on_target(3.5);
        let cb = c4_cb(s, ci);
        for &h in g::BURST_HITMARKS {
            s.queue_attack(ai, cb, h, h);
        }
        if s.chars[ci].cons >= 6 {
            s.queue_attack(ai, cb, g::BURST_HITMARK_C6, g::BURST_HITMARK_C6);
        }
        s.set_cd(ci, Action::Burst, 720);
        s.consume_energy(ci, 6);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_FIELD => s.add_core_status(FIELD, 600),
            TASK_TICK => {
                if arg == st(s, ci).field_src && s.player_within(st(s, ci).skill_area) {
                    infuse(s, ci);
                }
            }
            TASK_A4 => {
                if arg >> 2 != st(s, ci).field_src {
                    return;
                }
                let (mut ev, area) = st(s, ci).a4_pending[(arg & 3) as usize];
                let Pattern::Circle { x, y, .. } = area else { unreachable!() };
                ev.info.pattern = if s.enemy_alive && s.enemy_within(area) { s.circle_on_target(3.5) } else { Pattern::Circle { x, y, r: 3.5 } };
                let cbs = CB_A4 | c4_cb(s, ci);
                s.queue_attack_with_snap(ev, cbs, 0);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 12, true);
            s.queue_particle(4.0, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_A4 != 0 {
            s.enemy_add_resist(A4_SHRED, Element::Cryo, -0.10, 480);
        }
        // C4: energy on hitting a cryo-affected enemy, while on field
        if cbs & CB_C4 != 0 && s.player.active as usize == ci && s.enemy_aura_contains(Element::Cryo) && !s.mod_active(ci, C4_ICD) {
            s.add_status(ci, C4_ICD, 120, true);
            s.add_energy(ci, 2.0);
        }
    }

    /// C6: burst DMG on an enemy with a lower HP ratio than his.
    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::ElementalBurst && s.enemy_hp / st(s, ci).enemy_max_hp < s.chars[ci].hp_ratio {
            ev.stats[st::DMGP] += 0.15;
        }
    }

    /// A swap while the field lasts infuses the new active character.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Swap { .. } = *e {
            if s.core_status_key(FIELD) > 0 {
                infuse(s, ci);
            }
        }
    }
}
