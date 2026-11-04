//! Nilou (gcsim's internal/characters/nilou).
//!
//! Her skill starts Pirouette (10 s): her next three normal attacks or skill
//! presses are its dance steps, Sword Dance or Whirling Steps. The third
//! step ends Pirouette and grants Lunar Prayer (normal attacks stay Sword
//! Dance) or the Tranquility Aura (a hydro application every 0.5 s), and
//! A1's Golden Chalice's Bounty in a team of hydro and dendro only: blooms
//! of characters under it make Bountiful Cores (`Sim::make_bountiful`), and
//! a dendro hit on the player gives the party 100 EM.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::nilou as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// gcsim's dancestep tag: the next step of Pirouette (0 to 2).
    step: i32,
    aura_src: i32,
    a4_src: i32,
    a4_bonus: f64,
    /// A1's hooks are subscribed (gcsim subscribes them on every A1 under
    /// the same keys, which replaces them in place).
    a1_subscribed: bool,
    only_bloom_team: bool,
}

const A1: ModKey = ModKey::new(g::A1_STATUS);
const A1_EM: ModKey = ModKey::new("nilou-a1-em");
const A4: ModKey = ModKey::new(g::A4_MOD);
const PIROUETTE: ModKey = ModKey::new(g::PIROUETTE_STATUS);
const LUNAR_PRAYER: ModKey = ModKey::new(g::LUNAR_PRAYER_STATUS);
const TRANQUILITY_AURA: ModKey = ModKey::new(g::TRANQUILITY_AURA_STATUS);
const LINGERING_AEON: ModKey = ModKey::new(g::LINGERING_AEON_STATUS);
const INITIAL_PARTICLE_ICD: ModKey = ModKey::new(g::INITIAL_PARTICLE_ICD_KEY);
const PIROUETTE_PARTICLE_ICD: ModKey = ModKey::new(g::PIROUETTE_PARTICLE_ICD_KEY);
const C1: ModKey = ModKey::new("nilou-c1");
const C2_HYDRO: ModKey = ModKey::new("nilou-c2-hydro");
const C2_DENDRO: ModKey = ModKey::new("nilou-c2-dendro");
const C4: ModKey = ModKey::new("nilou-c4");
const C6_CR: ModKey = ModKey::new("nilou-c6-cr");
const C6_CD: ModKey = ModKey::new("nilou-c6-cd");

// attack callbacks
const CB_INITIAL_PARTICLE: u16 = 1 << 0;
const CB_PIROUETTE_PARTICLE: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;
const CB_LINGERING_AEON: u16 = 1 << 3;

// character tasks
/// The third step's end of Pirouette (arg: 1 dance, 2 steps).
const TASK_PIROUETTE_END: u8 = 0;
/// A Tranquility Aura tick (arg: auraSrc).
const TASK_AURA: u8 = 1;
/// A4's bonus is read from her max HP (arg: a4Src).
const TASK_A4: u8 = 2;
/// Lingering Aeon goes off (enemy queue).
const TASK_AEON: u8 = 3;

const DANCE: i32 = g::NILOU_SKILL_TYPE_DANCE;
const STEPS: i32 = g::NILOU_SKILL_TYPE_STEPS;

const NORMAL_NAMES: [&str; 3] = ["Normal 0", "Normal 1", "Normal 2"];
const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];
const SWORD_DANCE_NAMES: [&str; 3] = ["Sword Dance 0", "Sword Dance 1", g::SKILL_ILLUSION_ABIL];
const WHIRLING_NAMES: [&str; 3] = ["Whirling Steps 0", "Whirling Steps 1", "Water Wheel"];

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn char_task(s: &mut Sim, ci: usize, id: u8, arg: i32, delay: i32) {
    s.queue_char_task(ci, Task::Char { c: ci as u8, id, arg }, delay);
}

impl Character for Char {
    const KEY: &'static str = "nilou";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { step: 0, aura_src: -1, a4_src: -1, a4_bonus: 0.0, a1_subscribed: false, only_bloom_team: false }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // onExitField
        s.subscribe(EventKind::Swap, o);
        let cons = s.chars[ci].cons;
        // C1: Luminous Illusion DMG +65%
        if cons >= 1 {
            s.chars[ci].mods.push(Mod::owned(C1, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        // C2: resistance shred from the A1 party's hydro and bloom damage
        if cons >= 2 {
            s.subscribe(EventKind::EnemyDamage, o);
        }
        // C6: CR and CD from max HP
        if cons >= 6 {
            s.chars[ci].mods.push(Mod::owned(C6_CR, ModKind::Stat, o, st::CR, true, false, -1));
            s.chars[ci].mods.push(Mod::owned(C6_CD, ModKind::Stat, o, st::CD, true, false, -1));
        }
        let (mut dendro, mut hydro) = (0, 0);
        for c in &s.chars {
            match c.element {
                Element::Dendro => dendro += 1,
                Element::Hydro => hydro += 1,
                _ => {}
            }
        }
        let n = s.chars.len();
        st_mut(s, ci).only_bloom_team = dendro > 0 && dendro + hydro == n;
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let allowed: &[&str] = match a {
            Action::Attack => &["travel"],
            Action::Charge | Action::Skill | Action::Burst => &[],
            _ => return Some(name.into()),
        };
        p.0.iter().find(|(k, _)| !allowed.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
    }

    /// The skill is always ready during Pirouette.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && s.mod_active(ci, PIROUETTE)).then_some(None)
    }

    /// AnimationStartDelay: Xingqiu's wave (2).
    fn start_delay(_: &Sim, _: usize, key: usize) -> Option<i32> {
        (key == 2).then_some(11)
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.mod_active(ci, PIROUETTE) {
            return pirouette(s, ci, p, DANCE);
        }
        if s.mod_active(ci, LUNAR_PRAYER) {
            return sword_dance(s, ci, p);
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::AUTO[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = g::ATTACK_DEF_HALT[n];
        let hb = g::ATTACK_HITBOXES[n];
        ai.pattern = if n == 0 {
            s.box_on_player(0.0, g::ATTACK_OFFSETS[n], hb[0], hb[1])
        } else {
            s.circle_on_player(0.0, g::ATTACK_OFFSETS[n], hb[0])
        };
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
            ai.pattern = s.circle_on_player(0.0, 0.5, 2.3);
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        let last = g::CHARGE_HITMARKS[g::CHARGE_HITMARKS.len() - 1];
        ActInfo::new(Frames::Table(t), t[0], last, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.mod_active(ci, PIROUETTE) {
            return pirouette(s, ci, p, STEPS);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
        ai.abil = "Dance of Haftkarsvar";
        ai.flat_dmg = g::SKILL[s.talent(ci, 1)] * s.max_hp(ci);
        ai.pattern = s.circle_on_player(0.0, 0.0, 2.5);
        s.queue_attack(ai, CB_INITIAL_PARTICLE, g::SKILL_HITMARK, g::SKILL_HITMARK);
        st_mut(s, ci).step = 0;
        s.add_status(ci, PIROUETTE, 10 * 60, true);
        s.set_cd(ci, Action::Skill, 18 * 60);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
        ai.abil = "Dance of Abzendegi: Distant Dreams, Listening Spring";
        ai.flat_dmg = s.max_hp(ci) * g::BURST[s.talent(ci, 2)];
        ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
        s.queue_attack(ai, CB_LINGERING_AEON, g::BURST_HITMARK, g::BURST_HITMARK);
        s.consume_energy(ci, 4);
        s.set_cd(ci, Action::Burst, 18 * 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_PIROUETTE_END => {
                a1(s, ci);
                s.delete_mod(ci, PIROUETTE);
                if arg == DANCE {
                    s.add_status(ci, LUNAR_PRAYER, 8 * 60, true);
                } else {
                    let mut dur = 12 * 60;
                    if s.chars[ci].cons >= 1 {
                        dur += 6 * 60;
                    }
                    s.add_status(ci, TRANQUILITY_AURA, dur, true);
                    let f = s.f;
                    st_mut(s, ci).aura_src = f;
                    aura_tick(s, ci, f);
                }
            }
            TASK_AURA => aura_tick(s, ci, arg),
            TASK_A4 => {
                if st(s, ci).a4_src != arg || !s.mod_active(ci, A4) {
                    return;
                }
                let bonus = ((s.max_hp(ci) - 30000.0) * 0.001 * 0.09).clamp(0.0, 4.0);
                st_mut(s, ci).a4_bonus = bonus;
                char_task(s, ci, TASK_A4, arg, 30);
            }
            TASK_AEON => {
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
                ai.abil = "Lingering Aeon";
                ai.flat_dmg = s.max_hp(ci) * g::BURST_AEON[s.talent(ci, 2)];
                ai.pattern = Pattern::Single;
                s.queue_attack(ai, 0, 0, 0);
            }
            _ => {}
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_INITIAL_PARTICLE != 0 && !s.mod_active(ci, INITIAL_PARTICLE_ICD) {
            s.add_status(ci, INITIAL_PARTICLE_ICD, 6, true);
            let count = if s.rng.float64() < 0.5 { 2.0 } else { 1.0 };
            s.queue_particle(count, Element::Hydro, s.particle_delay[ci]);
        }
        if cbs & CB_C4 != 0 {
            s.add_energy(ci, 15.0);
            s.add_mod(ci, Mod::owned(C4, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 8 * 60);
        }
        if cbs & CB_PIROUETTE_PARTICLE != 0 && !s.mod_active(ci, PIROUETTE_PARTICLE_ICD) {
            s.add_status(ci, PIROUETTE_PARTICLE_ICD, 6, true);
            s.queue_particle(1.0, Element::Hydro, s.particle_delay[ci]);
        }
        if cbs & CB_LINGERING_AEON != 0 {
            s.enemy_add_status(LINGERING_AEON, g::BURST_AEON_HITMARK, false);
            s.queue_enemy_task(Task::Char { c: ci as u8, id: TASK_AEON, arg: 0 }, g::BURST_AEON_HITMARK);
        }
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C1 && ev.info.abil == g::SKILL_ILLUSION_ABIL {
            ev.stats[st::DMGP] += 0.65;
        } else if m.key == C4 && ev.info.tag == AttackTag::ElementalBurst {
            ev.stats[st::DMGP] += 0.5;
        }
    }

    /// A4: Bountiful Core damage.
    fn react_bonus(s: &Sim, ci: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if m.key != A4 || info.tag != AttackTag::Bloom || info.icd_tag != icd::TAG_BOUNTIFUL_CORE_DAMAGE {
            return 0.0;
        }
        st(s, ci).a4_bonus
    }

    /// C6: CR +0.6% and CD +1.2% per 1,000 max HP, up to 30% and 60%.
    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C6_CR {
            out[st::CR] += (s.max_hp(ci) * 0.001 * 0.006).min(0.3);
            return true;
        }
        if m.key == C6_CD {
            out[st::CD] += (s.max_hp(ci) * 0.001 * 0.012).min(0.6);
            return true;
        }
        false
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { .. } => {
                s.delete_mod(ci, PIROUETTE);
                s.delete_mod(ci, LUNAR_PRAYER);
                st_mut(s, ci).step = 0;
            }
            Event::DendroCore { key } => {
                let Some(actor) = s.gadgets.iter().find(|g| g.key == key && g.owner == CORE).map(|g| g.actor as usize) else { return };
                if s.mod_active(actor, A1) {
                    s.make_bountiful(key);
                }
            }
            Event::PlayerHit { on, ele, .. } => {
                if !s.mod_active(on, A1) || ele != Element::Dendro {
                    return;
                }
                for i in 0..s.chars.len() {
                    s.add_stat_mod(i, A1_EM, st::EM, 100.0, 10 * 60, true);
                }
            }
            Event::EnemyDamage(ev) => {
                if ev.damage == 0.0 || !s.mod_active(ev.info.actor as usize, A1) {
                    return;
                }
                if ev.info.element == Element::Hydro {
                    s.enemy_add_resist(C2_HYDRO, Element::Hydro, -0.35, 10 * 60);
                } else if matches!(ev.info.tag, AttackTag::Bloom | AttackTag::DirectLunarBloom) {
                    s.enemy_add_resist(C2_DENDRO, Element::Dendro, -0.35, 10 * 60);
                }
            }
            _ => {}
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match (fields[0].as_str(), fields.get(1).map(String::as_str)) {
            ("tags", Some(g::SKILL_STEP)) => Some(Ok(Num::Int(st(s, ci).step as i64))),
            _ => None,
        }
    }
}

/// gcsim's Pirouette: a dance step, and after the third its end, `delay`
/// later.
fn pirouette(s: &mut Sim, ci: usize, p: &Params, kind: i32) -> ActInfo {
    let (info, delay) = if kind == DANCE { (sword_dance(s, ci, p), g::DELAY_DANCE) } else { (whirling_steps(s, ci), g::DELAY_STEPS) };
    if st(s, ci).step == 0 {
        char_task(s, ci, TASK_PIROUETTE_END, kind, delay);
    }
    info
}

/// The callbacks of a dance step: C4's on the third during Pirouette, and
/// the Pirouette particle.
fn step_callbacks(s: &Sim, ci: usize) -> u16 {
    let pirouette = s.mod_active(ci, PIROUETTE);
    let mut cbs = 0;
    if s.chars[ci].cons >= 4 && st(s, ci).step == 2 && pirouette {
        cbs |= CB_C4;
    }
    if pirouette {
        cbs |= CB_PIROUETTE_PARTICLE;
    }
    cbs
}

fn advance_step(s: &mut Sim, ci: usize) {
    let st = st_mut(s, ci);
    st.step = (st.step + 1) % 3;
}

fn sword_dance(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
    let n = st(s, ci).step as usize;
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
    ai.abil = SWORD_DANCE_NAMES[n];
    ai.flat_dmg = g::SWORD_DANCE[n][s.talent(ci, 1)] * s.max_hp(ci);
    let hb = g::SWORD_DANCE_HITBOXES[n];
    let mut travel = 0;
    ai.pattern = match n {
        0 => s.box_on_player(0.0, g::SWORD_DANCE_OFFSETS[n], hb[0], hb[1]),
        1 => s.circle_on_player(0.0, g::SWORD_DANCE_OFFSETS[n], hb[0]),
        _ => {
            travel = p.get("travel").map_or(0, |t| t as i32);
            s.circle_on_target(hb[0])
        }
    };
    let h = g::SWORD_DANCE_HIT_MARKS[n] + travel;
    let cbs = step_callbacks(s, ci);
    s.queue_attack(ai, cbs, h, h);
    advance_step(s, ci);
    let atkspd = s.stat(ci, st::ATKSPD);
    let table = &g::SWORD_DANCE_FRAMES[n];
    ActInfo::new(Frames::Attack { table, atkspd }, table[0], table[Action::Jump as usize], AnimState::Normal)
}

fn whirling_steps(s: &mut Sim, ci: usize) -> ActInfo {
    let n = st(s, ci).step as usize;
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
    ai.abil = WHIRLING_NAMES[n];
    ai.flat_dmg = g::WHIRLING_STEPS[n][s.talent(ci, 1)] * s.max_hp(ci);
    let (o, hb) = (g::WHIRLING_STEPS_OFFSETS[n], g::WHIRLING_STEPS_HITBOXES[n]);
    ai.pattern = if n == 2 { s.box_on_player(o[0], o[1], hb[0], hb[1]) } else { s.circle_on_player(o[0], o[1], hb[0]) };
    let h = g::WHIRLING_STEPS_HIT_MARKS[n];
    let cbs = step_callbacks(s, ci);
    s.queue_attack(ai, cbs, h, h);
    advance_step(s, ci);
    let t = &g::WHIRLING_STEPS_FRAMES[n];
    let cqa = if n == 1 { t[Action::Skill as usize] } else { t[Action::Dash as usize] };
    ActInfo::new(Frames::Table(t), t[0], cqa, AnimState::Skill)
}

/// gcsim's TranquilityAura: a hydro application without damage on a circle
/// of radius 2.5 on the player, every 0.5 s while the aura lasts.
fn aura_tick(s: &mut Sim, ci: usize, src: i32) {
    if st(s, ci).aura_src != src || !s.mod_active(ci, TRANQUILITY_AURA) {
        return;
    }
    let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NILOU_TRANQUILITY_AURA, icd::GROUP_NILOU, Element::Hydro, 25.0, 0.0);
    ai.abil = "Tranquility Aura";
    ai.pattern = s.circle_on_player(0.0, 0.0, 2.5);
    s.queue_attack(ai, 0, -1, 1);
    char_task(s, ci, TASK_AURA, src, 30);
}

/// A1: Golden Chalice's Bounty for the party (30 s), then A4.
fn a1(s: &mut Sim, ci: usize) {
    if s.chars[ci].ascension < 1 || !st(s, ci).only_bloom_team {
        return;
    }
    for i in 0..s.chars.len() {
        s.add_status(i, A1, 30 * 60, true);
    }
    a4(s, ci);
    if !st(s, ci).a1_subscribed {
        st_mut(s, ci).a1_subscribed = true;
        let o = Owner::Char(ci as u8);
        s.subscribe(EventKind::DendroCore, o);
        s.subscribe(EventKind::PlayerHit, o);
    }
}

/// A4: Bountiful Core DMG +9% per 1,000 max HP above 30,000, up to 400%,
/// read every 0.5 s.
fn a4(s: &mut Sim, ci: usize) {
    if s.chars[ci].ascension < 4 {
        return;
    }
    let o = Owner::Char(ci as u8);
    for i in 0..s.chars.len() {
        s.add_mod(i, Mod::owned(A4, ModKind::ReactBonus, o, st::NONE, false, true, -1), 30 * 60);
    }
    let f = s.f;
    st_mut(s, ci).a4_src = f;
    char_task(s, ci, TASK_A4, f, 30);
}
