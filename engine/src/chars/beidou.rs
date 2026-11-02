//! Beidou, from gcsim's internal/characters/beidou: normal attacks, the
//! skill (param counter 0 to 2, its shield on her until the hit, A4 at 2),
//! the burst (its bounces off the active character's normal and charged
//! hits, C1's shield, C6's shred), and C1 to C6. gcsim gives her no
//! charged attack or plunges, and leaves A1 out; they stay errors.
//!
//! The bounce chains on to a random other enemy in reach (gcsim's
//! RandomEnemyWithinArea filtered to other enemies): with the one enemy
//! there is none, and no draw is made.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::beidou as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The bounce, snapshot at the burst (gcsim's burstAtk).
    burst_atk: AttackEvent,
}

const BURST: ModKey = ModKey::new(g::BURST_KEY);
const BURST_ICD: ModKey = ModKey::new(g::BURST_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const SHIELD: ModKey = ModKey::new("beidou-shield");
const C1_SHIELD: ModKey = ModKey::new("beidou-c1-shield");
const A4_DMG: ModKey = ModKey::new("beidou-a4-dmg");
const A4_SPD: ModKey = ModKey::new("beidou-a4-atkspd");
const C4: ModKey = ModKey::new(g::C4_KEY);
const C6: ModKey = ModKey::new("beidouc6");

/// The skill's particles, 2 + counter: bit 0 + counter.
const CB_PARTICLE: u16 = 1 << 0;
const CB_C4: u16 = 1 << 3;

const TASK_C6: u8 = 0;

const SKILL_NAMES: [&str; 3] = ["Tidecaller", "Tidecaller (Level 1)", "Tidecaller (Level 2)"];

impl Character for Char {
    const KEY: &'static str = "beidou";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { burst_atk: AttackEvent::new(AttackInfo::new(0, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 0.0, 0.0), 0) }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        s.subscribe(EventKind::EnemyDamage, o);
        if s.chars[ci].cons >= 4 {
            s.subscribe(EventKind::Drain, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Skill => p.0.iter().find(|(k, _)| k != "counter").map(|(k, _)| format!("{name} param {k}")),
            Action::Attack | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    /// gcsim indexes its tables with the counter: a negative one panics.
    fn invalid(s: &Sim, _: usize, a: Action, p: &Params) -> Option<String> {
        if a == Action::Skill && p.get("counter").unwrap_or(0) < 0 {
            return Some("beidou: negative counter".into());
        }
        crate::content::template_invalid(s, a)
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
        let (off, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
        ai.pattern = if n == 1 || n == 4 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.fan_on_player(0.0, off, hb[0], g::ATTACK_FAN_ANGLES[n]) };
        let h = g::ATTACK_HITMARKS[n];
        let cbs = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
        s.queue_attack(ai, cbs, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let mut counter = p.get("counter").unwrap_or(0) as usize;
        if counter >= 2 {
            counter = 2;
            // A4: normal and charged attacks after a perfect counter
            if s.chars[ci].ascension >= 4 {
                s.add_mod(ci, Mod::owned(A4_DMG, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 600);
                s.add_stat_mod(ci, A4_SPD, st::ATKSPD, 0.15, 600, true);
            }
        }
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 50.0, g::SKILLBASE[lvl] + g::SKILLBONUS[lvl] * counter as f64);
        ai.abil = SKILL_NAMES[counter];
        ai.blunt = true;
        ai.poise = 100.0 * (counter + 1) as f64;
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::SKILL_HITLAG_STAGES[counter] * 60.0;
        ai.def_halt = true;
        ai.pattern = s.circle_on_player(0.0, 0.0, g::SKILL_RADIUS[counter]);
        s.queue_attack(ai, CB_PARTICLE << counter, g::SKILL_HITMARK, g::SKILL_HITMARK);
        // her shield until the hit
        let hp = g::SHIELD_PER[lvl] * s.max_hp(ci) + g::SHIELD_BASE[lvl];
        let f = s.f;
        s.add_shield(Shield { owner: ci as u8, key: SHIELD, typ: ShieldType::BeidouThunderShield, target: ci as i8, ele: Element::Electro, hp, expiry: f + g::SKILL_HITMARK, em_bonus: 0.0 });
        s.set_cd_with_delay(ci, Action::Skill, 450, 4);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 100.0, g::BURSTONHIT[lvl]);
        ai.abil = "Stormbreaker (Initial)";
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = 0.1 * 60.0;
        ai.pattern = s.circle_on_player(0.0, 0.0, 4.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        let dur = 15 * 60;
        // not hitlag extended
        s.add_status(ci, BURST, dur, false);
        let mut proc = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::BURSTPROC[lvl]);
        proc.abil = "Stormbreaker (Bounce)";
        proc.pattern = Pattern::Single;
        let snap = s.snapshot_event(proc, 0);
        s.st_mut::<State>(ci).burst_atk = snap;
        if s.chars[ci].cons >= 1 {
            let hp = 0.16 * s.max_hp(ci);
            let f = s.f;
            s.add_shield(Shield { owner: ci as u8, key: C1_SHIELD, typ: ShieldType::BeidouC1, target: -1, ele: Element::Electro, hp, expiry: f + dur, em_bonus: 0.0 });
        }
        if s.chars[ci].cons >= 6 {
            let mut i = 30;
            while i <= dur {
                s.add_task(Task::Char { c: ci as u8, id: TASK_C6, arg: 0 }, i);
                i += 30;
            }
        }
        s.consume_energy(ci, 6);
        s.set_cd(ci, Action::Burst, 1200);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, _: usize, id: u8, _: i32) {
        match id {
            // C6: electro shred on enemies near the player
            TASK_C6 => {
                let area = s.circle_on_player(0.0, 0.0, 5.0);
                if s.enemy_alive && s.enemy_within(area) {
                    s.enemy_add_resist(C6, Element::Electro, -0.15, 90);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        for counter in 0..3 {
            if cbs & (CB_PARTICLE << counter) != 0 && !s.mod_active(ci, PARTICLE_ICD) {
                s.add_status(ci, PARTICLE_ICD, 24, true);
                s.queue_particle(2.0 + counter as f64, Element::Electro, s.particle_delay[ci]);
            }
        }
        // C4: an extra electro hit after being hit
        if cbs & CB_C4 != 0 && s.mod_active(ci, C4) {
            let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Electro, 25.0, 0.2);
            ai.abil = "Stunning Revenge (C4)";
            ai.pattern = Pattern::Single;
            s.queue_attack(ai, 0, 0, 1);
        }
    }

    /// A4: normal and charged attack DMG.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) {
            ev.stats[st::DMGP] += 0.15;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // the burst's bounce off the active character's normal and
            // charged hits
            Event::EnemyDamage(ev) => {
                if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) || ev.info.actor != s.player.active || !s.mod_active(ci, BURST) || s.mod_active(ci, BURST_ICD) {
                    return;
                }
                let mut atk = s.st::<State>(ci).burst_atk;
                atk.snap_f = s.f;
                s.queue_attack_with_snap(atk, 0, 1);
                // tied to the deployable, so not hitlag extended
                s.add_status(ci, BURST_ICD, 60, false);
            }
            // C4: hit while on field
            Event::Drain { external: true, .. } => {
                if s.player.active as usize == ci {
                    s.add_status(ci, C4, 600, true);
                }
            }
            _ => {}
        }
    }
}
