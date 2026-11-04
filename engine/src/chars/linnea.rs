//! Linnea, from gcsim's internal/characters/linnea: normal and aimed
//! shots, the skill (Lumi in Super Power, its recasts, the third of them
//! Million Ton Crush and Standard Power), Lumi's hammers (Heavy Overdrive
//! Hammer among them while Moondrifts are out), the burst's heals, A1's
//! Geo RES shred around Lumi, A4's EM from her DEF, her Moonsign bonus to
//! Lunar-Crystallize DMG, and C1 to C6. gcsim gives her no plunges; they
//! stay errors.
//!
//! Lumi stands on the enemy (gcsim's LumiPos is the primary target). The
//! burst's heal adds burstInitialFlat to burstInitialDef times her DEF, as
//! gcsim's tables have them.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::linnea as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    recast_count: i32,
    recast_src: i32,
    skill_src: i32,
    skill_hit: usize,
    c1_stacks: i32,
}

const RECAST: ModKey = ModKey::new(g::SKILL_RECAST_KEY);
const SUPER: ModKey = ModKey::new(g::SKILL_SUPER_POWER);
const STANDARD: ModKey = ModKey::new(g::SKILL_STANDARD_POWER);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const C1: ModKey = ModKey::new(g::C1_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C4: ModKey = ModKey::new(g::C4_KEY);
const C4_SELF: ModKey = ModKey::new(g::C4_KEY_SELF);

const CB_PARTICLE: u16 = 1 << 0;

/// The recast window's end (its arg the recast's source frame; 1 in arg's
/// top bit for the first press, which skips a pound).
const TASK_SUMMON: u8 = 0;
const TASK_LUMI: u8 = 1;
const TASK_POUND_2: u8 = 2;
const TASK_MILLION: u8 = 3;
const TASK_A1: u8 = 4;
const TASK_HEAL_INITIAL: u8 = 5;
const TASK_HEAL_TICK: u8 = 6;

const FIRST_PRESS: i32 = 1 << 30;

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn moonsign_level(s: &Sim) -> i32 {
    s.chars.iter().map(|c| c.moonsign).sum()
}

fn total_def_of(s: &Sim, ci: usize, non_extra: bool) -> f64 {
    total_def(&s.select_stats(ci, [st::BASEDEF, st::DEFP, st::DEF], non_extra))
}

impl Character for Char {
    const KEY: &'static str = "linnea";
    const MOONSIGN: i32 = 1;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { recast_count: 0, recast_src: -1, skill_src: -1, skill_hit: 0, c1_stacks: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        // A4: EM from her DEF, for the other Moonsign characters on the
        // field and for her
        if asc >= 4 {
            for i in 0..s.chars.len() {
                if s.chars[i].moonsign != 0 && i != ci {
                    s.chars[i].mods.push(Mod::owned(A4, ModKind::Stat, o, st::EM, false, true, -1));
                }
            }
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Stat, o, st::EM, false, true, -1));
        }
        // her Moonsign: Lunar-Crystallize, and its DMG from her DEF
        s.enable_lunar_crystallize();
        s.subscribe(EventKind::EnemyHit, o);
        s.subscribe(EventKind::SpecialReactionAttack, o);
        if cons >= 1 || cons >= 2 || cons >= 4 {
            s.subscribe(EventKind::MoondriftHarmony, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let allowed: &[&str] = match a {
            Action::Attack => &["travel"],
            Action::Aim => &["hold", "travel", "weakspot"],
            // gcsim validates recast and never reads it
            Action::Skill => &["recast"],
            Action::Burst => &[],
            _ => return Some(name.into()),
        };
        p.0.iter().find(|(k, _)| !allowed.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
    }

    fn invalid(s: &Sim, _: usize, a: Action, p: &Params) -> Option<String> {
        if a == Action::Aim {
            let hold = p.get("hold").unwrap_or(1);
            if !(0..=1).contains(&hold) {
                return Some(format!("invalid hold param supplied, got {hold}"));
            }
        }
        crate::content::template_invalid(s, a)
    }

    /// The recast is ready while its window lasts.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && s.mod_active(ci, RECAST)).then_some(None)
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").unwrap_or(10) as i32;
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").unwrap_or(10) as i32;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::FULLAIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        ai.deployable = true;
        // its hitlag applies on weak spot hits only (HitlagOnHeadshotOnly)
        if p.get("weakspot") == Some(1) {
            ai.weak_point = true;
            ai.hitlag_halt = 0.12 * 60.0;
        }
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let h = g::AIMED_HITMARKS[hold];
        s.queue_attack(ai, 0, h, h + travel);
        let t = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Aim)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, RECAST) {
            return skill_recast(s, ci);
        }
        st_mut(s, ci).recast_count = 0;
        s.delete_mod(ci, STANDARD);
        s.delete_mod(ci, SUPER);
        s.add_status(ci, RECAST, g::SKILL_RECAST_DUR as i32, true);
        let src = s.f;
        st_mut(s, ci).recast_src = src;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SUMMON, arg: src | FIRST_PRESS }, g::SKILL_RECAST_DUR as i32);
        c1_on_skill(s, ci);
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD, g::SKILL_CD_START);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let src = s.f;
        st_mut(s, ci).skill_src = src;
        if s.mod_active(ci, STANDARD) {
            s.add_status(ci, STANDARD, g::SKILL_DUR, false);
        } else if s.mod_active(ci, SUPER) {
            s.add_status(ci, SUPER, g::SKILL_DUR, false);
        } else {
            s.add_status(ci, SUPER, g::SKILL_DUR, false);
            a1_on_lumi(s, ci, src);
            // the first pound is skipped right after summoning
            advance_skill_index(s, ci);
            s.add_task(Task::Char { c: ci as u8, id: TASK_LUMI, arg: src }, g::FIRST_SKILL_HIT_DELAY);
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_HEAL_INITIAL, arg: 0 }, g::INITIAL_HEAL);
        for i in 0..5 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_HEAL_TICK, arg: 0 }, g::INITIAL_HEAL + i * 60 + 60);
        }
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, g::ENERGY_DRAIN_DELAY);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Skill as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            // the recast window ends: Lumi in Super Power
            TASK_SUMMON => {
                let src = arg & !FIRST_PRESS;
                if st(s, ci).recast_src != src {
                    return;
                }
                if !s.mod_active(ci, SUPER) && s.moondrift_count() > 0 {
                    st_mut(s, ci).skill_hit = 1;
                }
                s.add_status(ci, SUPER, g::SKILL_DUR, false);
                st_mut(s, ci).skill_src = src;
                a1_on_lumi(s, ci, src);
                if arg & FIRST_PRESS != 0 {
                    // the first pound is skipped right after summoning
                    advance_skill_index(s, ci);
                }
                s.add_task(Task::Char { c: ci as u8, id: TASK_LUMI, arg: src }, g::SKILL_SUPER_INTERVAL[1]);
            }
            TASK_LUMI => lumi_attack(s, ci, arg),
            TASK_POUND_2 => {
                if st(s, ci).skill_src != arg {
                    return;
                }
                let ai = pound_info(s, ci, "Lumi Pound-Pound Pummeler 2");
                s.queue_attack(ai, CB_PARTICLE, 0, 0);
            }
            TASK_MILLION => million_ton(s, ci),
            TASK_A1 => a1_tick(s, ci, arg),
            TASK_HEAL_INITIAL | TASK_HEAL_TICK => {
                let lvl = s.talent(ci, 2);
                let (flat, def) = if id == TASK_HEAL_INITIAL { (g::BURST_INITIAL_FLAT[lvl], g::BURST_INITIAL_DEF[lvl]) } else { (g::BURST_TICK_FLAT[lvl], g::BURST_TICK_DEF[lvl]) };
                let src = flat + def * total_def_of(s, ci, false);
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, None, false, src, bonus);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && ev.on_enemy() && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, g::PARTICLE_ICD, true);
            s.queue_particle(g::PARTICLE_COUNT as f64, Element::Geo, s.particle_delay[ci]);
        }
    }

    /// A4's EM (5% of her DEF from artifacts and weapon aside... gcsim's
    /// TotalDef(true)) on the active Moonsign character or on her; C4's
    /// DEF on the active character and on her.
    fn mod_amount(s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool {
        let active = s.player.active as usize;
        if m.key == A4 {
            if on != ci {
                if active != on {
                    return false;
                }
            } else if s.chars[active].moonsign > 0 && active != ci {
                return false;
            }
            out[st::EM] += total_def_of(s, ci, true) * 0.05;
            return true;
        }
        if m.key == C4 {
            if active != on {
                return false;
            }
            out[st::DEFP] += 0.25;
            return true;
        }
        if m.key == C4_SELF {
            out[st::DEFP] += 0.25;
            return true;
        }
        false
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // Moonsign: direct Lunar-Crystallize DMG from her DEF; C1's
            // Field Catalog; C6's elevation (gcsim's OnApplyAttack, the same
            // for the one enemy)
            Event::EnemyHit(ev) => {
                if ev.info.tag != AttackTag::DirectLunarCrystallize {
                    return;
                }
                s.hit_base_bonus += lcr_bonus(s, ci);
                if s.chars[ci].cons >= 1 {
                    let flat = c1_flat(s, ci, ev.info.actor as usize == ci && ev.info.abil == g::SKILL_MILLION_ABIL);
                    s.hit_flat += flat;
                }
                if s.chars[ci].cons >= 6 && moonsign_level(s) >= 2 {
                    s.hit_elevation += 250;
                }
            }
            Event::SpecialReactionAttack(ev) => {
                if ev.info.tag != AttackTag::ReactionLunarCrystallize {
                    return;
                }
                s.special_base_bonus += lcr_bonus(s, ci);
                if s.chars[ci].cons >= 1 {
                    let flat = c1_flat(s, ci, false);
                    s.special_atk.info.flat_dmg += flat;
                }
                if s.chars[ci].cons >= 6 && moonsign_level(s) >= 2 {
                    s.special_atk.elevation += 250;
                }
            }
            Event::MoondriftHarmony => {
                let cons = s.chars[ci].cons;
                // C1: Field Catalog stacks
                if cons >= 1 {
                    if !s.mod_active(ci, C1) {
                        st_mut(s, ci).c1_stacks = 0;
                    }
                    s.add_status(ci, C1, 10 * 60, true);
                    let gain = if cons >= 6 { 18 } else { 6 };
                    let x = st_mut(s, ci);
                    x.c1_stacks = (x.c1_stacks + gain).min(18);
                }
                // C2: CRIT DMG for the geo and hydro characters
                if cons >= 2 {
                    if !s.mod_active(ci, C1) {
                        st_mut(s, ci).c1_stacks = 0;
                    }
                    for i in 0..s.chars.len() {
                        if matches!(s.chars[i].element, Element::Geo | Element::Hydro) {
                            s.add_stat_mod(i, C2, st::CD, 0.4, 8 * 60, true);
                        }
                    }
                }
                // C4: DEF for the active character and for her
                if cons >= 4 {
                    let o = Owner::Char(ci as u8);
                    for i in 0..s.chars.len() {
                        s.add_mod(i, Mod::owned(C4, ModKind::Stat, o, st::DEFP, false, true, -1), 5 * 60);
                    }
                    s.add_mod(ci, Mod::owned(C4_SELF, ModKind::Stat, o, st::DEFP, false, true, -1), 5 * 60);
                }
            }
            _ => {}
        }
    }
}

/// Lunar-Crystallize's base DMG bonus: 0.7% per 100 DEF, up to 14%.
fn lcr_bonus(s: &Sim, ci: usize) -> f64 {
    (total_def_of(s, ci, false) / 100.0 * 0.007).min(0.14)
}

/// C1: Field Catalog stacks spent as flat DMG from her DEF.
fn c1_flat(s: &mut Sim, ci: usize, million: bool) -> f64 {
    if !s.mod_active(ci, C1) {
        return 0.0;
    }
    let (mut max_stacks, mut scaling) = if million { (5, 1.5) } else { (1, 0.75) };
    if s.chars[ci].cons >= 6 {
        max_stacks *= 2;
        scaling *= 1.5;
    }
    let have = st(s, ci).c1_stacks;
    if have <= 0 {
        return 0.0;
    }
    let def = total_def_of(s, ci, false);
    let stacks = have.min(max_stacks);
    st_mut(s, ci).c1_stacks -= stacks;
    def * scaling * stacks as f64
}

fn c1_on_skill(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 1 {
        return;
    }
    if !s.mod_active(ci, C1) {
        st_mut(s, ci).c1_stacks = 0;
    }
    s.add_status(ci, C1, 10 * 60, true);
    let x = st_mut(s, ci);
    x.c1_stacks = (x.c1_stacks + 6).min(18);
}

fn skill_recast(s: &mut Sim, ci: usize) -> ActInfo {
    st_mut(s, ci).recast_count += 1;
    let src = s.f;
    st_mut(s, ci).recast_src = src;
    if st(s, ci).recast_count == 3 {
        s.delete_mod(ci, RECAST);
        // the big hit, then Standard Power
        let x = st_mut(s, ci);
        x.skill_hit = 0;
        x.skill_src = src;
        a1_on_lumi(s, ci, src);
        s.add_task(Task::Char { c: ci as u8, id: TASK_MILLION, arg: 0 }, g::SKILL_MILLION_TON_HITMARK);
        s.add_status(ci, STANDARD, g::SKILL_DUR, false);
        s.add_task(Task::Char { c: ci as u8, id: TASK_LUMI, arg: src }, g::SKILL_MILLION_TON_HITMARK + g::SKILL_STANDARD_START);
    } else {
        s.add_status(ci, RECAST, g::SKILL_RECAST_DUR as i32, true);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SUMMON, arg: src }, g::SKILL_RECAST_DUR as i32);
    }
    let t = &g::SKILL_RECAST_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
}

/// Lumi's next hammer and the one after.
fn lumi_attack(s: &mut Sim, ci: usize, src: i32) {
    if st(s, ci).skill_src != src {
        return;
    }
    let (standard, sup) = (s.mod_active(ci, STANDARD), s.mod_active(ci, SUPER));
    if !standard && !sup {
        return;
    }
    let hit = st(s, ci).skill_hit;
    if sup && g::SKILL_SEQUENCE[hit] == g::SKILL_HEAVY_ATTACK {
        heavy_overdrive(s, ci);
    } else {
        let ai = pound_info(s, ci, "Lumi Pound-Pound Pummeler 1");
        s.queue_attack(ai, CB_PARTICLE, 0, 0);
        s.add_task(Task::Char { c: ci as u8, id: TASK_POUND_2, arg: src }, g::SKILL_SECOND_HIT_DELAY);
    }
    let mut delay = g::SKILL_SUPER_INTERVAL[st(s, ci).skill_hit];
    if s.mod_active(ci, STANDARD) {
        delay = g::SKILL_STANDARD_INTERVAL;
    } else {
        advance_skill_index(s, ci);
    }
    s.add_task(Task::Char { c: ci as u8, id: TASK_LUMI, arg: src }, delay);
}

/// The hammer sequence goes on only while Moondrifts are out.
fn advance_skill_index(s: &mut Sim, ci: usize) {
    if s.moondrift_count() == 0 {
        st_mut(s, ci).skill_hit = 0;
        return;
    }
    let x = st_mut(s, ci);
    x.skill_hit = (x.skill_hit + 1) % 3;
}

fn pound_info(s: &Sim, ci: usize, abil: &'static str) -> AttackInfo {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL_POUND[lvl]);
    ai.abil = abil;
    ai.blunt = true;
    ai.use_def = true;
    ai.pattern = s.circle_on_target(3.0);
    ai
}

/// A direct Lunar-Crystallize hammer on Lumi's place, snapshot now.
fn lcr_hammer(s: &mut Sim, ci: usize, abil: &'static str, mult: f64, r: f64, c2_cd: bool) {
    let mut ai = AttackInfo::new(ci, AttackTag::DirectLunarCrystallize, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 0.0, mult);
    ai.abil = abil;
    ai.blunt = true;
    ai.use_def = true;
    ai.ignore_def = 1.0;
    ai.pattern = s.circle_on_target(r);
    let mut ev = s.snapshot_event(ai, CB_PARTICLE);
    // C2: Million Ton Crush's CRIT DMG
    if c2_cd && s.chars[ci].cons >= 2 {
        ev.stats[st::CD] += 1.5;
    }
    s.queue_attack_with_snap(ev, CB_PARTICLE, 0);
    c2_moondrift(s, ci);
}

fn million_ton(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 1);
    lcr_hammer(s, ci, g::SKILL_MILLION_ABIL, g::SKILL_MILLION[lvl], 5.0, true);
}

fn heavy_overdrive(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 1);
    lcr_hammer(s, ci, g::SKILL_OVERDRIVE_ABIL, g::SKILL_HEAVY[lvl], 4.0, false);
}

/// C2: with Moonsign 2, the hammers bring Lunar-Crystallize hits from the
/// hydro and geo characters.
fn c2_moondrift(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 2 || moonsign_level(s) < 2 {
        return;
    }
    let mut contrib = 0u8;
    for i in 0..s.chars.len() {
        if matches!(s.chars[i].element, Element::Hydro | Element::Geo) {
            contrib |= 1 << i;
        }
    }
    s.do_lcr_attack_with(contrib, ci);
}

fn a1_on_lumi(s: &mut Sim, ci: usize, src: i32) {
    if s.chars[ci].ascension >= 1 {
        a1_tick(s, ci, src);
    }
}

/// A1: Geo RES -15% (-30% with Moonsign 2) around Lumi, every second.
fn a1_tick(s: &mut Sim, ci: usize, src: i32) {
    if st(s, ci).skill_src != src {
        return;
    }
    if !s.mod_active(ci, STANDARD) && !s.mod_active(ci, SUPER) {
        return;
    }
    let shred = if moonsign_level(s) >= 2 { 0.30 } else { 0.15 };
    let area = s.circle_on_target(5.0);
    if s.enemy_alive && s.enemy_within(area) {
        s.enemy_add_resist(A1, Element::Geo, -shred, 90);
    }
    s.add_task(Task::Char { c: ci as u8, id: TASK_A1, arg: src }, 60);
}
