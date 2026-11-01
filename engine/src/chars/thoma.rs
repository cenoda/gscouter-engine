//! Thoma, from gcsim's internal/characters/thoma: normal and charged
//! attacks, Blazing Blessing (the hit, the Blazing Barrier and the pyro
//! on the player), Crimson Ooyoroi (the hit, and Fiery Collapse on the
//! active character's normal attacks, which gives a stacking barrier),
//! A1, A4 and every constellation.
//!
//! As gcsim writes them: A1's shield strength reads Thoma's own
//! `shielded` tag, which nothing sets for him, so it never adds anything
//! (its stacks and statuses are still kept). C1's cooldown cuts always
//! apply (gcsim takes a shielded character to be hit). C2 lengthens the
//! burst to 18 s, C4 gives 15 energy 8 frames after the burst, and C6
//! gives the party +15% normal, charged and plunging DMG for 6 s each
//! time a barrier is made. gcsim has no plunges for him.

use super::watch::Watcher;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::thoma as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    a1_stack: i32,
    /// Blazing Barriers waiting their frame to be added (gcsim's genShield
    /// adds them a frame later), by slot.
    pending: [f64; 4],
    next_pending: u8,
    watch_src: i32,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const BURST_ICD: ModKey = ModKey::new(g::BURST_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new("thoma-a1");
const A1_ICD: ModKey = ModKey::new("thoma-a1-icd");
const C6: ModKey = ModKey::new("thoma-c6");
const SHIELD: ModKey = ModKey::new("thoma-blazing-barrier");

const CB_PARTICLE: u16 = 1 << 0;
const CB_BURST_SHIELD: u16 = 1 << 1;

/// The skill's barrier and the pyro on the player, 9 frames in (the
/// fight's queue).
const TASK_SKILL_SHIELD: u8 = 0;
/// genShield's add a frame later, with the pending slot.
const TASK_ADD_SHIELD: u8 = 1;
const TASK_TRIGGER: u8 = 2;
const TASK_TICK: u8 = 3;

/// minazuki's watcher with AnimationXingqiuN0StartDelay.
const WATCH: Watcher = Watcher { status: BURST, icd: Some(BURST_ICD), tick_on_active: false, delay_key: 2, trigger_task: TASK_TRIGGER, tick_task: TASK_TICK, fire: fiery_collapse };

impl Character for Char {
    const KEY: &'static str = "thoma";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { a1_stack: 0, pending: [0.0; 4], next_pending: 0, watch_src: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        if s.chars[ci].ascension >= 1 {
            s.shield_bonus_mods.push(ShieldBonusMod { key: A1, owner: o, expiry: -1 });
            s.subscribe(EventKind::Swap, o);
        }
        s.subscribe(EventKind::StateChange, o);
    }

    /// gcsim registers no params for him and reads none.
    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let marks = g::ATTACK_HITMARKS[n];
        let hb = g::ATTACK_HITBOXES[n];
        let oy = g::ATTACK_OFFSETS[n];
        for (i, &h) in marks.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            // N1, N2: NewCircleHitOnTargetFanAngle(Player, {Y: off}, r, fan);
            // N3, N4: NewBoxHitOnTarget(Player, {Y: off}, w, h)
            ai.pattern = if n >= 2 { s.box_on_player(0.0, oy, hb[0], hb[1]) } else { s.fan_on_player(0.0, oy, hb[0], g::ATTACK_FAN_ANGLES[n]) };
            s.queue_attack_at(ci, ai, 0, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *marks.last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGED[lvl]);
        ai.abil = "Charge";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        // NewCircleHit(Player, PrimaryTarget, nil, 0.8)
        ai.pattern = s.circle_on_target(0.8);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::SKILL[lvl]);
        ai.abil = "Blazing Blessing";
        ai.hitlag_halt = 0.06 * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        // NewCircleHitOnTargetFanAngle(Player, {Y: 1}, 3, 270)
        ai.pattern = s.fan_on_player(0.0, 1.0, 3.0, 270.0);
        // the barrier's task, then the infusion's, both at 9 on the
        // fight's queue, the barrier's queued first
        s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL_SHIELD, arg: 0 }, 9);
        s.queue_attack(ai, CB_PARTICLE, g::SKILL_HITMARK, g::SKILL_HITMARK);
        let cd = if s.chars[ci].cons >= 1 { 12 } else { 15 };
        s.set_cd_with_delay(ci, Action::Skill, cd * 60, 9);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 50.0, g::BURST[lvl]);
        ai.abil = "Crimson Ooyoroi";
        // NewCircleHitOnTarget(Player, nil, 4)
        ai.pattern = s.circle_on_player(0.0, 0.0, 4.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        let d = if s.chars[ci].cons >= 2 { 18 } else { 15 };
        s.add_status(ci, BURST, d * 60, true);
        if s.chars[ci].cons >= 4 {
            s.add_task(Task::AddEnergy { c: ci as u8, amt: 15.0 }, 8);
        }
        let cd = if s.chars[ci].cons >= 1 { 17 } else { 20 };
        s.set_cd(ci, Action::Burst, cd * 60);
        s.consume_energy(ci, 7);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Skill as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL_SHIELD => {
                let lvl = s.talent(ci, 1);
                let hp = g::SHIELDPP[lvl] * s.max_hp(ci) + g::SHIELDFLAT[lvl];
                gen_shield(s, ci, hp, false);
                s.apply_self_infusion(AURA_PYRO, 25.0, 30);
            }
            TASK_ADD_SHIELD => {
                let hp = s.st::<State>(ci).pending[arg as usize];
                let f = s.f;
                s.add_shield(Shield { owner: ci as u8, key: SHIELD, typ: ShieldType::ThomaSkill, target: -1, ele: Element::Pyro, hp, expiry: f + 8 * 60, em_bonus: 0.0 });
            }
            TASK_TRIGGER => {
                if let Some(src) = WATCH.trigger(s, ci) {
                    s.st_mut::<State>(ci).watch_src = src;
                }
            }
            TASK_TICK => {
                let current = s.st::<State>(ci).watch_src;
                WATCH.tick(s, ci, arg, current);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 18, true);
            let count = if s.rng.float64() < 0.5 { 4.0 } else { 3.0 };
            s.queue_particle(count, Element::Pyro, s.particle_delay[ci]);
        }
        if cbs & CB_BURST_SHIELD != 0 {
            // once per Fiery Collapse: its one hit on the enemy
            let lvl = s.talent(ci, 2);
            let hp = g::BURSTSHIELDPP[lvl] * s.max_hp(ci) + g::BURSTSHIELDFLAT[lvl];
            gen_shield(s, ci, hp, true);
        }
    }

    /// C6: +15% DMG to normal, charged and plunging attacks.
    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C6 && matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            ev.stats[st::DMGP] += 0.15;
        }
    }

    /// A1: reads his own `shielded` tag, never set for him (only Zhongli
    /// sets his), so it adds nothing and is never dropped.
    fn shield_bonus(_: &Sim, _: usize, _: ModKey) -> (f64, bool) {
        (0.0, false)
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::StateChange(next) => {
                if let Some(src) = WATCH.on_state(s, ci, next) {
                    s.st_mut::<State>(ci).watch_src = src;
                }
            }
            // A1's stacks reset on a swap
            Event::Swap { .. } => s.st_mut::<State>(ci).a1_stack = 0,
            _ => {}
        }
    }
}

/// summonFieryCollapse: the hit (snapshot now, damage 11 frames later),
/// whose first hit makes a barrier, and the 1 s ICD.
fn fiery_collapse(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BURSTPROC[lvl]);
    ai.abil = "Fiery Collapse";
    // A4: 2.2% of max HP as flat damage
    if s.chars[ci].ascension >= 1 {
        ai.flat_dmg = 0.022 * s.max_hp(ci);
    }
    // NewBoxHitOnTarget(Player, nil, 4.5, 8)
    ai.pattern = s.box_on_player(0.0, 0.0, 4.5, 8.0);
    s.queue_attack(ai, CB_BURST_SHIELD, 0, 11);
    s.add_status(ci, BURST_ICD, 60, true);
}

/// gcsim's genShield: A1's stack, then the amount (the skill's at least
/// what is left of the barrier, the burst's added to it, up to the
/// maximum when there is one), as a barrier added a frame later (the
/// fight's queue); C6 then buffs the party.
fn gen_shield(s: &mut Sim, ci: usize, mut hp: f64, stack: bool) {
    if !s.mod_active(ci, A1_ICD) && s.st::<State>(ci).a1_stack < 5 {
        s.st_mut::<State>(ci).a1_stack += 1;
        s.add_status(ci, A1_ICD, 18, true);
        s.add_status(ci, A1, 360, true);
    }
    if let Some(sh) = s.shields.iter().find(|x| x.typ == ShieldType::ThomaSkill) {
        hp = if stack { hp + sh.hp } else { hp.max(sh.hp) };
        let lvl = s.talent(ci, 1);
        hp = hp.min(g::SHIELDPPMAX[lvl] * s.max_hp(ci) + g::SHIELDFLATMAX[lvl]);
    }
    let st = s.st_mut::<State>(ci);
    let slot = st.next_pending;
    st.pending[slot as usize] = hp;
    st.next_pending = (slot + 1) % 4;
    s.add_task(Task::Char { c: ci as u8, id: TASK_ADD_SHIELD, arg: slot as i32 }, 1);
    if s.chars[ci].cons >= 6 {
        for i in 0..s.chars.len() {
            s.add_mod(i, Mod::owned(C6, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 360);
        }
    }
}
