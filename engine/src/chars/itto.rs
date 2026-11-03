//! Arataki Itto, from gcsim's internal/characters/itto: normal attacks
//! (whose combo survives a dash or his skill), the Arataki Kesagiri
//! charged slashes on Superlative Superstrength stacks, Ushi (a geo
//! construct, with the `travel` and `ushihit` params), the burst (with
//! `prestack`) and its leaving the field, A1, A4, every constellation, the
//! Xianyun plunges and the `will-c6-proc`, `slash-type`, `slash` and
//! `tags.strStack` conditions.
//! gcsim's charged attacks set `Segmented`, which only Gest of the Mighty
//! Wolf reads; it is not ported, so the flag is left out.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::itto as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// Superlative Superstrength (gcsim's Tags["strStack"]).
    stacks: i32,
    saved_normal: i32,
    /// The slash in progress (gcsim's SlashType, -1 for none).
    slash: i32,
    a1_stacks: i32,
    consumed: i32,
    burst_cast_f: i32,
    c2_count: i32,
    apply_c4: bool,
    c6_proc: bool,
    /// The skill's ushihit param, for its first hit.
    ushi_hit: i32,
    ushi_pending: bool,
    ushi_pos: [f64; 4],
    /// The last charged attack's ATK SPD, for its frames.
    charge_atkspd: f64,
}

const BURST: ModKey = ModKey::new(g::BURST_BUFF_KEY);
const BURST_ATKSPD: ModKey = ModKey::new(g::BURST_ATK_SPD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A1: ModKey = ModKey::new("itto-a1");
const C4: ModKey = ModKey::new("itto-c4");
const C6: ModKey = ModKey::new("itto-c6");
const USHI: ModKey = ModKey::new("itto-ushi");

const CB_USHI: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;

const TASK_USHI_DMG: u8 = 0;
const TASK_USHI_HIT: u8 = 1;
const TASK_C1: u8 = 2;
const TASK_C1_TIMER: u8 = 3;
const TASK_C2: u8 = 4;
// arg: the burst's cast frame
const TASK_C4: u8 = 5;

// arg: the slash + 4 × its windup
const FR_CHARGE: u8 = 0;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];
const SLASH_ABIL: [&str; 4] = ["Saichimonji Slash", "Arataki Kesagiri Combo Slash", "Arataki Kesagiri Combo Slash", "Arataki Kesagiri Final Slash"];

const SAICHI: i32 = g::SAICHI_SLASH;
const LEFT: i32 = g::LEFT_SLASH;
const RIGHT: i32 = g::RIGHT_SLASH;
const FINAL: i32 = g::FINAL_SLASH;

/// The airborne plunges (Xianyun's), outside and inside the burst (wider).
const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);
const PLUNGE_B: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMGB, g::HIGH_PLUNGE_POISE_DMGB, g::LOW_PLUNGE_RADIUS_B, g::HIGH_PLUNGE_RADIUS_B, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

/// gcsim's SlashType.Next.
fn next_slash(cur: i32, stacks: i32, c6: bool) -> i32 {
    match cur {
        LEFT | RIGHT if stacks == 1 && !c6 => FINAL,
        LEFT => RIGHT,
        RIGHT => LEFT,
        _ if stacks == 1 && !c6 => FINAL,
        _ if stacks >= 1 => LEFT,
        _ => SAICHI,
    }
}

/// gcsim's addStrStack: clamped to 0..5, a loss counted.
fn add_stacks(s: &mut Sim, ci: usize, inc: i32) {
    let st = st_mut(s, ci);
    let v = (st.stacks + inc).clamp(0, 5);
    if v < st.stacks {
        st.consumed += 1;
    }
    st.stacks = v;
}

/// gcsim's windupFrames: the frames a slash skips after what came before.
fn windup(s: &Sim, ci: usize, prev: i32, cur: i32) -> i32 {
    match s.current_state() {
        AnimState::Normal => match cur {
            SAICHI => match s.chars[ci].normal_counter - 1 {
                0 => 14,
                1 | 2 => 21,
                _ => 0,
            },
            LEFT | FINAL => 10,
            _ => 0,
        },
        AnimState::Charge => match (cur, prev) {
            (SAICHI, FINAL) => 14,
            (LEFT, SAICHI | FINAL) => 17,
            (LEFT, RIGHT) => 28,
            (FINAL, SAICHI | FINAL) => 17,
            (FINAL, LEFT | RIGHT) => 25,
            _ => 0,
        },
        AnimState::Skill => match cur {
            SAICHI => 14,
            LEFT | FINAL => 17,
            _ => 0,
        },
        AnimState::Plunge => match cur {
            SAICHI => 11,
            LEFT | FINAL => 10,
            _ => 0,
        },
        _ => 0,
    }
}

/// C4: when the burst's state ends, ATK and DEF +20% for the party.
fn c4(s: &mut Sim, ci: usize) {
    if !st(s, ci).apply_c4 {
        return;
    }
    st_mut(s, ci).apply_c4 = false;
    for x in 0..s.chars.len() {
        s.add_mod(x, Mod::owned(C4, ModKind::Stat, Owner::Char(ci as u8), st::NONE, false, true, -1), 600);
    }
}

fn reset_charge(s: &mut Sim, ci: usize) {
    let st = st_mut(s, ci);
    st.slash = -1;
    st.a1_stacks = 0;
    st.consumed = 0;
}

impl Character for Char {
    const KEY: &'static str = "aratakiitto";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { stacks: 0, saved_normal: 0, slash: -1, a1_stacks: 0, consumed: 0, burst_cast_f: -1, c2_count: 0, apply_c4: false, c6_proc: false, ushi_hit: 0, ushi_pending: false, ushi_pos: [0.0; 4], charge_atkspd: 0.0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // A1: ATK SPD for consecutive Arataki Kesagiri
        if s.chars[ci].ascension >= 1 {
            s.add_mod(ci, Mod::owned(A1, ModKind::Stat, o, st::ATKSPD, false, false, -1), -1);
        }
        // the burst ends on leaving the field (onExitField), and the slash
        // combo on anything but a charged attack (resetChargeState)
        s.subscribe(EventKind::Swap, o);
        s.subscribe(EventKind::ActionExec, o);
        if s.chars[ci].cons >= 2 {
            let n = s.chars.iter().filter(|c| c.element == Element::Geo).count().min(3);
            st_mut(s, ci).c2_count = n as i32;
        }
        if s.chars[ci].cons >= 6 {
            s.add_mod(ci, Mod::owned(C6, ModKind::Attack, o, st::NONE, false, false, -1), -1);
            let p = s.rng.float64() < 0.5;
            st_mut(s, ci).c6_proc = p;
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let only = |known: &[&str]| p.0.iter().find(|(k, _)| !known.iter().any(|n| k == *n)).map(|(k, _)| format!("{name} param {k}"));
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack | Action::Charge => only(&[]),
            Action::Skill => only(&["travel", "ushihit"]),
            Action::Burst => only(&["prestack"]),
            _ => Some(name.into()),
        }
    }

    /// gcsim's NextQueueItemIsValid: no N4 into Saichimonji Slash, no
    /// Saichimonji Slash into another; plunges only while airborne.
    fn invalid(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<String> {
        if let Some(e) = plunge::grounded(s, a) {
            return Some(e);
        }
        if a == Action::Charge {
            let st = st(s, ci);
            if s.player.last.action == Action::Attack && s.chars[s.player.active as usize].normal_counter == 0 && st.stacks == 0 {
                return Some("need to use attack right before charge".into());
            }
            if st.slash == SAICHI && next_slash(st.slash, st.stacks, st.c6_proc) == SAICHI {
                return Some("aratakiitto: Saichi Slash into Saichi Slash is invalid".into());
            }
        }
        crate::content::template_invalid(s, a)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        let st = st(s, ci);
        let int = |v: i32| Some(Ok(Num::Int(v as i64)));
        match (fields[0].as_str(), fields.get(1).map(|f| f.as_str())) {
            ("will-c6-proc", _) => int(st.c6_proc as i32),
            ("slash-type", Some("idle")) => int(-1),
            ("slash-type", Some("saichi")) => int(SAICHI),
            ("slash-type", Some("left")) => int(LEFT),
            ("slash-type", Some("right")) => int(RIGHT),
            ("slash-type", Some("final")) => int(FINAL),
            ("slash", Some("current")) => int(st.slash),
            ("slash", Some("next")) => int(next_slash(st.slash, st.stacks, st.c6_proc)),
            ("tags", Some(g::STR_STACK_KEY)) => int(st.stacks),
            _ => None,
        }
    }

    /// Charged attacks cost no stamina while he has stacks.
    fn action_stam(s: &Sim, ci: usize, a: Action) -> Option<f64> {
        (a == Action::Charge).then(|| if st(s, ci).stacks > 0 { 0.0 } else { 20.0 })
    }

    /// The burst's geo on normal, charged and plunging attacks, which
    /// nothing overrides.
    fn snapshot(s: &Sim, ci: usize, info: &mut AttackInfo) {
        if s.mod_active(ci, BURST) && matches!(info.tag, AttackTag::Normal | AttackTag::Plunge | AttackTag::Extra) {
            info.element = Element::Geo;
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == A1 {
            let n = st(s, ci).a1_stacks;
            if n == 0 || s.current_state() != AnimState::Charge {
                return false;
            }
            out[st::ATKSPD] += 0.10 * n as f64;
        } else if m.key == BURST_ATKSPD {
            if s.current_state() != AnimState::Normal {
                return false;
            }
            out[st::ATKSPD] += 0.10;
        } else if m.key == C4 {
            out[st::ATKP] += 0.2;
            out[st::DEFP] += 0.2;
        } else {
            return false;
        }
        true
    }

    /// C6: charged attacks CRIT DMG +70%.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::Extra {
            ev.stats[st::CD] += 0.7;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { prev, .. } if prev == ci && s.mod_active(ci, BURST) => {
                s.delete_mod(ci, BURST);
                s.delete_mod(ci, BURST_ATKSPD);
                c4(s, ci);
            }
            Event::ActionExec(a) if a != Action::Charge => reset_charge(s, ci),
            _ => {}
        }
    }

    /// A dash keeps the normal combo only after a normal attack or skill.
    fn template_action(s: &mut Sim, ci: usize, a: Action) {
        if a == Action::Dash && !matches!(s.player.last.action, Action::Attack | Action::Skill) {
            st_mut(s, ci).saved_normal = 0;
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if matches!(s.current_state(), AnimState::Dash | AnimState::Skill) {
            s.chars[ci].normal_counter = st(s, ci).saved_normal;
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.blunt = true;
        ai.poise = g::ATTACK_POISE_DMG[n];
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        let burst = s.mod_active(ci, BURST);
        let idx = burst as usize;
        let (off, hb) = (g::ATTACK_OFFSETS[idx][n], g::ATTACK_HITBOXES[idx][n]);
        ai.pattern = if n == 3 { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        // the stacks as if the strike hits (gcsim's assumption)
        match n {
            1 => add_stacks(s, ci, 1),
            3 => add_stacks(s, ci, 2),
            _ => {}
        }
        if burst && (n == 0 || n == 2) {
            add_stacks(s, ci, 1);
        }
        let state = (st(s, ci).stacks > 0) as usize;
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[state][n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        st_mut(s, ci).saved_normal = s.chars[ci].normal_counter;
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let (prev, stacks, c6) = {
            let st = st(s, ci);
            (st.slash, st.stacks, st.c6_proc)
        };
        let cur = next_slash(prev, stacks, c6);
        st_mut(s, ci).slash = cur;
        if matches!(prev, -1 | SAICHI | FINAL) {
            let st = st_mut(s, ci);
            st.a1_stacks = 0;
            st.consumed = 0;
        }
        let wind = windup(s, ci, prev, cur);
        let lvl = s.talent(ci, 0);
        let c = cur as usize;
        let mult = match cur {
            SAICHI => g::SAICHI_SLASH_[lvl],
            LEFT | RIGHT => g::AK_COMBO[lvl],
            _ => g::AK_FINAL[lvl],
        };
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult);
        ai.abil = SLASH_ABIL[c];
        ai.blunt = true;
        ai.hitlag_halt = 0.10 * 60.0;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        match cur {
            SAICHI => ai.poise = 120.0,
            LEFT | RIGHT => {
                ai.poise = 81.7;
                let halt = match st(s, ci).consumed {
                    0 => {
                        ai.poise = 143.4;
                        0.07
                    }
                    1 => 0.05,
                    _ => 0.03,
                };
                ai.hitlag_halt = halt * 60.0;
            }
            _ => ai.poise = 143.4,
        }
        // A4: Arataki Kesagiri DMG + 35% of DEF
        if cur != SAICHI && s.chars[ci].ascension >= 4 {
            ai.flat_dmg = total_def(&s.select_stats(ci, [st::BASEDEF, st::DEFP, st::DEF], false)) * 0.35;
        }
        let idx = s.mod_active(ci, BURST) as usize;
        let (off, hb) = (g::CHARGE_OFFSETS[idx][c], g::CHARGE_HITBOXES[idx][c]);
        ai.pattern = if cur == LEFT || cur == RIGHT { s.box_on_player(0.0, off, hb[0], hb[1]) } else { s.circle_on_player(0.0, off, hb[0]) };
        let h = g::CHARGE_HITMARKS[c];
        s.queue_attack(ai, 0, 0, h - wind);
        // C6: half the slashes keep their stack
        if !c6 {
            add_stacks(s, ci, -1);
        }
        // A1
        if s.chars[ci].ascension >= 1 {
            let st = st_mut(s, ci);
            match cur {
                SAICHI => st.a1_stacks = 0,
                LEFT | RIGHT => st.a1_stacks = (st.a1_stacks + 1).min(3),
                _ => {}
            }
        }
        let p = s.chars[ci].cons >= 6 && s.rng.float64() < 0.5;
        st_mut(s, ci).c6_proc = p;
        let atkspd = s.stat(ci, st::ATKSPD);
        st_mut(s, ci).charge_atkspd = atkspd;
        let mut info = ActInfo::new(Frames::CharLive { id: FR_CHARGE, arg: (cur + 4 * wind) as i16 }, g::CHARGE_FRAMES[c][0] - wind, h - wind, AnimState::Charge);
        info.on_removed = OnRemoved::Char(FR_CHARGE);
        info
    }

    fn frames_live(s: &Sim, ci: usize, _: u8, arg: i16, next: Action) -> i32 {
        let (cur, wind) = ((arg & 3) as i32, (arg >> 2) as i32);
        let mut f = g::CHARGE_FRAMES[cur as usize][next as usize];
        if next == Action::Charge {
            let st = st(s, ci);
            if next_slash(cur, st.stacks, st.c6_proc) == FINAL {
                match cur {
                    LEFT => f = 60,
                    RIGHT => f = 32,
                    _ => {}
                }
            }
        }
        atk_spd_adjust(f - wind, st(s, ci).charge_atkspd)
    }

    fn on_removed(s: &mut Sim, ci: usize, _: u8, next: AnimState) {
        if next != AnimState::Charge {
            reset_charge(s, ci);
        }
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        // the combo survives only a normal attack or skill before this
        if !matches!(s.player.last.action, Action::Attack | Action::Skill) {
            st_mut(s, ci).saved_normal = 0;
        }
        let travel = p.get("travel").unwrap_or(4) as i32;
        let ushihit = p.get("ushihit").unwrap_or(0).clamp(0, 3) as i32;
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL[lvl]);
        ai.abil = "Masatsu Zetsugi: Akaushi Burst!";
        ai.blunt = true;
        ai.poise = 250.0;
        ai.hitlag_halt = 0.02 * 60.0;
        ai.hitlag_factor = 0.01;
        ai.deployable = true;
        ai.pattern = s.circle_on_target(3.5);
        let ((x, y), (dx, dy)) = (s.enemy_pos, s.player_dir);
        {
            let st = st_mut(s, ci);
            st.ushi_hit = ushihit;
            st.ushi_pending = true;
            st.ushi_pos = [x, y, dx, dy];
        }
        // Ushi is assumed to hit for a stack
        s.add_task(Task::Char { c: ci as u8, id: TASK_USHI_DMG, arg: 0 }, g::SKILL_RELEASE + travel);
        s.queue_attack(ai, CB_USHI | CB_PARTICLE, g::SKILL_RELEASE, g::SKILL_RELEASE + travel);
        s.set_cd_with_delay(ci, Action::Skill, 600, g::SKILL_RELEASE);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        // N1 into burst: a stack before the DEF is read
        if p.get("prestack").unwrap_or(0) != 0 && s.current_state() == AnimState::Normal && s.chars[ci].normal_counter == 1 {
            add_stacks(s, ci, 1);
        }
        // gcsim's "fake" snapshot, which prunes the stat mods
        let mut snap = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 0.0, 0.0);
        snap.abil = "Royal Descent: Behold, Itto the Evil! (Stat Snapshot)";
        s.make_snapshot(&mut snap);
        let def = total_def(&s.select_stats(ci, [st::BASEDEF, st::DEFP, st::DEF], true));
        let atk = g::DEFCONV[s.talent(ci, 2)] * def;
        s.add_mod(ci, Mod::stat(BURST, st::ATK, atk, true, true, -1), g::BURST_DURATION);
        s.add_mod(ci, Mod::owned(BURST_ATKSPD, ModKind::Stat, Owner::Char(ci as u8), st::ATKSPD, false, true, -1), g::BURST_DURATION);
        let cons = s.chars[ci].cons;
        if cons >= 1 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_C1, arg: 0 }, 75);
        }
        if cons >= 2 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_C2, arg: 0 }, 9);
        }
        let f = s.f;
        st_mut(s, ci).burst_cast_f = f;
        if cons >= 4 {
            st_mut(s, ci).apply_c4 = true;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C4, arg: f }, g::BURST_DURATION);
        }
        s.set_cd(ci, Action::Burst, 1080);
        s.consume_energy(ci, 1);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let spec = if s.mod_active(ci, BURST) { &PLUNGE_B } else { &PLUNGE };
        plunge::standard(s, ci, true, p, spec, 0)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let spec = if s.mod_active(ci, BURST) { &PLUNGE_B } else { &PLUNGE };
        plunge::standard(s, ci, false, p, spec, 0)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_USHI_DMG | TASK_USHI_HIT | TASK_C1_TIMER => add_stacks(s, ci, 1),
            TASK_C1 => {
                add_stacks(s, ci, 2);
                for i in [60, 90, 120] {
                    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C1_TIMER, arg: 0 }, i);
                }
            }
            TASK_C2 => {
                let n = st(s, ci).c2_count;
                s.add_energy(ci, 6.0 * n as f64);
                s.reduce_cd(ci, Action::Burst, n * 90);
            }
            TASK_C4 => {
                if arg == st(s, ci).burst_cast_f {
                    c4(s, ci);
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_USHI != 0 && st(s, ci).ushi_pending {
            st_mut(s, ci).ushi_pending = false;
            let (f, pos, n) = (s.f, st(s, ci).ushi_pos, st(s, ci).ushi_hit);
            s.new_construct(Construct { owner: ci as u8, key: USHI, src: f, expiry: f + 360, pos: Some(pos), limited: true }, true);
            // ushihit: stacks from Ushi taking damage, at random, 2 s apart
            if n > 0 {
                let start = 6 - 2 * (n - 1);
                let mut next = 0;
                for i in 0..n {
                    let gain = s.rng.intn((start + 2 * i) * 60 - next) + next;
                    s.add_task(Task::Char { c: ci as u8, id: TASK_USHI_HIT, arg: 0 }, gain);
                    next = gain + 120;
                }
            }
        }
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 12, true);
            let n = if s.rng.float64() < 0.5 { 4.0 } else { 3.0 };
            s.queue_particle(n, Element::Geo, s.particle_delay[ci]);
        }
    }

    /// Ushi leaving grants a stack.
    fn construct_expired(s: &mut Sim, ci: usize, _: ModKey) {
        add_stacks(s, ci, 1);
    }
}
