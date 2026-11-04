//! Kaveh, from gcsim's internal/characters/kaveh: normal attacks (wider
//! during the burst), the skill and the burst (both rupture the dendro
//! cores they reach), the burst's dendro infusion of normal, charged and
//! plunging attacks and its bloom DMG bonus for the party (all ended by any
//! swap), A1's heal when a core's bloom, hyperbloom or burgeon hits him,
//! A4's EM stacks, the plunges from Xianyun's airborne state, and C1 to C6.
//! gcsim gives him no charged attack; it stays an error.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::kaveh as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    a4_stacks: i32,
    /// The areas whose cores the skill, the burst and C6 rupture, taken
    /// when gcsim builds them.
    skill_area: Pattern,
    burst_area: Pattern,
    c6_area: Pattern,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const BURST_DMG_BONUS: ModKey = ModKey::new(g::BURST_DMG_BONUS_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1_ICD: ModKey = ModKey::new(g::A1_ICD_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const C1: ModKey = ModKey::new("kaveh-c1");
const C2: ModKey = ModKey::new(g::C2_KEY);
const C4: ModKey = ModKey::new("kaveh-c4");
const C6_ICD: ModKey = ModKey::new(g::C6_ICD_KEY);

const CB_PARTICLE: u16 = 1 << 0;

const TASK_SKILL_RUPTURE: u8 = 0;
const TASK_C1: u8 = 1;
const TASK_BURST: u8 = 2;
const TASK_C6_RUPTURE: u8 = 3;

const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

impl Character for Char {
    const KEY: &'static str = "kaveh";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { a4_stacks: 0, skill_area: Pattern::Single, burst_area: Pattern::Single, c6_area: Pattern::Single }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let c = &s.chars[ci];
        let (cons, asc) = (c.cons, c.ascension);
        if cons >= 4 {
            s.chars[ci].mods.push(Mod::owned(C4, ModKind::ReactBonus, o, st::NONE, false, false, -1));
        }
        if cons >= 6 || asc >= 4 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        if asc >= 1 {
            s.subscribe(EventKind::PlayerHit, o);
        }
        s.subscribe(EventKind::Swap, o);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    /// The burst's dendro, which nothing overrides.
    fn snapshot(s: &Sim, ci: usize, info: &mut AttackInfo) {
        if s.mod_active(ci, BURST) && matches!(info.tag, AttackTag::Normal | AttackTag::Plunge | AttackTag::Extra) {
            info.element = Element::Dendro;
        }
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
        let idx = if s.mod_active(ci, BURST) { 1 } else { 0 };
        ai.pattern = s.fan_on_player(0.0, g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[idx][n], g::ATTACK_FAN_ANGLES[n]);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::SKILL[lvl]);
        ai.abil = "Artistic Ingenuity";
        let area = s.circle_on_player(0.0, 0.0, 5.0);
        ai.pattern = area;
        s.queue_attack(ai, CB_PARTICLE, 0, g::SKILL_HITMARK);
        st_mut(s, ci).skill_area = area;
        s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_RUPTURE, arg: 0 }, g::SKILL_HITMARK + 3);
        if s.chars[ci].cons >= 1 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_C1, arg: 0 }, 33);
        }
        s.set_cd_with_delay(ci, Action::Skill, 360, 33);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        st_mut(s, ci).a4_stacks = 0;
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 50.0, g::BURST[lvl]);
        ai.abil = "Painted Dome";
        let area = s.circle_on_player(0.0, 0.0, 5.0);
        ai.pattern = area;
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        st_mut(s, ci).burst_area = area;
        s.set_cd(ci, Action::Burst, 1200);
        s.consume_energy(ci, 3);
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST, arg: 0 }, g::BURST_HITMARK);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_SKILL_RUPTURE => {
                let area = st(s, ci).skill_area;
                s.rupture_cores(area, ci, "Artistic Ingenuity");
            }
            // C1: healing bonus for 3 s
            TASK_C1 => s.add_stat_mod(ci, C1, st::HEAL, 0.25, 180, true),
            TASK_BURST => {
                let area = st(s, ci).burst_area;
                s.rupture_cores(area, ci, "Painted Dome");
                s.add_status(ci, BURST, g::BURST_DURATION, true);
                // A4's EM, whatever his ascension (its stacks need A4)
                s.add_mod(ci, Mod::owned(A4, ModKind::Stat, Owner::Char(ci as u8), st::EM, false, true, -1), g::BURST_DURATION);
                if s.chars[ci].cons >= 2 {
                    s.add_stat_mod(ci, C2, st::ATKSPD, 0.15, g::BURST_DURATION, true);
                }
                for i in 0..s.chars.len() {
                    s.add_mod(i, Mod::owned(BURST_DMG_BONUS, ModKind::ReactBonus, Owner::Char(ci as u8), st::NONE, false, true, -1), g::BURST_DURATION);
                }
            }
            TASK_C6_RUPTURE => {
                let area = st(s, ci).c6_area;
                s.rupture_cores(area, ci, "Pairidaeza's Dreams (C6)");
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && ev.on_enemy() && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 12, false);
            s.queue_particle(2.0, Element::Dendro, s.particle_delay[ci]);
        }
    }

    /// The burst's bloom DMG bonus on every character, and C4's on him.
    fn react_bonus(s: &Sim, ci: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if info.tag != AttackTag::Bloom {
            return 0.0;
        }
        if m.key == BURST_DMG_BONUS {
            g::BURST_DMG_BONUS[s.talent(ci, 2)]
        } else if m.key == C4 {
            0.6
        } else {
            0.0
        }
    }

    /// A4: EM +25 per stack.
    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == A4 {
            out[st::EM] += (25 * st(s, ci).a4_stacks) as f64;
            return true;
        }
        false
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyHit(ev) => {
                if ev.info.actor as usize != ci || !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
                    return;
                }
                // C6 (subscribed before A4 in gcsim's Init)
                if s.chars[ci].cons >= 6 && s.mod_active(ci, BURST) && !s.mod_active(ci, C6_ICD) {
                    s.add_status(ci, C6_ICD, 180, false);
                    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, 0.618);
                    ai.abil = "Pairidaeza's Dreams (C6)";
                    ai.hitlag_factor = 0.01;
                    ai.hitlag_halt = 0.09 * 60.0;
                    let area = s.circle_on_target(4.0);
                    ai.pattern = area;
                    s.queue_attack(ai, 0, 0, 18);
                    st_mut(s, ci).c6_area = area;
                    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C6_RUPTURE, arg: 0 }, 18);
                }
                // A4
                if s.chars[ci].ascension >= 4 && st(s, ci).a4_stacks < 4 && s.mod_active(ci, BURST) && !s.mod_active(ci, A4_ICD) {
                    s.add_status(ci, A4_ICD, 6, true);
                    st_mut(s, ci).a4_stacks += 1;
                }
            }
            // A1: hit by a core's reaction
            Event::PlayerHit { on, tag, .. } => {
                if on != ci || !matches!(tag, AttackTag::Bloom | AttackTag::Hyperbloom | AttackTag::Burgeon) || s.mod_active(ci, A1_ICD) {
                    return;
                }
                s.add_status(ci, A1_ICD, 30, true);
                let src = 3.0 * s.stat(ci, st::EM);
                let bonus = s.stat(ci, st::HEAL);
                s.heal(ci, ci, src, bonus);
            }
            // any swap ends the burst
            Event::Swap { .. } => {
                s.delete_mod(ci, BURST);
                s.delete_mod(ci, A4);
                s.delete_mod(ci, C2);
                for i in 0..s.chars.len() {
                    s.delete_mod(i, BURST_DMG_BONUS);
                }
            }
            _ => {}
        }
    }
}
