//! Sucrose, from gcsim's internal/characters/sucrose: normal and charged
//! attacks, her own dash frames, the skill, the burst with its absorbed
//! element, A1, A4 and every constellation. Her Hexerei bonuses need a
//! second Hexerei character, which is an error elsewhere. No plunges.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::sucrose as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The element the burst absorbed (NoElement before it does).
    absorb: Option<Element>,
    /// The absorbed hit, snapshot as the burst is used.
    absorb_snap: Option<AttackEvent>,
    /// A4's EM, taken when it triggers.
    a4_em: f64,
    /// A1's same-frame guard, by swirled element (pyro, hydro, electro,
    /// cryo).
    a1_icd: [i32; 4],
    c4_count: i32,
    /// C6's element.
    c6: Option<Element>,
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST: ModKey = ModKey::new("sucroseburst");
const A1: ModKey = ModKey::new("sucrose-a1");
const A4: ModKey = ModKey::new("sucrose-a4");
const C6: ModKey = ModKey::new("sucrose-c6");

const CB_A4: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;

const TASK_ABSORB_HIT: u8 = 0;
// arg: the checks done so far
const TASK_ABSORB_CHECK: u8 = 1;

impl Character for Char {
    const KEY: &'static str = "sucrose";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { a1_icd: [-1; 4], ..Default::default() }
    }

    fn init(s: &mut Sim, ci: usize) {
        // C1: two skill charges
        if s.chars[ci].cons >= 1 {
            let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
            cd.available = 2;
            cd.max_extra = 1;
        }
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::Reaction, Owner::Char(ci as u8));
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
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.circle_on_target(g::ATTACK_RADIUS[n]);
        let c4 = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, c4, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.pattern = s.box_on_player(0.0, -0.2, 3.2, 7.5);
        let windup = if matches!(s.current_state(), AnimState::Normal | AnimState::Charge) { 15 } else { 0 };
        let c4 = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
        let h = g::CHARGE_HITMARK - windup;
        s.queue_attack(ai, c4, h, h);
        ActInfo::new(Frames::Char { id: 0, arg: windup as i16 }, g::CHARGE_FRAMES[0] - windup, h, AnimState::Charge)
    }

    fn frames(_: u8, windup: i16, next: Action) -> i32 {
        g::CHARGE_FRAMES[next as usize] - windup as i32
    }

    fn dash_info(_: &mut Sim, _: usize, _: ActInfo) -> ActInfo {
        let t = &g::DASH_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Skill as usize], AnimState::Dash)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::SKILL[lvl]);
        ai.abil = "Astable Anemohypostasis Creation-6308";
        ai.pattern = s.circle_on_player(0.0, 5.0, 6.0);
        s.queue_attack(ai, CB_A4 | CB_PARTICLE, 0, 42);
        s.set_cd_with_delay(ci, Action::Skill, 900, 9);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let duration = if s.chars[ci].cons >= 2 { 480 } else { 360 };
        s.st_mut::<State>(ci).absorb = None;
        s.add_core_status(BURST, duration);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::BURST_DOT[lvl]);
        ai.abil = "Forbidden Creation-Isomer 75/Type II";
        ai.pattern = s.circle_on_player(0.0, 5.0, 8.0);
        let ev = s.snapshot_event(ai, CB_A4);
        // the absorbed hit's element is set when it lands; the snapshot's
        // stats do not depend on it
        let mut abs = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::NoElement, 25.0, g::BURST_ABSORB[lvl]);
        abs.abil = "Forbidden Creation-Isomer 75/Type II (Absorb)";
        abs.pattern = ai.pattern;
        let abs = s.snapshot_event(abs, 0);
        s.st_mut::<State>(ci).absorb_snap = Some(abs);
        let mut i = 137;
        while i <= duration + 5 {
            s.queue_attack_with_snap(ev, CB_A4, i);
            s.add_task(Task::Char { c: ci as u8, id: TASK_ABSORB_HIT, arg: 0 }, i);
            i += 113;
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_ABSORB_CHECK, arg: 0 }, 136);
        s.set_cd_with_delay(ci, Action::Burst, 1200, 18);
        s.consume_energy(ci, 21);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_ABSORB_HIT => {
                let st = s.st::<State>(ci);
                let (Some(ele), Some(mut ev)) = (st.absorb, st.absorb_snap) else { return };
                ev.info.element = ele;
                s.queue_attack_with_snap(ev, 0, 0);
            }
            // every 0.3 s until an element is absorbed or the burst ends
            TASK_ABSORB_CHECK => {
                let duration = if s.chars[ci].cons >= 2 { 480 } else { 360 };
                if arg == duration / 18 {
                    return;
                }
                // a 2.5 by 2.5 box 4 in front of the player: the enemy's
                // edge, not the player or Guoba
                let area = s.box_on_player(0.0, 4.0, 2.5, 2.5);
                let e = s.absorb_check_in(area, &[Element::Pyro, Element::Hydro, Element::Electro, Element::Cryo]);
                if e != Element::NoElement {
                    s.st_mut::<State>(ci).absorb = Some(e);
                    if s.chars[ci].cons >= 6 {
                        c6(s, ci, e);
                    }
                    return;
                }
                s.add_task(Task::Char { c: ci as u8, id: TASK_ABSORB_CHECK, arg: arg + 1 }, 18);
            }
            _ => unreachable!(),
        }
    }

    fn mod_amount(s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool {
        let st = s.st::<State>(ci);
        if m.key == A4 {
            out[st::EM] += st.a4_em;
            return true;
        }
        // C6: DMG bonus of the absorbed element, more for Hexerei
        let Some(e) = st.c6 else { return false };
        out[e.dmg_stat()] += 0.20 + if s.chars[on].hexerei { 0.085714 } else { 0.0 };
        true
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_A4 != 0 && s.chars[ci].ascension >= 4 {
            // A4: 20% of her EM to the party but her, for 8 s
            s.st_mut::<State>(ci).a4_em = s.stat_ex(ci, st::EM, true) * 0.20;
            for i in 0..s.chars.len() {
                if i != ci {
                    s.add_mod(i, Mod::owned(A4, ModKind::Stat, Owner::Char(ci as u8), st::EM, true, true, -1), 480);
                }
            }
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 24, false);
            s.queue_particle(4.0, Element::Anemo, s.particle_delay[ci]);
        }
        // C4: every 7 normal or charged attacks, the skill's cooldown
        // goes down 1 to 7 s
        if cbs & CB_C4 != 0 {
            let st = s.st_mut::<State>(ci);
            st.c4_count += 1;
            if st.c4_count >= 7 {
                st.c4_count = 0;
                let cut = 60 * (s.rng.float64() * 6.0 + 1.0) as i32;
                s.reduce_cd(ci, Action::Skill, cut);
            }
        }
    }

    /// A1: her swirl on an enemy gives the characters of the swirled
    /// element 50 EM for 8 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        let (k, ele) = match r {
            Reaction::SwirlPyro => (0, Element::Pyro),
            Reaction::SwirlHydro => (1, Element::Hydro),
            Reaction::SwirlElectro => (2, Element::Electro),
            Reaction::SwirlCryo => (3, Element::Cryo),
            _ => return,
        };
        if ev.on_player || ev.info.actor as usize != ci {
            return;
        }
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        if f < st.a1_icd[k] {
            return;
        }
        st.a1_icd[k] = f + 1;
        for i in 0..s.chars.len() {
            if s.chars[i].element == ele {
                s.add_stat_mod(i, A1, st::EM, 50.0, 480, true);
            }
        }
    }
}

fn c6(s: &mut Sim, ci: usize, e: Element) {
    s.st_mut::<State>(ci).c6 = Some(e);
    for i in 0..s.chars.len() {
        s.add_mod(i, Mod::owned(C6, ModKind::Stat, Owner::Char(ci as u8), e.dmg_stat(), false, true, -1), 600);
    }
}
