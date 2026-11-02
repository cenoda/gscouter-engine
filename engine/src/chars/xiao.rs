//! Xiao, from gcsim's internal/characters/xiao: normal and charged attacks
//! (with their larger areas under his burst), Lemniscatic Wind Cycling with
//! its charges and A4's stacks, Bane of All Evil (the anemo conversion and
//! DMG bonus his Snapshot gives normal, charged and plunging attacks, its HP
//! drain, and its end when he leaves the field), A1, his burst's jump, the
//! plunges after a jump (his own, from Xianyun's jump, and both) and every
//! constellation.
//!
//! As gcsim writes him: C4's DEF bonus reads his HP ratio at every stat
//! read, C6 needs two enemies hit by one plunge (none with a single
//! target), a plunge after a plain jump takes his burst's hitmark and area,
//! and the low plunge's hitmark is the high one's.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::xiao as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    /// qStarted: the burst's cast frame.
    q_started: i32,
    /// The last burst's HP drain: its next drain frame (hpICD), and the
    /// frame its drain tasks end.
    hp_icd: i32,
    drain_end: i32,
    a4_stacks: i32,
    c6_count: i32,
}

const BURST: ModKey = ModKey::new(g::BURST_BUFF_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const A4: ModKey = ModKey::new(g::A4_BUFF_KEY);
const C2: ModKey = ModKey::new("xiao-c2");
const C4: ModKey = ModKey::new("xiao-c4");
const C6: ModKey = ModKey::new(g::C6_BUFF_KEY);

const CB_PARTICLE: u16 = 1 << 0;
const CB_C6: u16 = 1 << 1;

/// A4's stack, 15 frames after the skill.
const TASK_A4: u8 = 0;
/// The burst's HP drain, with the burst's cast frame.
const TASK_DRAIN: u8 = 1;

const NORMAL_NAMES: [&str; 6] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4", "Normal 5"];

impl Character for Char {
    const KEY: &'static str = "xiao";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        let cons = s.chars[ci].cons;
        let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
        if cons >= 1 {
            cd.available = 3;
            cd.max_extra = 2;
        } else {
            cd.available = 2;
            cd.max_extra = 1;
        }
        // onExitField
        s.subscribe(EventKind::Swap, o);
        if cons >= 2 {
            s.chars[ci].mods.push(Mod::owned(C2, ModKind::Stat, o, st::ER, false, false, -1));
        }
        if cons >= 4 {
            s.chars[ci].mods.push(Mod::owned(C4, ModKind::Stat, o, st::DEFP, false, false, -1));
        }
    }

    /// gcsim validates collision on the plunges and reads no params of the
    /// other actions.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            Action::LowPlunge | Action::HighPlunge => p.0.iter().find(|(k, _)| k != "collision").map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    /// His plunges come only after a jump (gcsim's error in the plunge).
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        if matches!(a, Action::LowPlunge | Action::HighPlunge) && s.current_state() != AnimState::Jump {
            return Some("only plunge after using jump".into());
        }
        crate::content::template_invalid(s, a)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let bi = if s.mod_active(ci, BURST) { 1 } else { 0 };
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let (ox, oy) = (g::ATTACK_OFFSETS[n][i][0], g::ATTACK_OFFSETS[n][i][1]);
            let hb = g::ATTACK_HITBOXES[bi][n][i];
            ai.pattern = if (n == 0 && i == 1) || n == 4 { s.box_on_player(ox, oy, hb[0], hb[1]) } else { s.fan_on_player(ox, oy, hb[0], g::ATTACK_FAN_ANGLES[n][i]) };
            s.queue_attack_at(ci, ai, 0, g::ATTACK_HITMARKS[n][i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let marks = g::ATTACK_HITMARKS[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], marks[marks.len() - 1], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge";
        ai.hitlag_halt = 0.02 * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        let r = if s.mod_active(ci, BURST) { 3.2 } else { 3.0 };
        ai.pattern = s.fan_on_player(0.0, 0.0, r, 200.0);
        s.queue_attack(ai, 0, g::CHARGE_HITMARK, g::CHARGE_HITMARK);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARK, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_XIAO_DASH, Element::Anemo, 25.0, g::SKILL[lvl]);
        ai.abil = "Lemniscatic Wind Cycling";
        // NewCircleHit(Player, PrimaryTarget, nil, 0.8)
        ai.pattern = s.circle_toward_target(0.0, 0.0, 0.8);
        // no energy during the burst
        let cb = if s.mod_active(ci, BURST) { 0 } else { CB_PARTICLE };
        s.queue_attack(ai, cb, 0, g::SKILL_HITMARK);
        if s.chars[ci].ascension >= 4 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_A4, arg: 0 }, 15);
        }
        // C6: no charge used and no cooldown
        if !(s.chars[ci].cons >= 6 && s.mod_active(ci, C6)) {
            s.set_cd(ci, Action::Skill, 600);
        }
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Skill as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let f = s.f;
        if s.st::<State>(ci).drain_end > f {
            s.unsupported("xiao: a burst while the last one's HP drain is still queued");
        }
        s.add_status(ci, BURST, 900 + g::BURST_START, true);
        {
            let st = s.st_mut::<State>(ci);
            st.q_started = f;
            st.hp_icd = 0;
            st.drain_end = f + 900 + g::BURST_START;
        }
        // a1
        if s.chars[ci].ascension >= 1 {
            s.add_mod(ci, Mod::owned(A1, ModKind::Stat, Owner::Char(ci as u8), st::DMGP, false, true, -1), 900 + g::BURST_START);
        }
        // gcsim checks the drain every frame from 1 s after the animation,
        // once a second while the burst lasts: the burst cannot come back
        // while these are queued (the check above), so only the frames a
        // drain can happen on are queued
        let mut i = g::BURST_START + 60;
        while i < 900 + g::BURST_START {
            s.add_task(Task::Char { c: ci as u8, id: TASK_DRAIN, arg: f }, i);
            i += 60;
        }
        s.set_cd_with_delay(ci, Action::Burst, 18 * 60, 29);
        s.consume_energy(ci, 36);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    /// The burst's jump, unless Xianyun's buff makes it hers.
    fn own_jump(s: &mut Sim, ci: usize, _: &Params) -> Option<ActInfo> {
        if !s.mod_active(ci, BURST) || s.mod_active(ci, XIANYUN_AIRBORNE) {
            return None;
        }
        let t = &g::BURST_JUMP_FRAMES;
        Some(ActInfo::new(Frames::Table(t), t[0], t[Action::LowPlunge as usize], AnimState::Jump))
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge(s, ci, false, p)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge(s, ci, true, p)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_A4 => {
                if !s.mod_active(ci, A4) {
                    s.st_mut::<State>(ci).a4_stacks = 0;
                }
                let st = s.st_mut::<State>(ci);
                st.a4_stacks = (st.a4_stacks + 1).min(3);
                s.add_mod(ci, Mod::owned(A4, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 420);
            }
            TASK_DRAIN => {
                let st = s.st::<State>(ci);
                if st.q_started != arg || !s.mod_active(ci, BURST) || s.f < st.hp_icd {
                    return;
                }
                s.st_mut::<State>(ci).hp_icd = s.f + 60;
                let amt = g::BURST_DRAIN[s.talent(ci, 2)] * s.current_hp(ci);
                s.drain_info(ci, amt, false, "Bane of All Evil");
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 {
            s.queue_particle(3.0, Element::Anemo, s.particle_delay[ci]);
        }
        // c6cb: two enemies hit by one plunge
        if cbs & CB_C6 != 0 && s.mod_active(ci, BURST) && !s.mod_active(ci, C6) {
            s.st_mut::<State>(ci).c6_count += 1;
            if s.st::<State>(ci).c6_count == 2 {
                s.reset_cd(ci, Action::Skill);
                s.add_status(ci, C6, 72, true);
                s.st_mut::<State>(ci).c6_count = 0;
            }
        }
    }

    /// His Snapshot under the burst: anemo normal, charged and plunging
    /// attacks, the charged attack on the normal attacks' ICD, and their
    /// hitlag.
    fn snapshot(s: &Sim, ci: usize, info: &mut AttackInfo) {
        if !s.mod_active(ci, BURST) {
            return;
        }
        match info.tag {
            AttackTag::Normal => {
                // QN1's hitlag differs from N1's
                if info.abil == "Normal 0" {
                    info.hitlag_halt = 0.01 * 60.0;
                }
            }
            AttackTag::Extra => {
                info.icd_tag = icd::TAG_NORMAL_ATTACK;
                info.hitlag_halt = 0.04 * 60.0;
            }
            AttackTag::Plunge => {}
            _ => return,
        }
        info.element = Element::Anemo;
    }

    /// The burst's DMG bonus on the attacks its Snapshot converts.
    fn snapshot_stats(s: &Sim, ci: usize, info: &AttackInfo, stats: &mut Stats) {
        if s.mod_active(ci, BURST) && matches!(info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            stats[st::DMGP] += g::BURST_BONUS[s.talent(ci, 2)];
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        let k = m.key;
        // A1: 5% DMG, 5% more every 3 s of the burst, up to 25%
        if k == A1 {
            let stacks = (1 + (s.f - s.st::<State>(ci).q_started) / 180).min(5);
            out[st::DMGP] += stacks as f64 * 0.05;
            true
        // C2: ER +25% off the field
        } else if k == C2 {
            if s.player.active as usize == ci {
                return false;
            }
            out[st::ER] += 0.25;
            true
        // C4: DEF +100% at or below half HP
        } else if k == C4 {
            if s.chars[ci].hp_ratio > 0.5 {
                return false;
            }
            out[st::DEFP] += 1.0;
            true
        } else {
            false
        }
    }

    /// A4: the skill's DMG +15% a stack.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A4 && ev.info.tag == AttackTag::ElementalArt {
            ev.stats[st::DMGP] += s.st::<State>(ci).a4_stacks as f64 * 0.15;
        }
    }

    /// onExitField: any swap ends the burst and A1.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Swap { .. } = *e {
            s.delete_mod(ci, BURST);
            s.delete_mod(ci, A1);
        }
    }
}

/// LowPlungeAttack and HighPlungeAttack: from Xianyun's jump under the
/// burst, from her jump, or (gcsim assumes the burst) his own.
fn plunge(s: &mut Sim, ci: usize, high: bool, p: &Params) -> ActInfo {
    let xy = s.mod_active(ci, XIANYUN_AIRBORNE);
    let burst = s.mod_active(ci, BURST);
    #[rustfmt::skip]
    let (t, q, poise, r, hitmark, collision) = match (high, xy, burst) {
        (false, true, true) => (&g::LOW_PLUNGE_FRAMES_XYX, Action::Skill, 150.0, 4.0, g::LOW_PLUNGE_HITMARK_XYX, g::COLLISION_HITMARK_XYX),
        (false, true, false) => (&g::LOW_PLUNGE_FRAMES_XY, Action::Skill, 100.0, 3.0, g::LOW_PLUNGE_HITMARK_XY, g::COLLISION_HITMARK_XY),
        // gcsim's low plunge from his own jump takes the high one's hitmark
        (false, false, _) => (&g::LOW_PLUNGE_FRAMES_X, Action::Skill, 150.0, 4.0, g::HIGH_PLUNGE_HITMARK_X, g::COLLISION_HITMARK_X),
        (true, true, true) => (&g::HIGH_PLUNGE_FRAMES_XYX, Action::Attack, 225.0, 6.0, g::HIGH_PLUNGE_HITMARK_XYX, g::COLLISION_HITMARK_XYX),
        (true, true, false) => (&g::HIGH_PLUNGE_FRAMES_XY, Action::Attack, 150.0, 5.0, g::HIGH_PLUNGE_HITMARK_XY, g::COLLISION_HITMARK_XY),
        (true, false, _) => (&g::HIGH_PLUNGE_FRAMES_X, Action::Attack, 225.0, 6.0, g::HIGH_PLUNGE_HITMARK_X, g::COLLISION_HITMARK_X),
    };
    let lvl = s.talent(ci, 0);
    if p.get("collision").unwrap_or(0) > 0 {
        let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 0.0, g::PLUNGE[lvl]);
        ai.abil = "Plunge Collision";
        ai.pattern = s.circle_on_player(0.0, 0.0, 1.0);
        s.queue_attack(ai, 0, collision, collision);
    }
    let mult = if high { g::HIGH_PLUNGE[lvl] } else { g::LOW_PLUNGE[lvl] };
    let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult);
    ai.abil = if high { "High Plunge" } else { "Low Plunge" };
    ai.blunt = true;
    ai.poise = poise;
    ai.pattern = s.circle_on_player(0.0, 0.0, r);
    // c6cb
    let c6 = if s.chars[ci].cons >= 6 {
        s.st_mut::<State>(ci).c6_count = 0;
        CB_C6
    } else {
        0
    };
    s.queue_attack(ai, c6, hitmark, hitmark);
    ActInfo::new(Frames::Table(t), t[0], t[q as usize], AnimState::Plunge)
}
