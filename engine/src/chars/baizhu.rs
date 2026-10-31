//! Baizhu, from gcsim's internal/characters/baizhu: normal and charged
//! attacks, Universal Diagnosis (the Gossamer Sprite's three hits, its
//! particles and its party heal), Holistic Revivification (the Seamless
//! Shields, their heals and Spiritveins), A1, A4 and every constellation.
//!
//! As gcsim writes them: the Gossamer Sprite's party heal scales with the
//! burst's talent level while C2's heal scales with the skill's; the
//! sprite's next hit draws one random number (RandomEnemyWithinArea on the
//! one enemy); A4's bonus also covers direct lunar bloom, which the engine
//! does not have (lunar reactions are an error elsewhere).
//!
//! A Seamless Shield that expires or breaks heals the active character and
//! fires a Spiritvein; one replaced by the next while up fires a Spiritvein
//! only. Its heals give A4 to the character healed.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::baizhu as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// The skill's attack as snapshot when cast: its later hits reuse it.
    skill_atk: Option<AttackEvent>,
}

const SHIELD: ModKey = ModKey::new("baizhu-seamless-shield");
const A1_HEAL: ModKey = ModKey::new("baizhu-a1-heal-bonus");
const A1_DENDRO: ModKey = ModKey::new("baizhu-a1-dendro-dmg");
const A4: ModKey = ModKey::new("baizhu-a4");
const C2_ICD: ModKey = ModKey::new(g::C2_ICD_KEY);
const C4: ModKey = ModKey::new("baizhu-c4");

const CB_PARTICLE: u16 = 1 << 0;
/// The sprite's chain: the next hit after the first.
const CB_CHAIN1: u16 = 1 << 1;
/// The sprite's chain: the third hit after the second.
const CB_CHAIN2: u16 = 1 << 2;
/// C6: a Seamless Shield, once per sprite (one enemy, so once per hit).
const CB_C6: u16 = 1 << 3;

const TASK_SHIELD: u8 = 0;
const TASK_REFRESH: u8 = 1;
const TASK_SKILL_HEAL: u8 = 2;
const TASK_SKILL_HEAL_NOW: u8 = 3;
const TASK_C2_HEAL: u8 = 4;

const FRAMES_CHARGE: u8 = 0;

impl Character for Char {
    const KEY: &'static str = "baizhu";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // A1: heal bonus or dendro DMG by the active character's HP
        if s.chars[ci].ascension >= 1 {
            s.chars[ci].mods.push(Mod::owned(A1_HEAL, ModKind::Stat, o, st::HEAL, false, false, -1));
            s.chars[ci].mods.push(Mod::owned(A1_DENDRO, ModKind::Stat, o, st::DENDROP, false, false, -1));
        }
        // C1: two skill charges
        if s.chars[ci].cons >= 1 {
            let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
            cd.available = 2;
            cd.max_extra = 1;
        }
        if s.chars[ci].cons >= 2 {
            s.subscribe(EventKind::EnemyDamage, o);
        }
    }

    /// gcsim's package reads no params.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let marks = g::ATTACK_HITMARKS[n];
        let hb = g::ATTACK_HITBOXES[n];
        for i in 0..marks.len() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::ATTACK[n][i][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.pattern = s.box_on_player(0.0, g::ATTACK_OFFSETS[n], hb[0], hb[1]);
            s.queue_attack(ai, 0, marks[i], marks[i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *marks.last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        // a circle on the player offset by the enemy's position: on the enemy
        ai.pattern = s.circle_on_target(2.3);
        let windup = if s.current_state() == AnimState::Normal { 14 } else { 0 };
        let h = g::CHARGE_HITMARK - windup;
        s.queue_attack(ai, 0, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(
            Frames::Char { id: FRAMES_CHARGE, arg: windup as i16 },
            t[0] - windup,
            t[Action::Dash as usize] - windup,
            AnimState::Charge,
        )
    }

    fn frames(_: u8, windup: i16, next: Action) -> i32 {
        g::CHARGE_FRAMES[next as usize] - windup as i32
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::SKILL_DAMAGE[lvl]);
        ai.abil = "Universal Diagnosis";
        ai.pattern = s.circle_on_target(0.6);
        let ev = s.snapshot_event(ai, 0);
        s.st_mut::<State>(ci).skill_atk = Some(ev);
        let mut cbs = CB_PARTICLE | CB_CHAIN1;
        if s.chars[ci].cons >= 6 {
            cbs |= CB_C6;
        }
        s.queue_attack_with_snap(ev, cbs, g::SKILL_FIRST_HITMARK);
        s.set_cd_with_delay(ci, Action::Skill, 10 * 60, 23);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // the first shield heals nothing; five refreshes follow
        s.add_task(Task::Char { c: ci as u8, id: TASK_SHIELD, arg: 0 }, g::BURST_FIRST_SHIELD);
        for i in 0..=4 {
            s.add_task(
                Task::Char { c: ci as u8, id: TASK_REFRESH, arg: 0 },
                g::BURST_FIRST_SHIELD + g::BURST_FIRST_REFRESH + g::BURST_REFRESH_RATE * i,
            );
        }
        // C4: EM +80 for the party, 15 s
        if s.chars[ci].cons >= 4 {
            for i in 0..s.chars.len() {
                s.add_stat_mod(i, C4, st::EM, 80.0, 900, true);
            }
        }
        s.set_cd(ci, Action::Burst, 20 * 60);
        s.consume_energy(ci, 5);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_SHIELD => summon_shield(s, ci),
            TASK_REFRESH => {
                summon_shield(s, ci);
                shield_healing(s, ci);
            }
            TASK_SKILL_HEAL => s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_HEAL_NOW, arg: 0 }, g::SKILL_RETURN_TRAVEL),
            // the party, at the burst's talent level, as gcsim's skillHealing
            TASK_SKILL_HEAL_NOW => {
                let lvl = s.talent(ci, 2);
                let src = g::SKILL_HEAL_PP[lvl] * s.max_hp(ci) + g::SKILL_HEAL_FLAT[lvl];
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, None, false, src, bonus);
            }
            TASK_C2_HEAL => {
                let lvl = s.talent(ci, 1);
                let src = (g::SKILL_HEAL_PP[lvl] * s.max_hp(ci) + g::SKILL_HEAL_FLAT[lvl]) * 0.2;
                let bonus = s.stat(ci, st::HEAL);
                let active = s.player.active as usize;
                s.heal(ci, active, src, bonus);
            }
            _ => unreachable!(),
        }
    }

    /// The order gcsim appends them: particles, the chain, C6.
    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 {
            let count = if s.rng.float64() < 0.5 { 4.0 } else { 3.0 };
            s.queue_particle(count, Element::Dendro, s.particle_delay[ci]);
        }
        if cbs & (CB_CHAIN1 | CB_CHAIN2) != 0 {
            // RandomEnemyWithinArea: the one enemy, the same target
            let _ = s.rng.intn(1);
            let Some(ev) = s.st::<State>(ci).skill_atk else { unreachable!() };
            let delay = g::SKILL_TICK_INTERVAL;
            if cbs & CB_CHAIN1 != 0 {
                s.queue_attack_with_snap(ev, CB_CHAIN2, delay);
            } else {
                s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_HEAL, arg: 0 }, delay);
                s.queue_attack_with_snap(ev, 0, delay);
            }
        }
        if cbs & CB_C6 != 0 {
            summon_shield(s, ci);
        }
    }

    /// A1: the active character below half HP gives her +20% healing,
    /// at half or more +25% dendro DMG.
    fn mod_amount(s: &Sim, _: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let active = s.player.active as usize;
        let low = s.chars[active].hp_ratio < 0.5;
        if m.key == A1_HEAL {
            if !low {
                return false;
            }
            out[st::HEAL] += 0.2;
            return true;
        }
        if m.key == A1_DENDRO {
            if low {
                return false;
            }
            out[st::DENDROP] += 0.25;
            return true;
        }
        false
    }

    /// A4: per 1,000 of her max HP up to 50,000, burning, bloom, hyperbloom
    /// and burgeon +2%, aggravate and spread +0.8%. Those are every reaction
    /// in the engine it names; gcsim's direct lunar bloom is not one.
    fn react_bonus(s: &Sim, ci: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        let limit = (s.max_hp(ci) / 1000.0).min(50.0);
        if matches!(info.catalyzed, Some(Reaction::Aggravate) | Some(Reaction::Spread)) {
            return limit * 0.008;
        }
        match info.tag {
            AttackTag::Bloom | AttackTag::Hyperbloom | AttackTag::Burgeon | AttackTag::BurningDamage => limit * 0.02,
            _ => 0.0,
        }
    }

    /// C2: the active character's damage sends a Gossamer Sprite: Splice,
    /// once every 5 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != s.player.active as usize {
            return;
        }
        if s.mod_active(ci, C2_ICD) {
            return;
        }
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_BAIZHU_C2, Element::Dendro, 25.0, 2.5);
        ai.abil = "Gossamer Sprite: Splice. (C2)";
        ai.pattern = s.circle_on_target(0.6);
        let cbs = if s.chars[ci].cons >= 6 { CB_C6 } else { 0 };
        s.queue_attack(ai, cbs, 0, g::SKILL_FIRST_HITMARK);
        s.add_task(Task::Char { c: ci as u8, id: TASK_C2_HEAL, arg: 0 }, g::SKILL_RETURN_TRAVEL);
        s.add_status(ci, C2_ICD, 60 * 5, false);
    }

    /// A broken shield heals and fires a Spiritvein.
    fn shield_damaged(s: &mut Sim, ci: usize, _: ModKey, broken: bool) {
        if broken {
            remove_shield(s, ci);
        }
    }

    /// An expired shield heals and fires a Spiritvein.
    fn shield_expired(s: &mut Sim, ci: usize, _: ModKey) {
        remove_shield(s, ci);
    }
}

/// gcsim's summonSeamlessShield: a Spiritvein when a shield is up, then a
/// new shield (replacing it) of her max HP now, for 152 frames.
fn summon_shield(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 2);
    let hp = g::BURST_SHIELD_PP[lvl] * s.max_hp(ci) + g::BURST_SHIELD_FLAT[lvl];
    if s.shields.iter().any(|x| x.typ == ShieldType::BaizhuBurst) {
        spiritvein(s, ci);
    }
    let f = s.f;
    s.add_shield(Shield {
        owner: ci as u8,
        key: SHIELD,
        typ: ShieldType::BaizhuBurst,
        target: -1,
        ele: Element::Dendro,
        hp,
        expiry: f + g::BURST_SHIELD_EXPIRY,
        em_bonus: 0.0,
    });
}

/// gcsim's summonSeamlessShieldHealing: heals the active character, who
/// then gets A4 for 6 s.
fn shield_healing(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 2);
    let src = g::BURST_HEAL_PP[lvl] * s.max_hp(ci) + g::BURST_HEAL_FLAT[lvl];
    let bonus = s.stat(ci, st::HEAL);
    let active = s.player.active as usize;
    s.heal(ci, active, src, bonus);
    if s.chars[ci].ascension >= 4 {
        let active = s.player.active as usize;
        s.add_mod(active, Mod::owned(A4, ModKind::ReactBonus, Owner::Char(ci as u8), st::NONE, false, true, -1), 6 * 60);
    }
}

/// gcsim's removeShield.
fn remove_shield(s: &mut Sim, ci: usize) {
    shield_healing(s, ci);
    spiritvein(s, ci);
}

/// A Spiritvein: burst damage on the enemy; C6 adds 8% of her max HP.
fn spiritvein(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::BURST_ATK[lvl]);
    ai.abil = "Spiritvein Damage";
    if s.chars[ci].cons >= 6 {
        ai.flat_dmg = s.max_hp(ci) * 0.08;
    }
    ai.pattern = s.circle_on_target(1.5);
    s.queue_attack(ai, 0, g::BURST_TICK_RELEASE, g::BURST_TICK_RELEASE + g::BURST_TICK_TRAVEL);
}
