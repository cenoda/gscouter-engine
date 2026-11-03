//! Yun Jin, from gcsim's internal/characters/yunjin: normal and charged
//! attacks, the skill (pressed, held to level 1 or 2 with `hold`, or
//! `perfect`), its shield until the hit, the burst's Flying Cloud Flag
//! Formation (flat DMG from her DEF on each character's normal attacks,
//! 30 of them for 12 s), A4 and every constellation. gcsim leaves A1 out
//! and gives her no plunges.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::yunjin as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The burst's stacks left, by character (gcsim's tag on each).
    stacks: [i32; 4],
    /// A4: the party's distinct elements.
    elements: i32,
}

const BURST_BUFF: ModKey = ModKey::new(g::BURST_BUFF_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const SHIELD: ModKey = ModKey::new("yunjin-shield");
const C2: ModKey = ModKey::new(g::C2_KEY);
const C4: ModKey = ModKey::new("yunjin-c4");
const C6: ModKey = ModKey::new(g::C6_KEY);

/// The skill's particles: 2 or 3.
const CB_PARTICLE_2: u16 = 1 << 0;
const CB_PARTICLE_3: u16 = 1 << 1;

// arg: the normal attack's counter times 2 plus its hit
const TASK_NORMAL: u8 = 0;
const TASK_BURST: u8 = 1;

const SKILL_NAMES: [&str; 3] = ["Opening Flourish (Press)", "Opening Flourish (Level 1)", "Opening Flourish (Level 2)"];

impl Character for Char {
    const KEY: &'static str = "yunjin";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].ascension >= 4 {
            let mut seen: Vec<Element> = Vec::new();
            for c in &s.chars {
                if !seen.contains(&c.element) {
                    seen.push(c.element);
                }
            }
            s.st_mut::<State>(ci).elements = seen.len() as i32;
        }
        let o = Owner::Char(ci as u8);
        s.subscribe(EventKind::EnemyHit, o);
        if s.chars[ci].cons >= 4 {
            s.subscribe(EventKind::Reaction, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Skill => p.0.iter().find(|(k, _)| k != "hold" && k != "perfect").map(|(k, _)| format!("{name} param {k}")),
            Action::Attack | Action::Charge | Action::Burst => None,
            _ => Some(name.into()),
        }
    }

    /// gcsim indexes its tables with min(hold, 2): a negative one panics
    /// (unless perfect is 1, which sets the level to 2).
    fn invalid(s: &Sim, _: usize, a: Action, p: &Params) -> Option<String> {
        if a == Action::Skill && p.get("perfect") != Some(1) && p.get("hold").unwrap_or(0) < 0 {
            return Some("yunjin: negative hold".into());
        }
        crate::content::template_invalid(s, a)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let hits = g::ATTACK_HITMARKS[n];
        for (i, &h) in hits.iter().enumerate() {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: (n * 2 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hits[hits.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = s.circle_on_target(0.8);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let mut level = p.get("hold").unwrap_or(0).min(2) as usize;
        let mut anim = level;
        if p.get("perfect") == Some(1) {
            anim = 0;
            level = 2;
        }
        let lvl = s.talent(ci, 1);
        let dur = if level == 2 { 100.0 } else { 50.0 };
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, dur, g::SKILL_DMG[level][lvl]);
        ai.abil = SKILL_NAMES[level];
        ai.use_def = true;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        let (halt, radius) = [(0.06, 4.0), (0.09, 6.0), (0.12, 8.0)][level];
        ai.hitlag_halt = halt * 60.0;
        // level 1: 2 or 3 particles, drawn now
        let cb = match level {
            0 => CB_PARTICLE_2,
            1 => {
                if s.rng.float64() < 0.5 {
                    CB_PARTICLE_2
                } else {
                    CB_PARTICLE_3
                }
            }
            _ => CB_PARTICLE_3,
        };
        ai.pattern = s.circle_on_player(0.0, 0.0, radius);
        let hit = g::SKILL_HITMARKS[anim];
        s.queue_attack(ai, cb, hit, hit);
        // her shield until the hit
        let hp = g::SKILL_SHIELD_PCT[lvl] * s.max_hp(ci) + g::SKILL_SHIELD_FLAT[lvl];
        let f = s.f;
        s.add_shield(Shield { owner: ci as u8, key: SHIELD, typ: ShieldType::YunjinSkill, target: ci as i8, ele: Element::Geo, hp, expiry: f + hit, em_bonus: 0.0 });
        let cd = if s.chars[ci].cons >= 1 { 443 } else { 9 * 60 };
        s.set_cd_with_delay(ci, Action::Skill, cd, g::SKILL_CD_STARTS[anim]);
        let t = &g::SKILL_FRAMES[anim];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // gcsim deletes her own status, and C2's and C6's mods on everyone
        s.delete_mod(ci, BURST_BUFF);
        if s.chars[ci].cons >= 2 {
            for i in 0..s.chars.len() {
                s.delete_mod(i, C2);
            }
        }
        if s.chars[ci].cons >= 6 {
            for i in 0..s.chars.len() {
                s.delete_mod(i, C6);
            }
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST, arg: 0 }, g::BURST_HITMARK);
        s.consume_energy(ci, 4);
        s.set_cd(ci, Action::Burst, 15 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_NORMAL => {
                let (n, i) = (arg as usize / 2, arg as usize % 2);
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
                ai.def_halt = g::ATTACK_DEF_HALT[n][i];
                let (off, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
                ai.pattern = if n == 3 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.fan_on_player(0.0, off, hb[0], g::ATTACK_FAN_ANGLES[n]) };
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_BURST => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Geo, 50.0, g::BURST_DMG[lvl]);
                ai.abil = "Cliffbreaker's Banner";
                ai.blunt = true;
                ai.poise = 200.0;
                ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
                s.queue_attack(ai, 0, 0, 0);
                for i in 0..s.chars.len() {
                    s.st_mut::<State>(ci).stacks[i] = 30;
                    s.add_status(i, BURST_BUFF, 720, true);
                }
                let o = Owner::Char(ci as u8);
                if s.chars[ci].cons >= 2 {
                    for i in 0..s.chars.len() {
                        s.add_mod(i, Mod::owned(C2, ModKind::Attack, o, st::NONE, false, true, -1), 12 * 60);
                    }
                }
                if s.chars[ci].cons >= 6 {
                    for i in 0..s.chars.len() {
                        s.add_mod(i, Mod::owned(C6, ModKind::Stat, o, st::ATKSPD, false, true, -1), 12 * 60);
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & (CB_PARTICLE_2 | CB_PARTICLE_3) != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, true);
            let count = if cbs & CB_PARTICLE_3 != 0 { 3.0 } else { 2.0 };
            s.queue_particle(count, Element::Geo, s.particle_delay[ci]);
        }
    }

    /// C2: normal attacks deal 15% more.
    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C2 && ev.info.tag == AttackTag::Normal {
            ev.stats[st::DMGP] += 0.15;
        }
    }

    /// C6: 12% ATK SPD while the character has stacks left.
    fn mod_amount(s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C6 && s.st::<State>(ci).stacks[on] != 0 {
            out[st::ATKSPD] += 0.12;
            return true;
        }
        false
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // the burst: a normal attack by a character with stacks adds
            // her current DEF times the multiplier (and A4), using a stack
            Event::EnemyHit(ev) => {
                if ev.info.tag != AttackTag::Normal {
                    return;
                }
                let a = ev.info.actor as usize;
                if s.st::<State>(ci).stacks[a] == 0 || !s.mod_active(a, BURST_BUFF) {
                    return;
                }
                let amt = total_def(&s.stats(ci)) * (g::BURST_BUFF[s.talent(ci, 2)] + a4(s, ci));
                s.hit_flat += amt;
                s.st_mut::<State>(ci).stacks[a] -= 1;
            }
            // C4: her crystallize raises her DEF by 20% for 12 s
            Event::Reaction(Reaction::CrystallizeElectro | Reaction::CrystallizeHydro | Reaction::CrystallizePyro | Reaction::CrystallizeCryo | Reaction::LunarCrystallize, ev)
                if ev.on_enemy() && ev.info.actor as usize == ci =>
            {
                s.add_stat_mod(ci, C4, st::DEFP, 0.2, 12 * 60, true);
            }
            _ => {}
        }
    }
}

/// A4: 2.5% per element in the party, 11.5% with four.
fn a4(s: &Sim, ci: usize) -> f64 {
    if s.chars[ci].ascension < 4 {
        return 0.0;
    }
    match s.st::<State>(ci).elements {
        4 => 0.115,
        n => 0.025 * n as f64,
    }
}

/// gcsim's Tags["yunjin-q"] on the character in slot `on`: the burst's
/// stacks it has left (0 without Yun Jin).
pub fn burst_tag(s: &Sim, on: usize) -> i64 {
    match s.chars.iter().position(|c| c.kind == CharKind::Yunjin) {
        Some(y) => s.st::<State>(y).stacks[on] as i64,
        None => 0,
    }
}
