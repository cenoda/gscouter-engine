//! Jean, from gcsim's internal/characters/jean: normal and charged
//! attacks, the skill (pressed or held), the burst with its field's heals
//! and self swirl (gcsim's ReactWithSelf), A1, A4 and constellations 1
//! to 6. No plunges (gcsim's package has none). C6 (less damage taken in
//! the field) does nothing, as in gcsim, which logs it as not implemented.

use crate::content::{Character, Event, EventKind, Owner};
use crate::gcsimdata::chars::jean as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The burst's heals: initial, then per tick, with the healing bonus
    /// it snapshot.
    heal: f64,
    heal_dot: f64,
    heal_bonus: f64,
}

const BURST: ModKey = ModKey::new("jean-q");
const BASE_PARTICLE_ICD: ModKey = ModKey::new(g::BASE_PARTICLE_ICD_KEY);
const EXTRA_PARTICLE_ICD: ModKey = ModKey::new(g::EXTRA_PARTICLE_ICD_KEY);
const C2: ModKey = ModKey::new("jean-c2");
const C4: ModKey = ModKey::new("jean-c4");

const CB_PARTICLES: u16 = 1 << 0;
const CB_A1: u16 = 1 << 1;

const TASK_HEAL: u8 = 0;
const TASK_FIELD: u8 = 1;
const TASK_A4: u8 = 2;
const TASK_C4: u8 = 3;

impl Character for Char {
    const KEY: &'static str = "jean";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        // C2: OnParticleReceived
        if s.chars[ci].cons >= 2 {
            s.subscribe(EventKind::ParticleReceived, Owner::Char(ci as u8));
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::AUTO[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        ai.pattern = s.fan_on_player(0.0, g::ATTACK_OFFSETS[n], g::ATTACK_RADIUS[n], g::ATTACK_FAN_ANGLES[n]);
        let a1 = if s.chars[ci].ascension >= 1 { CB_A1 } else { 0 };
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, a1, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge";
        ai.pattern = s.fan_on_player(0.0, 1.0, 1.8, 160.0);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(0).min(300) as i32;
        let hitmark = g::SKILL_HITMARK + hold;
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 50.0, g::SKILL[lvl]);
        ai.abil = "Gale Blade";
        ai.pattern = s.box_on_player(0.0, 0.0, 4.0, 4.1);
        // snapshot as the skill is used
        let mut ev = s.snapshot_event(ai, CB_PARTICLES);
        // C1: held for 1 s or more, 40% more damage
        if s.chars[ci].cons >= 1 && p.get("hold").unwrap_or(0) >= 60 {
            ev.stats[st::DMGP] += 0.4;
        }
        s.queue_attack_with_snap(ev, CB_PARTICLES, hitmark);
        s.set_cd_with_delay(ci, Action::Skill, 360, hitmark - 2);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Char { id: 0, arg: hold as i16 }, t[0] + hold, t[Action::Dash as usize] + hold, AnimState::Skill)
    }

    fn frames(_: u8, hold: i16, next: Action) -> i32 {
        g::SKILL_FRAMES[next as usize] + hold as i32
    }

    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let enter = p.get("enter").unwrap_or(0).max(1) as i32;
        let delay = p.get("enter_delay").map_or(600 / enter, |d| d as i32);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 50.0, g::BURST[lvl]);
        ai.abil = "Dandelion Breeze";
        ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
        let ev = s.snapshot_event(ai, 0);
        s.queue_attack_with_snap(ev, 0, g::BURST_START + 15);
        s.add_core_status(BURST, 600 + g::BURST_START);
        let mut ev = ev;
        ev.info.abil = "Dandelion Breeze (In/Out)";
        ev.info.mult = g::BURST_ENTER[lvl];
        ev.info.pattern = s.circle_on_target(6.0);
        for i in 0..enter {
            s.queue_attack_with_snap(ev, 0, g::BURST_START + i * delay);
        }
        s.queue_attack_with_snap(ev, 0, 600 + g::BURST_START);
        let atk = total_atk(&ev.stats);
        let st = s.st_mut::<State>(ci);
        st.heal_bonus = ev.stats[st::HEAL];
        st.heal = g::BURST_INITIAL_HEAL_FLAT[lvl] + atk * g::BURST_INITIAL_HEAL_PER[lvl];
        st.heal_dot = g::BURST_DOT_HEAL_FLAT[lvl] + atk * g::BURST_DOT_HEAL_PER[lvl];
        s.add_task(Task::Char { c: ci as u8, id: TASK_HEAL, arg: 0 }, g::BURST_START);
        // C4 also applies once right before the burst starts
        if s.chars[ci].cons >= 4 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_C4, arg: 0 }, g::BURST_START - 1);
        }
        // the field ticks every second; the player stays in it
        let mut i = 100;
        while i <= 600 + g::BURST_START {
            s.add_task(Task::Char { c: ci as u8, id: TASK_FIELD, arg: 0 }, i);
            i += 60;
        }
        s.consume_energy(ci, 41);
        s.set_cd_with_delay(ci, Action::Burst, 1200, 38);
        s.add_task(Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, g::BURST_START + 1);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        let st = s.st::<State>(ci);
        let (heal, heal_dot, bonus) = (st.heal, st.heal_dot, st.heal_bonus);
        match id {
            TASK_HEAL => s.heal_info(ci, None, false, heal, bonus),
            TASK_FIELD => {
                let active = s.player.active as usize;
                s.heal(ci, active, heal_dot, bonus);
                let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, 0.0);
                ai.abil = "Dandelion Breeze (Self Swirl)";
                s.react_with_self(ai);
                if s.chars[ci].cons >= 4 {
                    c4(s);
                }
            }
            TASK_C4 => c4(s),
            // A4: 20% of the burst's energy back
            TASK_A4 => {
                if s.chars[ci].ascension >= 4 {
                    s.add_energy(ci, 16.0);
                }
            }
            _ => unreachable!(),
        }
    }

    /// C2: when Jean is on the field as the party takes a particle, every
    /// member gets ATK SPD +15% for 15 s, extended by its hitlag.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::ParticleReceived = *e else { return };
        if s.player.active as usize != ci {
            return;
        }
        for i in 0..s.chars.len() {
            s.add_stat_mod(i, C2, st::ATKSPD, 0.15, 900, true);
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLES != 0 {
            if !s.mod_active(ci, BASE_PARTICLE_ICD) {
                s.add_status(ci, BASE_PARTICLE_ICD, 18, true);
                s.queue_particle(2.0, Element::Anemo, s.particle_delay[ci]);
            }
            if !s.mod_active(ci, EXTRA_PARTICLE_ICD) {
                s.add_status(ci, EXTRA_PARTICLE_ICD, 60, true);
                if s.rng.float64() < 0.67 {
                    s.queue_particle(1.0, Element::Anemo, s.particle_delay[ci]);
                }
            }
        }
        // A1: half the time, the party heals 15% of the hit's ATK
        if cbs & CB_A1 != 0 && s.rng.float64() < 0.5 {
            let bonus = s.stat(ci, st::HEAL);
            s.heal_info(ci, None, false, total_atk(&ev.stats) * 0.15, bonus);
        }
    }
}

/// C4: the enemy in the field (its center in a circle of radius 6 on the
/// player, EnemiesWithinArea) takes
/// Anemo RES -40% for 1.2 s, hitlag affected; renewed with each heal tick.
fn c4(s: &mut Sim) {
    let area = s.circle_on_player(0.0, 0.0, 6.0);
    if s.enemy_within(area) {
        s.enemy_add_resist(C4, Element::Anemo, -0.4, 72);
    }
}
