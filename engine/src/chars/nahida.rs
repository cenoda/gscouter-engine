//! Nahida, from gcsim's internal/characters/nahida: normal and charged
//! attacks, the skill pressed or held with its Seeds of Skandha and
//! Tri-Karma Purification, the burst's Shrine of Maya with its pyro,
//! electro and hydro bonuses, A1 and A4. No constellations.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::nahida as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    tri_karma_interval: i32,
    burst_src: i32,
    mark_count: i32,
    /// Pyro, electro and hydro characters in the team, at most 2 each.
    pyro: usize,
    electro: usize,
    hydro: usize,
    /// The burst's DMG bonus to Tri-Karma with pyro characters.
    pyro_buff: f64,
    /// A1's EM, taken when the burst starts.
    a1_em: f64,
    /// A4's DMG bonus and CRIT Rate to Tri-Karma, taken every 0.5 s.
    a4_dmg: f64,
    a4_cr: f64,
}

const MARK: ModKey = ModKey::new(g::SKILL_MARK_KEY);
const SKILL_ICD: ModKey = ModKey::new(g::SKILL_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::TRI_KARMA_PARTICLE_ICD);
const WITHIN_BURST: ModKey = ModKey::new(g::WITHIN_BURST_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const A1: ModKey = ModKey::new(g::A1_BUFF_KEY);
const A4: ModKey = ModKey::new(g::A4_BUFF_KEY);

const CB_MARK: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;

const TASK_INTERVAL: u8 = 0;
const TASK_A4: u8 = 1;
const TASK_BURST_START: u8 = 2;
// arg: the burst's frame
const TASK_BURST_TICK: u8 = 3;

impl Character for Char {
    const KEY: &'static str = "nahida";
    const MAX_CONS: i32 = 0;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        s.subscribe(EventKind::EnemyDamage, o);
        s.subscribe(EventKind::Reaction, o);
        update_interval(s, ci);
        let count = |e: Element| s.chars.iter().filter(|c| c.element == e).count().min(2);
        let (pyro, electro, hydro) = (count(Element::Pyro), count(Element::Electro), count(Element::Hydro));
        let lvl = s.talent(ci, 2);
        let st = s.st_mut::<State>(ci);
        st.pyro = pyro;
        st.electro = electro;
        st.hydro = hydro;
        if pyro > 0 {
            st.pyro_buff = g::BURST_TRI_KARMA_DMG_BONUS[pyro - 1][lvl];
        }
        if s.chars[ci].ascension >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Attack, o, st::NONE, false, false, -1));
            a4_tick(s, ci);
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
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        // a box on the target
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], table[Action::Swap as usize], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        let windup = if s.current_state() == AnimState::Normal { 10 } else { 0 };
        // a 6 by 6 box 3 short of the target
        let h = g::CHARGE_HITMARK - windup;
        s.queue_attack(ai, 0, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: 0, arg: windup as i16 }, t[0] - windup, t[Action::Dash as usize] - windup, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        s.st_mut::<State>(ci).mark_count = 0;
        let lvl = s.talent(ci, 1);
        let hold = p.get("hold").unwrap_or(0);
        if hold == 0 {
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::SKILL_PRESS[lvl]);
            ai.abil = "All Schemes to Know (Press)";
            s.queue_attack(ai, CB_MARK, 0, g::SKILL_PRESS_HITMARK);
            s.set_cd_with_delay(ci, Action::Skill, g::SKILL_PRESS_CD, 11);
            let t = &g::SKILL_PRESS_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill);
        }
        let hold = (hold.clamp(1, 300) - 1) as i32;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::SKILL_HOLD[lvl]);
        ai.abil = "All Schemes to Know (Hold)";
        s.queue_attack(ai, CB_MARK, 0, hold + 3 + 30);
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_HOLD_CD, hold + 30);
        let t = &g::SKILL_HOLD_FRAMES;
        ActInfo::new(Frames::Char { id: 1, arg: hold as i16 }, hold + t[0], hold + t[Action::Swap as usize], AnimState::Skill)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            0 => g::CHARGE_FRAMES[next as usize] - arg as i32,
            _ => arg as i32 + g::SKILL_HOLD_FRAMES[next as usize],
        }
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_START, arg: 0 }, 66);
        s.consume_energy(ci, 5);
        s.set_cd(ci, Action::Burst, 810);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_INTERVAL => update_interval(s, ci),
            TASK_A4 => a4_tick(s, ci),
            TASK_BURST_START => {
                let f = s.f;
                let hydro = s.st::<State>(ci).hydro;
                let mut dur = 15.0;
                if hydro > 0 {
                    dur += g::BURST_TRI_KARMA_DURATION_EXTEND[hydro - 1][s.talent(ci, 2)];
                }
                let frames = (dur * 60.0) as i32;
                s.st_mut::<State>(ci).burst_src = f;
                s.add_core_status(BURST, frames);
                // A1 is taken as the burst starts
                if s.chars[ci].ascension >= 1 {
                    let max = (0..s.chars.len()).map(|i| s.stat_ex(i, st::EM, true)).fold(0.0, f64::max);
                    s.st_mut::<State>(ci).a1_em = (0.25 * max).min(250.0);
                }
                let mut i = 30;
                while i <= frames {
                    s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_TICK, arg: f }, i);
                    i += 30;
                }
            }
            TASK_BURST_TICK => {
                // the player stands in the Shrine of Maya
                if arg != s.st::<State>(ci).burst_src {
                    return;
                }
                s.add_status(ci, WITHIN_BURST, 60, true);
                if s.st::<State>(ci).pyro > 0 {
                    s.add_mod(ci, Mod::owned(BURST, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 60);
                }
                if s.chars[ci].ascension >= 1 {
                    for i in 0..s.chars.len() {
                        s.add_mod(i, Mod::owned(A1, ModKind::Stat, Owner::Char(ci as u8), st::EM, true, false, -1), 60);
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    /// A1: EM to the active character in the field.
    fn mod_amount(s: &Sim, ci: usize, on: usize, _: &Mod, out: &mut Stats) -> bool {
        if s.player.active as usize != on {
            return false;
        }
        out[st::EM] += s.st::<State>(ci).a1_em;
        true
    }

    /// The burst's pyro bonus and A4, on Tri-Karma Purification.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        let st = s.st::<State>(ci);
        if m.key == BURST {
            if ev.info.abil == g::TRI_KARMA_ABIL {
                ev.stats[st::DMGP] += st.pyro_buff;
            }
            return;
        }
        if ev.info.tag == AttackTag::ElementalArt && ev.info.abil.starts_with("Tri-Karma") {
            ev.stats[st::DMGP] += st.a4_dmg;
            ev.stats[st::CR] += st.a4_cr;
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_MARK != 0 && s.st::<State>(ci).mark_count < 8 {
            s.enemy_add_status(MARK, 1500, true);
            s.st_mut::<State>(ci).mark_count += 1;
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 7 * 60, true);
            s.queue_particle(3.0, Element::Dendro, s.particle_delay[ci]);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // bloom, hyperbloom and burgeon damage
            Event::EnemyDamage(ev) if matches!(ev.info.tag, AttackTag::Bloom | AttackTag::Hyperbloom | AttackTag::Burgeon) => tri_karma(s, ci),
            // any reaction on the enemy (hyperbloom and burgeon are on the
            // dendro core)
            Event::Reaction(r, _) if !matches!(r, Reaction::Hyperbloom | Reaction::Burgeon) => tri_karma(s, ci),
            _ => {}
        }
    }
}

/// Tri-Karma Purification on the marked enemy, at most once per interval.
fn tri_karma(s: &mut Sim, ci: usize) {
    if s.mod_active(ci, SKILL_ICD) || !s.enemy_status_active(MARK) {
        return;
    }
    let interval = s.st::<State>(ci).tri_karma_interval;
    s.add_status(ci, SKILL_ICD, interval, true);
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NAHIDA_SKILL, icd::GROUP_NAHIDA_SKILL, Element::Dendro, 25.0, g::TRI_KARMA_ATK[lvl]);
    ai.abil = g::TRI_KARMA_ABIL;
    let mut ev = s.snapshot_event(ai, CB_PARTICLE);
    ev.info.flat_dmg = ev.stats[st::EM] * g::TRI_KARMA_EM[lvl];
    s.queue_attack_with_snap(ev, CB_PARTICLE, 3);
}

/// updateTriKarmaInterval: every second, the interval with the burst's
/// electro reduction while the player is in the field.
fn update_interval(s: &mut Sim, ci: usize) {
    let mut cd = 150;
    let electro = s.st::<State>(ci).electro;
    if electro > 0 && s.mod_active(ci, WITHIN_BURST) {
        cd -= (g::BURST_TRI_KARMA_CD_REDUCTION[electro - 1][s.talent(ci, 2)] * 60.0) as i32;
    }
    s.st_mut::<State>(ci).tri_karma_interval = cd;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_INTERVAL, arg: 0 }, 60);
}

/// a4Tick: A4's bonuses from Nahida's EM above 200, every 0.5 s.
fn a4_tick(s: &mut Sim, ci: usize) {
    let em = s.stat(ci, st::EM);
    let (mut dmg, mut cr) = (0.0, 0.0);
    if em > 200.0 {
        dmg = ((em - 200.0) * 0.001).min(0.8);
        cr = ((em - 200.0) * 0.0003).min(0.24);
    }
    let st = s.st_mut::<State>(ci);
    st.a4_dmg = dmg;
    st.a4_cr = cr;
    s.add_task(Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, 30);
}
