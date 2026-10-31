//! Raiden Shogun, from gcsim's internal/characters/raiden: normal and
//! charged attacks with and without Musou Isshin, the Eye of Stormy
//! Judgment and its coordinated attacks, the burst with its Resolve, A1,
//! A4 and every constellation. Plunges are not implemented.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::raiden as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    a4_electro: f64,
    burst_cast_f: i32,
    eye_icd: i32,
    stacks_consumed: f64,
    stacks: f64,
    restore_icd: i32,
    restore_count: i32,
    apply_c4: bool,
    c6_count: i32,
    c6_icd: i32,
    burst_anim_src: i32,
    a1_icd: i32,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const EYE: ModKey = ModKey::new(g::SKILL_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A4: ModKey = ModKey::new("raiden-a4");
const C4: ModKey = ModKey::new("raiden-c4");

const CB_PARTICLE: u16 = 1 << 0;
const CB_RESTORE: u16 = 1 << 1;
const CB_C6: u16 = 1 << 2;

// arg: normal counter * 8 + hit
const TASK_NORMAL: u8 = 0;
const TASK_SWORD: u8 = 1;
// arg: hit; the mult rides in the task's slot
const TASK_SWORD_CA: u8 = 2;
// arg: the character given the eye
const TASK_EYE: u8 = 3;
const TASK_BURST_HIT: u8 = 4;
// arg: the burst's frame
const TASK_C4: u8 = 5;
const TASK_A4: u8 = 6;

impl Character for Char {
    const KEY: &'static str = "raidenshogun";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { burst_anim_src: -1, ..Default::default() }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // eyeOnDamage, a1, a4, onBurstStackCount, onSwapClearBurst
        s.subscribe(EventKind::EnemyDamage, o);
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::ParticleReceived, o);
        }
        if s.chars[ci].ascension >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Stat, o, st::ELECTROP, true, false, -1));
            a4_tick(s, ci);
        }
        s.subscribe(EventKind::EnergyBurst, o);
        s.subscribe(EventKind::Swap, o);
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn charge_stam(s: &Sim, ci: usize) -> Option<f64> {
        Some(if s.mod_active(ci, BURST) { 20.0 } else { 25.0 })
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == A4 {
            out[st::ELECTROP] += s.st::<State>(ci).a4_electro;
        } else {
            out[st::ATKP] += 0.3;
        }
        true
    }

    /// The eye: burst DMG from each character's energy cost.
    fn attack_mod(s: &Sim, ci: usize, on: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::ElementalBurst {
            ev.stats[st::DMGP] += g::SKILL_BURST_BONUS[s.talent(ci, 1)] * s.chars[on].energy_max;
        }
    }

    /// C2: Musou Isshin's attacks ignore 60% DEF.
    fn own_hit(s: &Sim, ci: usize, ev: &mut AttackEvent) {
        if s.chars[ci].cons >= 2 && (s.mod_active(ci, BURST) || s.player.state_start == s.st::<State>(ci).burst_anim_src) {
            ev.info.ignore_def += 0.6;
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let sword = s.mod_active(ci, BURST);
        let (hitmarks, table, id) = if sword {
            let f = s.f;
            s.st_mut::<State>(ci).burst_anim_src = f;
            (g::SWORD_HITMARKS[n], &g::SWORD_FRAMES[n], TASK_SWORD)
        } else {
            (g::ATTACK_HITMARKS[n], &g::ATTACK_FRAMES[n], TASK_NORMAL)
        };
        for (i, &h) in hitmarks.iter().enumerate() {
            // Musou Isshin's mult comes from the Resolve when the attack starts
            let slot = if sword { s.alloc(AttackEvent::new(sword_info(s, ci, n, i), CB_RESTORE | CB_C6)) as i32 } else { 0 };
            s.queue_char_task(ci, Task::Char { c: ci as u8, id, arg: if sword { slot } else { (n * 8 + i) as i32 } }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *hitmarks.last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, BURST) {
            let f = s.f;
            s.st_mut::<State>(ci).burst_anim_src = f;
            let (lvl, consumed) = (s.talent(ci, 2), s.st::<State>(ci).stacks_consumed);
            for (i, mult) in g::CHARGE_SWORD.iter().enumerate() {
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Electro, 25.0, mult[lvl] + g::RESOLVE_BONUS[lvl] * consumed);
                ai.abil = ["Musou Isshin (Charge Attack 0)", "Musou Isshin (Charge Attack 1)"][i];
                // only the last hit has hitlag
                if i == 1 {
                    ai.hitlag_halt = 0.02 * 60.0;
                    ai.def_halt = true;
                }
                ai.hitlag_factor = 0.01;
                let slot = s.alloc(AttackEvent::new(ai, CB_RESTORE | CB_C6)) as i32;
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SWORD_CA, arg: slot }, g::SWORD_CA_HITMARKS[i]);
            }
            let last = *g::SWORD_CA_HITMARKS.last().unwrap();
            return ActInfo::new(Frames::Table(&g::SWORD_CA_FRAMES), g::SWORD_CA_FRAMES[0], last, AnimState::Charge);
        }
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.hitlag_halt = 0.02 * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        ActInfo::new(Frames::Table(&g::CHARGE_FRAMES), g::CHARGE_FRAMES[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL[lvl]);
        ai.abil = "Eye of Stormy Judgement";
        s.queue_attack(ai, 0, g::SKILL_HITMARK, g::SKILL_HITMARK);
        // the eye comes a second after the cooldown starts
        for i in 0..s.chars.len() {
            s.add_task(Task::Char { c: ci as u8, id: TASK_EYE, arg: i as i32 }, 6 + 60);
        }
        s.set_cd_with_delay(ci, Action::Skill, 600, 6);
        ActInfo::new(Frames::Table(&g::SKILL_FRAMES), g::SKILL_FRAMES[0], g::SKILL_FRAMES[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        st.burst_anim_src = f;
        st.burst_cast_f = f;
        st.restore_count = 0;
        st.restore_icd = 0;
        st.c6_count = 0;
        st.c6_icd = 0;
        s.add_status(ci, BURST, 420 + g::BURST_HITMARK, true);
        if s.chars[ci].cons >= 4 {
            s.st_mut::<State>(ci).apply_c4 = true;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C4, arg: f }, 420 + g::BURST_HITMARK);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_HIT, arg: 0 }, g::BURST_HITMARK);
        s.set_cd(ci, Action::Burst, 18 * 60);
        s.consume_energy(ci, 8);
        ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_FRAMES[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = ((arg / 8) as usize, (arg % 8) as usize);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.hitlag_factor = 0.01;
                ai.def_halt = g::ATTACK_DEF_HALT[n][i];
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_SWORD | TASK_SWORD_CA => {
                // the attack built when the action started, snapshot now
                s.queue_attack_slot(arg as u16, 0, 0);
            }
            TASK_EYE => {
                let mut m = Mod::owned(EYE, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1);
                m.value = 0.0;
                s.add_mod(arg as usize, m, 1500);
            }
            TASK_BURST_HIT => {
                let st = s.st_mut::<State>(ci);
                st.stacks_consumed = st.stacks;
                st.stacks = 0.0;
                let consumed = st.stacks_consumed;
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Electro, 50.0, g::BURST_BASE[lvl]);
                ai.abil = "Musou Shinsetsu";
                ai.mult += g::RESOLVE_BASE_BONUS[lvl] * consumed;
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_C4 => {
                let st = s.st_mut::<State>(ci);
                if arg == st.burst_cast_f && st.apply_c4 {
                    st.apply_c4 = false;
                    c4(s, ci);
                }
            }
            TASK_A4 => a4_tick(s, ci),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 48, true);
            if s.rng.float64() < 0.5 {
                s.queue_particle(1.0, Element::Electro, s.particle_delay[ci]);
            }
        }
        if cbs & CB_RESTORE != 0 {
            burst_restore(s, ci);
        }
        if cbs & CB_C6 != 0 {
            c6(s, ci);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyDamage(ev) => eye_on_damage(s, ci, ev),
            Event::ParticleReceived => {
                // A1: 2 Resolve once every 3 s
                let f = s.f;
                let st = s.st_mut::<State>(ci);
                if st.a1_icd > f {
                    return;
                }
                st.a1_icd = f + 180;
                st.stacks = (st.stacks + 2.0).min(60.0);
            }
            Event::EnergyBurst { on, amount } => {
                if s.player.active as usize == ci {
                    return;
                }
                let mut stacks = g::RESOLVE_STACK_GAIN[s.talent(ci, 2)] * amount;
                if s.chars[ci].cons > 0 {
                    stacks *= if s.chars[on].element == Element::Electro { 1.8 } else { 1.2 };
                }
                let st = s.st_mut::<State>(ci);
                st.stacks = (st.stacks + stacks).min(60.0);
            }
            Event::Swap { prev, .. } => {
                if !s.mod_active(ci, BURST) || prev != ci {
                    return;
                }
                s.delete_mod(ci, BURST);
                if s.st::<State>(ci).apply_c4 {
                    s.st_mut::<State>(ci).apply_c4 = false;
                    c4(s, ci);
                }
            }
            _ => {}
        }
    }
}

fn sword_info(s: &Sim, ci: usize, n: usize, i: usize) -> AttackInfo {
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::ATTACK_B[n][i][lvl]);
    ai.abil = ["Musou Isshin 0", "Musou Isshin 1", "Musou Isshin 2", "Musou Isshin 3", "Musou Isshin 4"][n];
    ai.mult += g::RESOLVE_BONUS[lvl] * s.st::<State>(ci).stacks_consumed;
    ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
    ai.hitlag_factor = 0.01;
    ai.def_halt = g::ATTACK_DEF_HALT[n][i];
    ai
}

/// The eye's coordinated attack, once every 0.9 s for the party, when a
/// character with the eye deals damage.
fn eye_on_damage(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    if s.st::<State>(ci).eye_icd > s.f || !s.mod_active(ev.info.actor as usize, EYE) {
        return;
    }
    // electro-charged, burning and hydro swirl come from the enemy, and the eye
    // does not trigger itself
    if matches!(ev.info.tag, AttackTag::ECDamage | AttackTag::BurningDamage | AttackTag::SwirlHydro) || (ev.info.actor as usize == ci && ev.info.abil == "Eye of Stormy Judgement (Strike)") || ev.damage == 0.0 {
        return;
    }
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL_TICK[lvl]);
    ai.abil = "Eye of Stormy Judgement (Strike)";
    s.queue_attack(ai, CB_PARTICLE, 5, 5);
    s.st_mut::<State>(ci).eye_icd = s.f + 54;
}

/// Musou Isshin's hits restore the party's energy, once a second, five
/// times.
fn burst_restore(s: &mut Sim, ci: usize) {
    let f = s.f;
    let st = s.st::<State>(ci);
    if f <= st.restore_icd || st.restore_count >= 5 {
        return;
    }
    let st = s.st_mut::<State>(ci);
    st.restore_count += 1;
    st.restore_icd = f + 60;
    let er = (s.stat_ex(ci, st::ER, true) - 1.0).max(0.0);
    let a4 = if s.chars[ci].ascension >= 4 { (er * 100.0) as i32 as f64 * 0.006 } else { 0.0 };
    let energy = g::BURST_RESTORE[s.talent(ci, 2)] * (1.0 + a4);
    for i in 0..s.chars.len() {
        s.add_energy(i, energy);
    }
}

/// A4: electro DMG from ER above 100%, updated every half second.
fn a4_tick(s: &mut Sim, ci: usize) {
    let v = (s.stat_ex(ci, st::ER, true) - 1.0).max(0.0) * 0.4;
    s.st_mut::<State>(ci).a4_electro = v;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, 30);
}

/// C4: when Musou Isshin ends, ATK +30% to the others for 10 s.
fn c4(s: &mut Sim, ci: usize) {
    for i in 0..s.chars.len() {
        if i != ci {
            s.add_mod(i, Mod::owned(C4, ModKind::Stat, Owner::Char(ci as u8), st::ATKP, false, true, -1), 600);
        }
    }
}

/// C6: Musou Isshin's hits shorten the others' burst cooldowns by 1 s,
/// once a second, five times.
fn c6(s: &mut Sim, ci: usize) {
    let f = s.f;
    let st = s.st::<State>(ci);
    if s.chars[ci].cons < 6 || f < st.c6_icd || st.c6_count == 5 {
        return;
    }
    let st = s.st_mut::<State>(ci);
    st.c6_icd = f + 60;
    st.c6_count += 1;
    for i in 0..s.chars.len() {
        if i != ci {
            s.reduce_cd(i, Action::Burst, 60);
        }
    }
}
