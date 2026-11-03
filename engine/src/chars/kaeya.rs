//! Kaeya, from gcsim's internal/characters/kaeya: normal and charged
//! attacks, the skill with A1's heal and A4's particles, the burst's
//! icicles (a status of the whole fight; C6's fourth icicle and energy),
//! the utility dash's stamina, every constellation (C2 extends the burst on
//! a kill, in damage mode; C4's shield on a drain below 20% HP), and the
//! plunges from Xianyun's airborne state.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::kaeya as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The icicles' attack, snapshot as the burst is cast.
    snap: AttackEvent,
    /// The frame of the last burst: older icicles stop (gcsim's
    /// burstTickSrc).
    tick_src: i32,
    c2_count: i32,
    c4_icd: i32,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const C1: ModKey = ModKey::new("kaeya-c1");
const C4_SHIELD: ModKey = ModKey::new("kaeya-c4");

// callbacks, in the order gcsim gives them to the skill
const CB_A1: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;
const CB_A4: u16 = 1 << 2;

const TASK_ICICLE: u8 = 0;
const TASK_C6_ENERGY: u8 = 1;

const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];

const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

/// gcsim's burstTickerFunc: an icicle hit while the burst lasts, then the
/// next 2 s later.
fn icicle(s: &mut Sim, ci: usize, src: i32) {
    if s.core_status_key(BURST) == 0 || s.st::<State>(ci).tick_src != src {
        return;
    }
    let mut ev = s.st::<State>(ci).snap;
    ev.info.pattern = s.circle_on_player(0.0, 0.0, 4.0);
    s.queue_attack_with_snap(ev, 0, 0);
    s.add_task(Task::Char { c: ci as u8, id: TASK_ICICLE, arg: src }, 120);
}

impl Character for Char {
    const KEY: &'static str = "kaeya";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { snap: AttackEvent::new(AttackInfo::new(0, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 0.0, 0.0), 0), tick_src: 0, c2_count: 0, c4_icd: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        // gcsim's "utility-dash" stamina mod, one key for everyone
        s.utility_dash = Some(ci as u8);
        let cons = s.chars[ci].cons;
        if cons >= 1 {
            s.add_mod(ci, Mod::owned(C1, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1), -1);
        }
        if cons >= 2 {
            s.subscribe(EventKind::TargetDied, Owner::Char(ci as u8));
        }
        if cons >= 4 {
            s.subscribe(EventKind::Drain, Owner::Char(ci as u8));
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

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::AUTO[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        let (off, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
        ai.pattern = if n == 2 || n == 3 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::CHARGE.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = CHARGE_NAMES[i];
            ai.pattern = s.circle_on_player(0.0, g::CHARGE_OFFSETS[i], 2.2);
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 50.0, g::SKILL[lvl]);
        ai.abil = "Frostgnaw";
        ai.pattern = s.box_on_player(0.0, -0.2, 4.0, 8.0);
        let mut cbs = CB_A1 | CB_PARTICLE;
        if s.chars[ci].ascension >= 4 {
            cbs |= CB_A4;
        }
        s.queue_attack(ai, cbs, 0, g::SKILL_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 360, 25);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST[lvl]);
        ai.abil = "Glacial Waltz";
        let snap = s.snapshot_event(ai, 0);
        s.add_core_status(BURST, g::BURST_DURATION + g::BURST_HITMARK + 1);
        let count = if s.chars[ci].cons == 6 { 4 } else { 3 };
        let offset = 120 / count;
        let src = s.f;
        let state = st_mut(s, ci);
        state.snap = snap;
        state.tick_src = src;
        for i in 0..count {
            s.add_task(Task::Char { c: ci as u8, id: TASK_ICICLE, arg: src }, g::BURST_HITMARK + offset * i);
        }
        s.consume_energy(ci, 51);
        if s.chars[ci].cons >= 6 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_C6_ENERGY, arg: 0 }, 52);
        }
        s.set_cd_with_delay(ci, Action::Burst, 900, g::BURST_CD_START);
        st_mut(s, ci).c2_count = 0;
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_ICICLE => icicle(s, ci, arg),
            TASK_C6_ENERGY => s.add_energy(ci, 15.0),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        // A1: the active character heals 15% of the skill's ATK
        if cbs & CB_A1 != 0 && s.chars[ci].ascension >= 1 {
            let active = s.player.active as usize;
            let src = total_atk(&ev.stats) * 0.15;
            let bonus = s.stat(ci, st::HEAL);
            s.heal(ci, active, src, bonus);
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, true);
            let count = if s.rng.float64() < 0.67 { 3.0 } else { 2.0 };
            s.queue_particle(count, Element::Cryo, s.particle_delay[ci]);
        }
        // A4: a particle from a frozen enemy (at most two a skill; the
        // skill hits the one enemy once)
        if cbs & CB_A4 != 0 && s.enemy.aura.get(AURA_FROZEN) > ZERO_DUR {
            s.queue_particle(1.0, Element::Cryo, s.particle_delay[ci]);
        }
    }

    /// C1: CRIT Rate for normal and charged attacks on an enemy with cryo
    /// or frozen.
    fn attack_mod(s: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) && s.enemy_aura_contains(Element::Cryo) {
            ev.stats[st::CR] += 0.15;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // C2: a kill while the burst lasts extends it, three times a burst
            Event::TargetDied(_) => {
                let c2 = s.st::<State>(ci).c2_count;
                if s.core_status_key(BURST) == 0 || c2 > 2 {
                    return;
                }
                let ext = if c2 == 2 { 120 } else { 150 };
                s.extend_core_status(BURST, ext);
                st_mut(s, ci).c2_count += 1;
            }
            // C4: a shield of 30% max HP below 20% HP, once every 60 s
            Event::Drain { amount, .. } => {
                let icd = s.st::<State>(ci).c4_icd;
                if amount <= 0.0 || (s.f < icd && icd != 0) {
                    return;
                }
                if s.current_hp(ci) / s.max_hp(ci) < 0.2 {
                    let f = s.f;
                    st_mut(s, ci).c4_icd = f + 3600;
                    let hp = 0.3 * s.max_hp(ci);
                    s.add_shield(Shield { owner: ci as u8, key: C4_SHIELD, typ: ShieldType::KaeyaC4, target: ci as i8, ele: Element::Cryo, hp, expiry: f + 1200, em_bonus: 0.0 });
                }
            }
            _ => {}
        }
    }
}
