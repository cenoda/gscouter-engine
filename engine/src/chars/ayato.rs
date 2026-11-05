//! Kamisato Ayato, from gcsim's internal/characters/ayato: normal attacks
//! and, in Kyouka's state, Shunsuiken (Namisen's flat damage at the
//! snapshot, the hydro infusion, three strikes before the counter wraps),
//! the charged attack, Kamisato Art: Kyouka (its water illusion and A1),
//! Kamisato Art: Suiyuu (its drops on the unmarked enemy in the area or a
//! random point, the active member's normal attack bonus), A4's energy off
//! field, the airborne plunges and every constellation.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gamedata::chars::ayato as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    stacks: i32,
    stacks_max: i32,
    c6_ready: bool,
    /// C1 (damage mode): the enemy's HP at the start.
    enemy_max_hp: f64,
    /// The normal attacks' hit areas, made when the attack starts.
    normal_patterns: [[Pattern; 2]; 5],
    /// The burst's area center at the cast and its snapshot, taken when
    /// the circle forms.
    burst_center: (f64, f64),
    burst_ev: AttackEvent,
}

const SKILL: ModKey = ModKey::new(g::SKILL_BUFF_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST_MARK: ModKey = ModKey::new(g::BURST_MARK_KEY);
const BURST_BUFF: ModKey = ModKey::new("ayato-burst");
const C1: ModKey = ModKey::new("ayato-c1");
const C2: ModKey = ModKey::new("ayato-c2");
const C4: ModKey = ModKey::new("ayato-c4");

const CB_PARTICLE: u16 = 1 << 0;
const CB_STACKS: u16 = 1 << 1;
const CB_C6: u16 = 1 << 2;

/// A normal attack's hit: arg is attack * 2 + hit (his queue).
const TASK_NORMAL: u8 = 0;
/// The water illusion's explosion (the fight's queue).
const TASK_ILLUSION: u8 = 1;
/// The burst's snapshot when the circle forms (the fight's queue).
const TASK_BURST_SNAP: u8 = 2;
/// A tick of the burst (the fight's queue).
const TASK_BURST_TICK: u8 = 3;
/// A4: 2 energy a second off field below 40 (the fight's queue).
const TASK_A4: u8 = 4;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];
const SHUNSUIKEN_NAMES: [&str; 3] = ["Shunsuiken 0", "Shunsuiken 1", "Shunsuiken 2"];
/// Shunsuiken's strikes before the counter wraps.
const SHUNSUIKEN_COUNTER: i32 = 3;

/// The airborne plunges (Xianyun's), from gcsim's plunge.go.
const PLUNGE: plunge::Spec = plunge::Spec::physical(
    g::LOW_PLUNGE_HITMARK,
    g::HIGH_PLUNGE_HITMARK,
    g::COLLISION_HITMARK,
    &g::LOW_PLUNGE_FRAMES,
    &g::HIGH_PLUNGE_FRAMES,
    g::LOW_PLUNGE_POISE_DMG,
    g::HIGH_PLUNGE_POISE_DMG,
    g::LOW_PLUNGE_RADIUS,
    g::HIGH_PLUNGE_RADIUS,
    g::LOW_PLUNGE,
    g::HIGH_PLUNGE,
    g::COLLISION,
);

impl Character for Char {
    const KEY: &'static str = "kamisatoayato";
    type State = State;

    fn new_state(cs: &CharSetup) -> State {
        let ev = AttackEvent::new(AttackInfo::new(0, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0), 0);
        State {
            stacks: 0,
            stacks_max: if cs.cons >= 2 { 5 } else { 4 },
            c6_ready: false,
            enemy_max_hp: 0.0,
            normal_patterns: [[Pattern::Single; 2]; 5],
            burst_center: (0.0, 0.0),
            burst_ev: ev,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        a4(s, ci);
        s.subscribe(EventKind::Swap, o);
        // C1 (damage mode only): normal attacks +40% on an enemy at half HP
        // or less
        if s.chars[ci].cons >= 1 && s.damage_mode {
            s.st_mut::<State>(ci).enemy_max_hp = s.enemy_hp;
            s.chars[ci].mods.push(Mod::owned(C1, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        // C2: HP +50% with 3 Namisen stacks or more
        if s.chars[ci].cons >= 2 {
            s.chars[ci].mods.push(Mod::owned(C2, ModKind::Stat, o, st::HPP, false, false, -1));
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Skill => p.0.iter().find(|(k, _)| k != "illusion_delay").map(|(k, _)| format!("skill param {k}")),
            Action::Attack | Action::Charge | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// Plunges only while airborne; no charged attack or plunge in Kyouka's
    /// state (gcsim's errors).
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if s.mod_active(ci, SKILL) {
            match a {
                Action::Charge => return Some("charged attack called in skill state".into()),
                Action::LowPlunge => return Some("cannot use low_plunge while in skill state".into()),
                Action::HighPlunge => return Some("cannot use high_plunge while in skill state".into()),
                _ => {}
            }
        }
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    /// Xingqiu's N0 delay: 17 frames in Kyouka's state, 15 otherwise.
    fn start_delay(s: &Sim, ci: usize, key: usize) -> Option<i32> {
        (key == 2).then(|| if s.mod_active(ci, SKILL) { 17 } else { 15 })
    }

    /// Kyouka's state: normal and charged attacks are hydro, and all but
    /// C6's strikes add Namisen's flat damage.
    fn snapshot(s: &Sim, ci: usize, info: &mut AttackInfo) {
        if !s.mod_active(ci, SKILL) || !matches!(info.tag, AttackTag::Normal | AttackTag::Extra) {
            return;
        }
        info.element = Element::Hydro;
        if info.abil == g::C6_ABIL {
            return;
        }
        let stacks = s.st::<State>(ci).stacks;
        info.flat_dmg += s.max_hp(ci) * g::SKILLPP[s.talent(ci, 1)] * stacks as f64;
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, SKILL) {
            return shunsuiken(s, ci);
        }
        let n = s.chars[ci].normal_counter as usize;
        let hits = g::ATTACK_HITMARKS[n];
        let hb = g::ATTACK_HITBOXES[n];
        let oy = g::ATTACK_OFFSETS[n];
        // gcsim makes the hit areas when the attack starts
        let p = if n >= 2 { s.box_on_player(0.0, oy, hb[0], hb[1]) } else { s.circle_on_player(0.0, oy, hb[0]) };
        for (i, &h) in hits.iter().enumerate() {
            s.st_mut::<State>(ci).normal_patterns[n][i] = p;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: (n * 2 + i) as i32 }, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], hits[hits.len() - 1], AnimState::Normal);
        advance_normal(s, ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CA[lvl]);
        ai.abil = "Charge";
        ai.pattern = s.circle_toward_target(0.0, 0.0, 0.8);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    /// Kamisato Art: Kyouka: the state for 6 s (A1's two stacks), and the
    /// water illusion's explosion `illusion_delay` frames later (35 to 360).
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let delay = (p.get("illusion_delay").unwrap_or(0) as i32).max(35).min(360);
        if s.chars[ci].ascension >= 1 {
            s.st_mut::<State>(ci).stacks = 2;
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_ILLUSION, arg: 0 }, delay);
        s.add_status(ci, SKILL, 6 * 60, true);
        if s.chars[ci].cons >= 6 {
            s.st_mut::<State>(ci).c6_ready = true;
        }
        s.set_cd(ci, Action::Skill, 12 * 60);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    /// Kamisato Art: Suiyuu: a drop every 30 frames for 18 s from 101
    /// frames in, snapshot then; C4's ATK SPD for the party.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let c = ci as u8;
        s.add_task(Task::Char { c, id: TASK_BURST_SNAP, arg: 0 }, g::BURST_START);
        let center = s.player_pos;
        s.st_mut::<State>(ci).burst_center = center;
        let mut i = 0;
        while i < 18 * 60 {
            s.add_task(Task::Char { c, id: TASK_BURST_TICK, arg: 0 }, i + g::BURST_START);
            i += 30;
        }
        if s.chars[ci].cons >= 4 {
            for k in 0..s.chars.len() {
                s.add_stat_mod(k, C4, st::ATKSPD, 0.15, 15 * 60, true);
            }
        }
        s.set_cd(ci, Action::Burst, 20 * 60);
        s.consume_energy(ci, 5);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
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
                ai.pattern = s.st::<State>(ci).normal_patterns[n][i];
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_ILLUSION => {
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::SKILL[lvl]);
                ai.abil = "Kamisato Art: Kyouka";
                // on the player, where it is now
                ai.pattern = s.circle_on_player(0.0, 0.0, 3.5);
                s.queue_attack(ai, 0, 0, 0);
                // A1: the explosion fills Namisen
                if s.chars[ci].ascension >= 1 {
                    let st = s.st_mut::<State>(ci);
                    st.stacks = st.stacks_max;
                }
            }
            TASK_BURST_SNAP => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::BURST[lvl]);
                ai.abil = "Kamisato Art: Suiyuu";
                let ev = s.snapshot_event(ai, 0);
                s.st_mut::<State>(ci).burst_ev = ev;
            }
            TASK_BURST_TICK => burst_tick(s, ci),
            TASK_A4 => a4(s, ci),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 114, true);
            let count = if s.rng.float64() < 0.5 { 2.0 } else { 1.0 };
            s.queue_particle(count, Element::Hydro, s.particle_delay[ci]);
        }
        if cbs & CB_STACKS != 0 {
            let st = s.st_mut::<State>(ci);
            if st.stacks < st.stacks_max {
                st.stacks += 1;
            }
        }
        // C6: after Kyouka, the first Shunsuiken hit while he is on field
        // adds two strikes
        if cbs & CB_C6 != 0 && s.player.active as usize == ci && s.st::<State>(ci).c6_ready {
            s.st_mut::<State>(ci).c6_ready = false;
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 4.5);
            ai.abil = g::C6_ABIL;
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = 0.03 * 60.0;
            ai.deployable = true;
            ai.pattern = s.box_on_player(0.0, 0.0, 8.0, 7.0);
            for i in 0..2 {
                s.queue_attack(ai, 0, 20 + i * 2, 20 + i * 2);
            }
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C2 && s.st::<State>(ci).stacks >= 3 {
            out[st::HPP] += 0.5;
            return true;
        }
        false
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag != AttackTag::Normal {
            return;
        }
        if m.key == BURST_BUFF {
            ev.stats[st::DMGP] += g::BURSTATKP[s.talent(ci, 2)];
        } else if m.key == C1 && s.enemy_hp / s.st::<State>(ci).enemy_max_hp <= 0.5 {
            ev.stats[st::DMGP] += 0.4;
        }
    }

    /// Leaving the field ends Kyouka and its stacks, and starts A4.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Swap { prev, .. } = *e {
            if prev != ci {
                return;
            }
            s.st_mut::<State>(ci).stacks = 0;
            s.delete_mod(ci, SKILL);
            s.add_task(Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, 60);
        }
    }
}

/// gcsim's AdvanceNormalIndex of his: three strikes in Kyouka's state.
fn advance_normal(s: &mut Sim, ci: usize) {
    let skill = s.mod_active(ci, SKILL);
    let c = &mut s.chars[ci];
    c.normal_counter += 1;
    let wrap = if skill { SHUNSUIKEN_COUNTER } else { c.normal_hit_num };
    if c.normal_counter == wrap {
        c.normal_counter = 0;
    }
}

/// Shunsuiken: a hydro strike 5 frames in (its particles, a Namisen
/// stack, C6), its frames by the ATK SPD at the start.
fn shunsuiken(s: &mut Sim, ci: usize) -> ActInfo {
    let n = s.chars[ci].normal_counter as usize;
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::SHUNSUIKEN[n][lvl]);
    ai.abil = SHUNSUIKEN_NAMES[n];
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = 0.03 * 60.0;
    ai.pattern = s.box_on_player(0.0, 0.0, 8.0, 7.0);
    let mut cbs = CB_PARTICLE | CB_STACKS;
    // makeC6CB: none unless ready now
    if s.chars[ci].cons >= 6 && s.st::<State>(ci).c6_ready {
        cbs |= CB_C6;
    }
    s.queue_attack(ai, cbs, 0, g::SHUNSUIKEN_HITMARK);
    advance_normal(s, ci);
    let atkspd = s.stat(ci, st::ATKSPD);
    let table = &g::SHUNSUIKEN_FRAMES;
    ActInfo::new(Frames::Attack { table, atkspd }, table[0], g::SHUNSUIKEN_HITMARK, AnimState::Normal)
}

/// A tick of Suiyuu: a drop on the enemy when it is in the area and
/// unmarked (marked for 1.45 s), else on a random point, 38 frames later;
/// the active member in the area gets the normal attack bonus for 1.5 s.
fn burst_tick(s: &mut Sim, ci: usize) {
    let (cx, cy) = s.st::<State>(ci).burst_center;
    let area = Pattern::Circle { x: cx, y: cy, r: 10.0 };
    let pos = if s.enemy_alive && s.enemy_within(area) && !s.enemy_status_active(BURST_MARK) {
        let _ = s.rng.intn(1);
        s.enemy_add_status(BURST_MARK, 87, true);
        s.enemy_pos
    } else {
        s.random_point(cx, cy, 1.5, 9.5)
    };
    let mut ev = s.st::<State>(ci).burst_ev;
    ev.info.pattern = Pattern::Circle { x: pos.0, y: pos.1, r: 2.5 };
    s.queue_attack_with_snap(ev, 0, 38);
    if !s.player_within(area) {
        return;
    }
    let active = s.player.active as usize;
    s.add_mod(active, Mod::owned(BURST_BUFF, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 90);
}

/// A4: off field below 40 energy, 2 energy now and the check again in a
/// second.
fn a4(s: &mut Sim, ci: usize) {
    if s.chars[ci].ascension < 4 || s.player.active as usize == ci || s.chars[ci].energy >= 40.0 {
        return;
    }
    s.add_energy(ci, 2.0);
    s.add_task(Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, 60);
}
