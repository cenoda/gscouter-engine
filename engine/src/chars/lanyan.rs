//! Lan Yan, from gcsim's internal/characters/lanyan: normal attacks, the
//! charged attack, Swallow-Wisp Pinion Dance (the detecting dash, the
//! shield, A1's absorbed element, and the Feathermoon Rings of its second
//! press), Lustrous Moonrise, A4's flat damage, the airborne plunges and
//! every constellation.
//!
//! The skill's frames are the hit ones while the leap-back status lasts
//! and the miss ones otherwise, read when the next action asks (gcsim's
//! closure): its detecting hit sets a bit of the running skill's frames,
//! and the status's end clears it.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gamedata::chars::lanyan as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    particle_generated: bool,
    absorbed: Element,
}

const LEAP_BACK: ModKey = ModKey::new(g::LEAP_BACK_STATUS);
const SHIELD: ModKey = ModKey::new("Swallow-Wisp Pinion Dance (Shield)");
const C2_ICD: ModKey = ModKey::new(g::C2_ICD);
const C4: ModKey = ModKey::new("lanyan-c4");

const CB_LEAP: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;

/// The shield after a miss, unless one is there (the fight's queue).
const TASK_MISS_SHIELD: u8 = 0;
/// The shield after a hit, of the absorbed element (the fight's queue).
const TASK_HIT_SHIELD: u8 = 1;
/// The leap-back status has ended: the skill's miss frames (her queue).
const TASK_LEAP_END: u8 = 2;

const FR_SKILL: u8 = 0;
/// The skill's frames argument: the hold, and this bit while it reads its
/// hit frames.
const FR_HIT: i16 = 1 << 12;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];

/// The airborne plunges (Xianyun's), from gcsim's plunge.go.
const PLUNGE: plunge::Spec = plunge::Spec::catalyst(
    Element::Anemo,
    g::LOW_PLUNGE_HITMARK,
    g::HIGH_PLUNGE_HITMARK,
    g::COLLISION_HITMARK,
    &g::LOW_PLUNGE_FRAMES,
    &g::HIGH_PLUNGE_FRAMES,
    g::LOW_PLUNGE_RADIUS,
    g::HIGH_PLUNGE_RADIUS,
    g::LOW_PLUNGE,
    g::HIGH_PLUNGE,
    g::COLLISION,
);

impl Character for Char {
    const KEY: &'static str = "lanyan";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { particle_generated: false, absorbed: Element::Anemo }
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].cons >= 2 {
            s.subscribe(EventKind::EnemyHit, Owner::Char(ci as u8));
        }
        if s.chars[ci].cons >= 6 {
            let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
            cd.available = 2;
            cd.max_extra = 1;
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// Plunges only while airborne (Xianyun's jump).
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    /// The second press, in leap-back, is ready whatever the cooldown.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && s.mod_active(ci, LEAP_BACK)).then_some(None)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, LEAP_BACK) {
            return rings(s, ci);
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let hb = g::ATTACK_HITBOXES[n];
        let oy = g::ATTACK_OFFSETS[n];
        for (i, &h) in g::ATTACK_HITMARKS[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::ATTACK[n][i][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            if ai.hitlag_halt > 0.0 {
                ai.hitlag_factor = 0.01;
            }
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            ai.pattern = if hb.len() == 2 {
                s.box_on_player(0.0, oy, hb[0], hb[1])
            } else if g::ATTACK_FAN_ANGLES[n] > 0.0 {
                s.fan_on_player(0.0, oy, hb[0], g::ATTACK_FAN_ANGLES[n])
            } else {
                s.circle_on_player(0.0, oy, hb[0])
            };
            s.queue_attack(ai, 0, h, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], g::ATTACK_CQA[n], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charged Attack";
        ai.deployable = true;
        ai.pattern = s.circle_toward_target(0.0, 0.0, 2.5);
        for &h in g::CHARGE_HITMARKS {
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Charge)
    }

    /// Swallow-Wisp Pinion Dance: the detecting dash `hold` frames late (0
    /// to 610); a hit leaps back (the second press) and shields with the
    /// absorbed element, a miss shields with anemo. In leap-back, the
    /// Feathermoon Rings.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.mod_active(ci, LEAP_BACK) {
            return rings(s, ci);
        }
        let hold = p.get("hold").map_or(0, |h| h.clamp(0, 610)) as i32;
        let st = s.st_mut::<State>(ci);
        st.particle_generated = false;
        st.absorbed = Element::Anemo;
        s.delete_mod(ci, LEAP_BACK);
        let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 0.0, 0.0);
        ai.abil = "Swallow-Wisp Pinion Dance: Detect";
        ai.pattern = s.box_on_player(0.0, 0.0, 0.1, 5.0);
        let h = g::DETECT_HITMARK + hold;
        s.queue_attack(ai, CB_LEAP, h, h);
        s.add_task(Task::Char { c: ci as u8, id: TASK_MISS_SHIELD, arg: 0 }, h + g::MISS_GENERATE_SHIELD);
        s.set_cd_with_delay(ci, Action::Skill, 16 * 60, 4 + hold);
        ActInfo::new(Frames::Char { id: FR_SKILL, arg: hold as i16 }, g::SKILL_HIT_FRAMES[0] + hold, h, AnimState::Skill)
    }

    /// Lustrous Moonrise: three hits on the enemy (A4's flat damage from
    /// EM at the cast); C4's EM for the party.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Anemo, 50.0, g::BURST[lvl]);
        ai.abil = "Lustrous Moonrise";
        if s.chars[ci].ascension >= 4 {
            ai.flat_dmg = s.stat(ci, st::EM) * 7.74;
        }
        ai.pattern = s.circle_on_target(6.0);
        for &h in g::BURST_HITMARKS {
            s.queue_attack(ai, 0, h, h);
        }
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 4);
        if s.chars[ci].cons >= 4 {
            for k in 0..s.chars.len() {
                s.add_stat_mod(k, C4, st::EM, 60.0, 12 * 60, true);
            }
        }
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        debug_assert_eq!(id, FR_SKILL);
        let t = if arg & FR_HIT != 0 { &g::SKILL_HIT_FRAMES } else { &g::SKILL_MISS_FRAMES };
        t[next as usize] + (arg & (FR_HIT - 1)) as i32
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_MISS_SHIELD => {
                if !has_shield(s) {
                    gen_shield(s, ci, Element::Anemo);
                }
            }
            TASK_HIT_SHIELD => {
                let ele = s.st::<State>(ci).absorbed;
                gen_shield(s, ci, ele);
            }
            TASK_LEAP_END => {
                if !s.mod_active(ci, LEAP_BACK) {
                    set_hit_frames(s, ci, false);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        // the detecting dash hit: leap back, and the shield with A1's
        // absorbed element 11 frames later
        if cbs & CB_LEAP != 0 {
            s.add_status(ci, LEAP_BACK, g::LEAP_BACK_DURATION, true);
            set_hit_frames(s, ci, true);
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_LEAP_END, arg: 0 }, g::LEAP_BACK_DURATION + 1);
            let ele = absorb_a1(s, ci);
            s.st_mut::<State>(ci).absorbed = ele;
            s.add_task(Task::Char { c: ci as u8, id: TASK_HIT_SHIELD, arg: 0 }, g::HIT_GENERATE_SHIELD);
        }
        if cbs & CB_PARTICLE != 0 && !s.st::<State>(ci).particle_generated {
            s.st_mut::<State>(ci).particle_generated = true;
            s.queue_particle(3.0, Element::Anemo, s.particle_delay[ci]);
        }
    }

    /// C2: the active member's normal attack hit restores 40% of the
    /// shield, once in 2 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if !has_shield(s) || ev.info.actor != s.player.active || ev.info.tag != AttackTag::Normal || s.mod_active(ci, C2_ICD) {
            return;
        }
        s.add_status(ci, C2_ICD, 2 * 60, true);
        restore_shield(s, ci, 0.4);
    }
}

/// Sets or clears the hit-frames bit of her running skill.
fn set_hit_frames(s: &mut Sim, ci: usize, hit: bool) {
    if let Some(a) = &mut s.player.anim {
        if a.char as usize == ci {
            if let Frames::Char { id: FR_SKILL, arg } = &mut a.frames {
                if hit {
                    *arg |= FR_HIT;
                } else {
                    *arg &= !FR_HIT;
                }
            }
        }
    }
}

/// A1: the element of a circle of 4 on the player (pyro, hydro, electro,
/// cryo), anemo without one.
fn absorb_a1(s: &Sim, ci: usize) -> Element {
    if s.chars[ci].ascension < 1 {
        return Element::Anemo;
    }
    let p = s.circle_on_player(0.0, 0.0, 4.0);
    match s.absorb_check_in(p, &[Element::Pyro, Element::Hydro, Element::Electro, Element::Cryo]) {
        Element::NoElement => Element::Anemo,
        e => e,
    }
}

fn shield_hp(s: &Sim, ci: usize) -> f64 {
    let lvl = s.talent(ci, 1);
    g::SHIELD_AMT[lvl] * s.total_atk(ci) + g::SHIELD_FLAT[lvl]
}

fn has_shield(s: &Sim) -> bool {
    s.shields.iter().any(|x| x.typ == ShieldType::LanyanShield)
}

fn gen_shield(s: &mut Sim, ci: usize, ele: Element) {
    let hp = shield_hp(s, ci);
    let expiry = s.f + 750;
    s.add_shield(Shield { owner: ci as u8, key: SHIELD, typ: ShieldType::LanyanShield, target: -1, ele, hp, expiry, em_bonus: 0.0 });
}

/// The shield back up by a share of its full HP, with its element and
/// expiry.
fn restore_shield(s: &mut Sim, ci: usize, pct: f64) {
    let Some(old) = s.shields.iter().find(|x| x.typ == ShieldType::LanyanShield).copied() else { return };
    let full = shield_hp(s, ci);
    let hp = (full * pct + old.hp).min(full);
    s.add_shield(Shield { hp, ..old });
}

/// The Feathermoon Rings: three hits on the enemy, each with half of it
/// in the absorbed element (A1), and C1's three more.
fn rings(s: &mut Sim, ci: usize) -> ActInfo {
    s.delete_mod(ci, LEAP_BACK);
    ring_hits(s, ci, 0, g::RING_HITMARKS);
    let absorbed = s.st::<State>(ci).absorbed;
    if s.chars[ci].cons >= 1 && s.chars[ci].ascension >= 1 && absorbed != Element::Anemo {
        ring_hits(s, ci, 1, g::C1_HITMARKS);
    }
    let t = &g::RINGS_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Attack as usize], AnimState::Skill)
}

const RING_NAMES: [&str; 2] = ["Feathermoon Ring", "Feathermoon Ring (C1)"];
/// The absorbed part's names: ring kind, then pyro, hydro, electro, cryo.
const RING_MIX_NAMES: [[&str; 4]; 2] = [
    ["Feathermoon Ring (pyro)", "Feathermoon Ring (hydro)", "Feathermoon Ring (electro)", "Feathermoon Ring (cryo)"],
    ["Feathermoon Ring (C1) (pyro)", "Feathermoon Ring (C1) (hydro)", "Feathermoon Ring (C1) (electro)", "Feathermoon Ring (C1) (cryo)"],
];

fn ring_hits(s: &mut Sim, ci: usize, kind: usize, hitmarks: &[i32]) {
    let lvl = s.talent(ci, 1);
    let absorbed = s.st::<State>(ci).absorbed;
    let flat = if s.chars[ci].ascension >= 4 { s.stat(ci, st::EM) * 3.09 } else { 0.0 };
    for &h in hitmarks {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_LANYAN_RING_ATTACK, icd::GROUP_LANYAN_RING_ATTACK, Element::Anemo, 25.0, g::RING[lvl]);
        ai.abil = RING_NAMES[kind];
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.flat_dmg = flat;
        ai.pattern = Pattern::Single;
        s.queue_attack(ai, CB_PARTICLE, h, h);
        if s.chars[ci].ascension >= 1 && absorbed != Element::Anemo {
            let k = match absorbed {
                Element::Pyro => 0,
                Element::Hydro => 1,
                Element::Electro => 2,
                _ => 3,
            };
            ai.abil = RING_MIX_NAMES[kind][k];
            ai.icd_tag = icd::TAG_LANYAN_RING_ATTACK_MIX;
            ai.icd_group = icd::GROUP_LANYAN_RING_ATTACK_MIX;
            ai.element = absorbed;
            ai.mult *= 0.5;
            s.queue_attack(ai, CB_PARTICLE, h, h);
        }
        // the next ring goes to another enemy within 8, of which there is
        // none
    }
}
