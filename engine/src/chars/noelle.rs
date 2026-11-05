//! Noelle, from gcsim's internal/characters/noelle: normal attacks, the
//! claymore spin (charged attacks held until `final` or the 5 s limit,
//! `no_limit`, with stamina drained per tick), the plunges from Xianyun's
//! airborne state, the skill's Breastplate (shield, heal on hit), the
//! burst's DEF-to-ATK conversion and Geo infusion that nothing overrides,
//! A1's shield on a low-HP active character, A4 and every constellation.
//!
//! gcsim's charged attack marks the spin's later segments `Segmented` for
//! OnStateChange; only Gest of the Mighty Wolf reads it, and the engine
//! does not have that weapon (port the flag with it).

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gamedata::chars::noelle as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

/// gcsim's ChargeState.
#[derive(Default, Clone, Copy)]
struct ChargeState {
    counter: i32,
    start_f: i32,
    ended: bool,
    error: bool,
}

/// The current charged attack's frames function (gcsim's closure).
#[derive(Default, Clone, Copy)]
struct ChargeFrames {
    windup: i32,
    hitmark: i32,
    spin: i32,
    trans: i32,
    fin: bool,
    atkspd: f64,
}

#[derive(Default)]
pub struct State {
    shield_timer: i32,
    a4_counter: i32,
    /// The burst's ATK from DEF (gcsim's burstBuff[ATK]).
    burst_atk: f64,
    ca: ChargeState,
    ca_frames: ChargeFrames,
    /// The charged attack's QueueAction tasks still to run, by delay, in
    /// the order gcsim added them (one at a time goes in ActInfo.queued).
    queue: [(i32, u8, i32); 5],
    queue_len: usize,
    queue_next: usize,
}

const A1_ICD: ModKey = ModKey::new(g::A1_ICD_KEY);
const BURST_BUFF: ModKey = ModKey::new(g::BURST_BUFF_KEY);
const C2: ModKey = ModKey::new("noelle-c2-dmg");
const SHIELD: ModKey = ModKey::new("noelle-skill-shield");
const A1_SHIELD: ModKey = ModKey::new("noelle-a1-shield");

/// gcsim's skillHealCB and makeA4CB.
const CB_HEAL: u16 = 1 << 0;
const CB_A4: u16 = 1 << 1;

const TASK_NORMAL: u8 = 0;
const TASK_SKILL: u8 = 1;
const TASK_C4_TIMER: u8 = 2;
// arg: 0 for the burst's hit, 1 for its skill hit
const TASK_BURST: u8 = 3;
// the charged attack's QueueAction tasks
const TASK_CA_ERROR: u8 = 4;
const TASK_CA_STAMINA_START: u8 = 5;
const TASK_CA_HIT: u8 = 6;
const TASK_CA_RESET: u8 = 7;
const TASK_CA_LIMIT: u8 = 8;
const TASK_CA_STAMINA: u8 = 9;
// a core task (gcsim's Tasks.Add(..., 0) around the hit)
const TASK_CA_QUEUE_HIT: u8 = 10;

const FR_CHARGE: u8 = 0;
const REMOVED_CHARGE: u8 = 0;

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

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn burst_active(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, BURST_BUFF)
}

impl Character for Char {
    const KEY: &'static str = "noelle";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        if s.chars[ci].ascension >= 1 {
            s.subscribe(EventKind::Drain, o);
        }
        if s.chars[ci].cons >= 2 {
            s.add_mod(ci, Mod::owned(C2, ModKind::Attack, o, st::NONE, false, false, -1), -1);
            s.noelle_c2 = Some(ci as u8);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Charge => p.0.iter().find(|(k, _)| k != "final" && k != "no_limit").map(|(k, _)| format!("{name} param {k}")),
            Action::Burst => p.0.iter().find(|(k, _)| k != "extend").map(|(k, _)| format!("{name} param {k}")),
            Action::Attack | Action::Skill => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        // a delay after a spin's hit, before the next charged attack
        if a == Action::Charge && s.current_state() == AnimState::Charge && st(s, ci).ca.error {
            return Some("use of delays between charge attack hits is invalid".into());
        }
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    /// The burst's Geo infusion, in the snapshot so nothing overrides it.
    fn snapshot(s: &Sim, ci: usize, info: &mut AttackInfo) {
        if burst_active(s, ci) && matches!(info.tag, AttackTag::Normal | AttackTag::Plunge | AttackTag::Extra) {
            info.element = Element::Geo;
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        // a char task for the hitlag of C4
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_NORMAL, arg: n as i32 }, g::ATTACK_HITMARKS[n]);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], g::ATTACK_HITMARKS[n], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if s.current_state() != AnimState::Charge {
            st_mut(s, ci).ca = ChargeState::default();
        }
        let ca = st(s, ci).ca;
        let mut fin = false;
        let mut hit_index = ca.counter;
        let windup = windup_frames(s, ci);
        if ca.ended || p.get("final").unwrap_or(0) != 0 {
            fin = true;
            hit_index = g::CHARGE_HIT_NUM;
        }
        let mut hitmark = g::CHARGE_HITMARKS[hit_index as usize];
        let (mut spin, mut trans) = (0, 0);
        if ca.start_f != 0 && fin {
            // spin (previous) to an explicit final
            trans = g::CHARGE_FINAL_DELAY[((ca.counter - 1) % g::CHARGE_HIT_NUM) as usize];
        } else if !fin {
            // this spin to an implicit final
            trans = g::CHARGE_FINAL_DELAY[hit_index as usize];
        }
        if fin {
            hitmark += trans;
        } else {
            spin = hitmark;
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let can_queue = windup + hitmark;
        let mut info = ActInfo::new(
            Frames::CharLive { id: FR_CHARGE, arg: 0 },
            windup + spin + trans + g::CHARGE_FRAMES[0],
            can_queue,
            AnimState::Charge,
        );
        info.on_removed = OnRemoved::Char(REMOVED_CHARGE);
        // gcsim's QueueAction calls, in its order
        let mut q: [(i32, u8, i32); 5] = [(0, 0, 0); 5];
        let mut n = 0;
        q[n] = (can_queue + 1, TASK_CA_ERROR, fin as i32);
        n += 1;
        // start a spin
        if ca.start_f == 0 && !fin {
            let src = s.f;
            st_mut(s, ci).ca.start_f = src;
            if p.get("no_limit").unwrap_or(0) == 0 {
                s.add_task(Task::Char { c: ci as u8, id: TASK_CA_LIMIT, arg: src }, 60 * 5);
            }
            q[n] = (windup, TASK_CA_STAMINA_START, src);
            n += 1;
        }
        q[n] = (windup + hitmark, TASK_CA_HIT, hit_index);
        n += 1;
        let final_start = windup + spin + trans;
        q[n] = (final_start, TASK_CA_RESET, 0);
        n += 1;
        if !fin {
            q[n] = (final_start + g::CHARGE_HITMARKS[g::CHARGE_HIT_NUM as usize], TASK_CA_HIT, g::CHARGE_HIT_NUM);
            n += 1;
        }
        // one runs a frame: by delay, then the order added (no two share
        // a delay)
        q[..n].sort_by_key(|x| x.0);
        info.queued = Some(q[0]);
        let stt = st_mut(s, ci);
        stt.ca_frames = ChargeFrames { windup, hitmark, spin, trans, fin, atkspd };
        stt.queue = q;
        stt.queue_len = n;
        stt.queue_next = 1;
        stt.ca.counter = (stt.ca.counter + 1) % g::CHARGE_HIT_NUM;
        info
    }

    fn frames_live(s: &Sim, ci: usize, _: u8, _: i16, next: Action) -> i32 {
        let fr = st(s, ci).ca_frames;
        let mut f = g::CHARGE_FRAMES[next as usize];
        if f == 0 {
            // spin or final to dash, jump or swap
            f = fr.hitmark;
        } else if !fr.fin && next == Action::Charge {
            // spin to spin or final
            f = fr.hitmark;
        } else {
            f += fr.spin + fr.trans;
        }
        fr.windup + atk_spd_adjust(f, fr.atkspd)
    }

    fn on_removed(s: &mut Sim, ci: usize, _: u8, next: AnimState) {
        if next != AnimState::Charge {
            st_mut(s, ci).ca = ChargeState::default();
        }
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        // the snapshot's DEF for the shield
        let def = total_def(&s.stats_pruning(ci));
        let hp = g::SHIELD_FLAT[lvl] + g::SHIELD_DEF[lvl] * def;
        let f = s.f;
        s.add_shield(Shield { owner: ci as u8, key: SHIELD, typ: ShieldType::NoelleSkill, target: -1, ele: Element::Geo, hp, expiry: f + 720, em_bonus: 0.0 });
        let stt = st_mut(s, ci);
        stt.shield_timer = f + 720;
        stt.a4_counter = 0;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL, arg: 0 }, g::SKILL_HITMARK);
        if s.chars[ci].cons >= 4 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_C4_TIMER, arg: 0 }, 720);
        }
        s.set_cd_with_delay(ci, Action::Skill, 24 * 60, 6);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        // the stat snapshot gcsim takes for its log (its Stats() drops
        // expired mods)
        s.stats_pruning(ci);
        let def = total_def(&s.select_stats(ci, [st::BASEDEF, st::DEFP, st::DEF], true));
        let mut mult = g::DEFCONV[s.talent(ci, 2)];
        if s.chars[ci].cons >= 6 {
            mult += 0.5;
        }
        st_mut(s, ci).burst_atk = mult * def;
        let mut dur = 900 + g::BURST_START;
        if s.chars[ci].cons >= 6 {
            // the C6 extension, 10 s unless `extend` says less
            let ext = p.get("extend").map_or(10, |e| e.clamp(0, 10)) as i32;
            dur += ext * 60;
        }
        let o = Owner::Char(ci as u8);
        s.add_mod(ci, Mod::owned(BURST_BUFF, ModKind::Stat, o, st::ATK, true, true, -1), dur);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST, arg: 0 }, 24);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST, arg: 1 }, 65);
        s.set_cd(ci, Action::Burst, 900);
        s.consume_energy(ci, 8);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
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
                let n = arg as usize;
                let lvl = s.talent(ci, 0);
                let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
                ai.abil = NORMAL_NAMES[n];
                ai.blunt = true;
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
                ai.def_halt = true;
                let mut b = 0;
                if burst_active(s, ci) {
                    b = 1;
                    if n == 2 {
                        // the burst's N3 has other hitlag
                        ai.hitlag_halt = 0.1 * 60.0;
                    }
                    ai.icd_tag = icd::TAG_NONE;
                }
                ai.poise = g::ATTACK_POISE_DMG[b][n];
                let hb = g::ATTACK_HITBOXES[b][n];
                let off = g::ATTACK_OFFSETS[n];
                ai.pattern = if n == 3 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
                s.queue_attack(ai, CB_HEAL | a4_cb(s, ci), 0, 0);
            }
            TASK_SKILL => {
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 50.0, g::SHIELD_DMG[lvl]);
                ai.abil = "Breastplate";
                ai.blunt = true;
                ai.poise = 100.0;
                ai.use_def = true;
                ai.def_halt = true;
                ai.pattern = s.circle_on_player(0.0, 0.0, 2.0);
                s.queue_attack(ai, CB_HEAL, 0, 0);
            }
            TASK_C4_TIMER => {
                if st(s, ci).shield_timer == s.f {
                    explode_shield(s, ci);
                }
            }
            TASK_BURST => {
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::BURST[lvl]);
                ai.abil = "Sweeping Time (Burst)";
                ai.blunt = true;
                ai.poise = 150.0;
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = 0.15 * 60.0;
                ai.def_halt = true;
                let r = if arg == 0 {
                    6.5
                } else {
                    ai.abil = "Sweeping Time (Skill)";
                    ai.mult = g::BURSTSKILL[lvl];
                    4.0
                };
                ai.pattern = s.circle_on_player(0.0, 0.0, r);
                s.queue_attack(ai, CB_HEAL, 0, 0);
            }
            TASK_CA_LIMIT => {
                if st(s, ci).ca.start_f == arg {
                    st_mut(s, ci).ca.ended = true;
                }
            }
            TASK_CA_STAMINA => stamina_tick(s, ci, arg),
            TASK_CA_QUEUE_HIT => charge_hit(s, ci, arg),
            TASK_CA_ERROR | TASK_CA_STAMINA_START | TASK_CA_HIT | TASK_CA_RESET => {
                run_queued(s, ci, id, arg);
                queue_next(s, ci);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        // skillHealCB: heals the party while Breastplate is up, at the
        // skill's chance (always in C1's burst)
        if cbs & CB_HEAL != 0 && s.shields.iter().any(|x| x.typ == ShieldType::NoelleSkill) {
            let lvl = s.talent(ci, 1);
            let prob = if s.chars[ci].cons >= 1 && burst_active(s, ci) { 1.0 } else { g::HEAL_CHANCE[lvl] };
            if s.rng.float64() < prob {
                let def = total_def(&ev.stats);
                let heal = g::SHIELD_HEAL[lvl] * def + g::SHIELD_HEAL_FLAT[lvl];
                s.heal_info(ci, None, false, heal, ev.stats[st::HEAL]);
            }
        }
        // A4: every 4 hits take 1 s off Breastplate's cooldown
        if cbs & CB_A4 != 0 {
            let stt = st_mut(s, ci);
            stt.a4_counter += 1;
            if stt.a4_counter == 4 {
                stt.a4_counter = 0;
                if s.cooldown(ci, Action::Skill) > 0 {
                    s.reduce_cd(ci, Action::Skill, 60);
                }
            }
        }
    }

    /// C2: charged attacks deal 15% more.
    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C2 && ev.info.tag == AttackTag::Extra {
            ev.stats[st::DMGP] += 0.15;
        }
    }

    /// The burst's ATK from her DEF.
    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == BURST_BUFF {
            out[st::ATK] += st(s, ci).burst_atk;
            return true;
        }
        false
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        // A1: the active character drained below 30% HP gets a shield of
        // 400% of her DEF for 20 s, once a minute
        if let Event::Drain { amount, .. } = *e {
            if amount <= 0.0 || s.mod_active(ci, A1_ICD) {
                return;
            }
            let active = s.player.active as usize;
            if s.chars[active].hp_ratio >= 0.3 {
                return;
            }
            s.add_status(ci, A1_ICD, 3600, false);
            let def = total_def(&s.stats_pruning(ci));
            let f = s.f;
            s.add_shield(Shield { owner: ci as u8, key: A1_SHIELD, typ: ShieldType::NoelleA1, target: active as i8, ele: Element::Cryo, hp: def * 4.0, expiry: f + 1200, em_bonus: 0.0 });
        }
    }

    /// C4: Breastplate broken by damage explodes.
    fn shield_damaged(s: &mut Sim, ci: usize, key: ModKey, _: f64, broken: bool) {
        if key == SHIELD && broken && s.chars[ci].cons >= 4 {
            explode_shield(s, ci);
        }
    }

    /// C4: Breastplate explodes as it expires.
    fn shield_expired(s: &mut Sim, ci: usize, key: ModKey) {
        if key == SHIELD && s.chars[ci].cons >= 4 {
            explode_shield(s, ci);
        }
    }

    /// C4: Breastplate replaced by a new one explodes.
    fn shield_overwritten(s: &mut Sim, ci: usize, key: ModKey) {
        if key == SHIELD && s.chars[ci].cons >= 4 {
            explode_shield(s, ci);
        }
    }
}

fn a4_cb(s: &Sim, ci: usize) -> u16 {
    if s.chars[ci].ascension >= 4 { CB_A4 } else { 0 }
}

/// gcsim's windupFrames: the spin's windup after the action before it.
fn windup_frames(s: &Sim, ci: usize) -> i32 {
    let mut windup = 56;
    match s.current_state() {
        AnimState::Normal => {
            if s.chars[ci].normal_counter == 3 {
                windup -= 11;
            }
        }
        AnimState::Charge => {
            windup -= 15;
            if st(s, ci).ca.start_f != 0 {
                windup = 0;
            }
        }
        AnimState::Plunge => windup -= 9,
        AnimState::Dash => windup -= 12,
        _ => {}
    }
    windup
}

/// A QueueAction task of the charged attack.
fn run_queued(s: &mut Sim, ci: usize, id: u8, arg: i32) {
    match id {
        TASK_CA_ERROR => st_mut(s, ci).ca.error = arg == 0,
        TASK_CA_STAMINA_START => stamina_task(s, ci, arg),
        // gcsim wraps the hit in Tasks.Add(..., 0) for its hitlag
        TASK_CA_HIT => s.add_task(Task::Char { c: ci as u8, id: TASK_CA_QUEUE_HIT, arg }, 0),
        TASK_CA_RESET => st_mut(s, ci).ca = ChargeState::default(),
        _ => unreachable!(),
    }
}

/// The charged attack's next QueueAction task into ActInfo.queued.
fn queue_next(s: &mut Sim, ci: usize) {
    let stt = st(s, ci);
    if stt.queue_next >= stt.queue_len {
        return;
    }
    let next = stt.queue[stt.queue_next];
    st_mut(s, ci).queue_next += 1;
    if let Some(a) = s.player.anim.as_mut() {
        a.queued = Some(next);
    }
}

/// gcsim's caStaminaTask: every 0.3 s while the spin lasts.
fn stamina_task(s: &mut Sim, ci: usize, src: i32) {
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CA_STAMINA, arg: src }, 18);
}

fn stamina_tick(s: &mut Sim, ci: usize, src: i32) {
    let ca = st(s, ci).ca;
    if src != ca.start_f || ca.ended {
        return;
    }
    if s.player.stam == 0.0 {
        st_mut(s, ci).ca.ended = true;
        return;
    }
    let r = (1.0 + s.stam_percent_mod(Action::Charge)).max(0.0);
    s.use_stam((40.0 * r) * 0.3);
    stamina_task(s, ci, src);
}

/// gcsim's queueChargeAttack.
fn charge_hit(s: &mut Sim, ci: usize, hit_index: i32) {
    let h = hit_index as usize;
    let b = burst_active(s, ci) as usize;
    let lvl = s.talent(ci, 0);
    let fin = hit_index == g::CHARGE_HIT_NUM;
    let mult = if fin { g::CHARGE_FINAL[lvl] } else { g::CHARGE[lvl] };
    let tag = if b != 0 { icd::TAG_NONE } else { icd::TAG_NORMAL_ATTACK };
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, tag, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult);
    ai.abil = if fin { "Charge Attack (Finisher)" } else { "Charge Attack" };
    ai.blunt = true;
    ai.poise = g::CHARGE_POISE_DMG[h];
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = g::CHARGE_HITLAG_HALT_FRAME[h] * 60.0;
    ai.def_halt = true;
    ai.pattern = s.circle_on_player(0.0, g::CHARGE_OFFSETS[h], g::CHARGE_HITBOXES[b][h]);
    s.queue_attack(ai, CB_HEAL | a4_cb(s, ci), 0, 0);
}

/// C4: Breastplate's 400% ATK Geo explosion.
fn explode_shield(s: &mut Sim, ci: usize) {
    st_mut(s, ci).shield_timer = 0;
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 50.0, 4.0);
    ai.abil = "Breastplate (C4)";
    ai.blunt = true;
    ai.poise = 100.0;
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = 0.15 * 60.0;
    ai.def_halt = true;
    ai.pattern = s.circle_on_player(0.0, 0.0, 4.0);
    s.queue_attack(ai, 0, 0, 0);
}
