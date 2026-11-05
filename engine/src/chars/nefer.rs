//! Nefer, from gcsim's internal/characters/nefer, all of it: normal
//! attacks, the charged attack's three routes (`hold=0` with no Phantasm
//! ready: the ordinary charged attack; Phantasm Performance when Shadow
//! Dance, a Phantasm charge and Verdant Dew are there; `hold>0`: Slither
//! for up to 150 frames, moving the player toward the enemy, spending
//! stamina and absorbing Seeds of Deceit, until Phantasm is ready or the
//! hold ends in the ordinary charged attack), the skill (Shadow Dance,
//! three Phantasm charges, A1's seed window), the burst (spending Veil of
//! Falsehood stacks for DMG), Lunar-Bloom for the fight and its base DMG
//! bonus, A1 (dendro cores become Seeds of Deceit while the window lasts,
//! with Ascendant Gleam), A4 (Verdant Dew faster in Slither after a
//! Lunar-Bloom), Veil of Falsehood and its EM, and C1 to C6. gcsim's
//! package has no plunge or aimed shot, and no fields (`.nefer.veilStacks`
//! and `.nefer.phantasmCharges` are only in its docs).
//!
//! Slither moves the player (gcsim's SetPlayerPos), so it needs every
//! member of the team checked for a moved player (`MOVES_WITH_PLAYER`):
//! `gse --check` names the others for a held charged attack, and the fight
//! stops with an error if one moves the player anyway.

use crate::content::{Character, Event};
use crate::gamedata::chars::nefer as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Clone, Copy, Default)]
struct Route {
    src: i32,
    slither_src: i32,
    release_start: i32,
    phantasm_start: i32,
    phantasm_end: i32,
}

pub struct State {
    ascendant: bool,
    max_veil: usize,
    /// Veil of Falsehood (gcsim's MultipleRefreshNoRemove): each stack's
    /// start frame and its own token, which its expiry task checks.
    veil: [Option<(i32, i32)>; 5],
    veil_token: i32,
    phantasm_charges: i32,
    commit_src: i32,
    veil_mult: f64,
    route: Route,
    /// The source frame of the latest charged attack and the one before
    /// (OnRemoved of the old one runs after the new one started).
    cur_src: i32,
    prev_src: i32,
    /// Each Phantasm Performance's hit area and the EM its flat damage
    /// took, by source frame, made when it started.
    perf: [(i32, Pattern, f64); 2],
    burst_bonus: f64,
}

const SHADOW_DANCE: ModKey = ModKey::new(g::SHADOW_DANCE_KEY);
const SLITHER: ModKey = ModKey::new(g::SLITHER_KEY);
const SEED_WINDOW: ModKey = ModKey::new(g::SEED_WINDOW_KEY);
const VEIL_EM: ModKey = ModKey::new(g::VEIL_EM_BUFF_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const P2_DEW_WINDOW: ModKey = ModKey::new(g::P2_DEW_WINDOW_KEY);
const BURST_BONUS: ModKey = ModKey::new(g::NEFER_BURST_BONUS_KEY);
const C4_SHRED: ModKey = ModKey::new(g::C4_RES_SHRED_KEY);

const CB_PARTICLE: u16 = 1 << 0;

// arg: the source frame
const TASK_START_PHANTASM: u8 = 0;
const TASK_SLITHER_START: u8 = 1;
const TASK_HELD_FINISH: u8 = 2;
const TASK_SLITHER_TICK: u8 = 3;
const TASK_PHANTASM_END: u8 = 4;
const TASK_CONSUME_DEW: u8 = 5;
// arg: the source frame * 8 + the hit's index
const TASK_PHANTASM_HIT: u8 = 6;
const TASK_ABSORB: u8 = 7;
// arg: the stack's token
const TASK_VEIL_EXPIRE: u8 = 8;

const FRAMES_TAP: u8 = 0;
const FRAMES_HELD: u8 = 1;
const FRAMES_BURST: u8 = 2;

const REMOVED_TAP: u8 = 0;
const REMOVED_HELD: u8 = 1;

/// The ids of its Verdant Dew rate mods (`Sim::dew_rate_mods`).
const DEW_P2: u8 = 0;
const DEW_C4: u8 = 1;

const BURST_HIT1: &str = "Sacred Vow: True Eye's Phantasm (Hit 1)";
const BURST_HIT2: &str = "Sacred Vow: True Eye's Phantasm (Hit 2)";
/// The Phantasm Performance's hits, in gcsim's order, and their delays.
const PERF_DELAYS: [i32; 6] = [g::PHANTASM_HIT1, g::PHANTASM_HIT2, g::PHANTASM_HIT3, g::PHANTASM_HIT4, g::PHANTASM_HIT5, g::PHANTASM_ANIMATION_LENGTH];

impl Character for Char {
    const KEY: &'static str = "nefer";
    const MOONSIGN: i32 = 1;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            ascendant: false,
            max_veil: 3,
            veil: [None; 5],
            veil_token: 0,
            phantasm_charges: 0,
            commit_src: 0,
            veil_mult: 0.0,
            route: Route::default(),
            cur_src: -1,
            prev_src: -1,
            perf: [(-1, Pattern::Unknown, 0.0); 2],
            burst_bonus: 0.0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // NewChar: two skill charges
        let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
        cd.available = 2;
        cd.max_extra = 1;
        let ascendant = s.chars.iter().map(|c| c.moonsign).sum::<i32>() >= 2;
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        let st = s.st_mut::<State>(ci);
        st.ascendant = ascendant;
        st.max_veil = if cons >= 2 { 5 } else { 3 };
        // swapResetInit
        s.subscribe(EventKind::Swap, o);
        // lunarbloomInit (and c6Init's elevation, in the same hook)
        s.enable_lunar_bloom();
        s.subscribe(EventKind::EnemyHit, o);
        // p1Init
        if asc >= 1 && ascendant {
            s.subscribe(EventKind::DendroCore, o);
        }
        // p2Init
        if asc >= 4 {
            s.dew_rate_mods.push((ci as u8, DEW_P2));
            s.subscribe(EventKind::Reaction, o);
        }
        // c4Init
        if cons >= 4 {
            s.dew_rate_mods.push((ci as u8, DEW_C4));
            s.tick_subs.push(ci as u8);
        }
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// ActionStam: none for a Phantasm Performance, else Slither's entry
    /// (60 frames of its drain).
    fn action_stam(s: &Sim, ci: usize, a: Action) -> Option<f64> {
        if a != Action::Charge {
            return None;
        }
        // slitherStamTickCost * slitherActivationFrames, a Go constant
        Some(if can_trigger_phantasm(s, ci) { 0.0 } else { 18.15 })
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = ["Normal 1", "Normal 2", "Normal 3", "Normal 4"][n];
        let hb = g::ATTACK_HITBOXES[n];
        ai.pattern = s.box_on_target(0.0, g::ATTACK_OFFSETS[n], hb[0], hb[1]);
        if n == 2 {
            s.queue_attack(ai, 0, 10, 10);
            s.queue_attack(ai, 0, 26, 26);
        } else {
            let h = g::ATTACK_HITMARKS[n];
            s.queue_attack(ai, 0, h, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], table[Action::Skill as usize], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        st.prev_src = st.cur_src;
        st.cur_src = f;
        let hold = p.get("hold").unwrap_or(0);
        if hold != 0 {
            return held_charge(s, ci, hold);
        }
        if can_trigger_phantasm(s, ci) {
            return tap_phantasm(s, ci);
        }
        let req = s.abil_stam_cost(ci, Action::Charge);
        s.restore_stam(req);
        clear_loop(s, ci);
        // basicChargeAttack
        clear_loop(s, ci);
        queue_basic_release(s, ci);
        let t = &g::CHARGE_FRAMES;
        let cqa = t[Action::Attack as usize].min(t[Action::Swap as usize]);
        ActInfo::new(Frames::Table(t), t[0], cqa, AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        let em = s.stat(ci, st::EM);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::SKILL[0][lvl]);
        ai.abil = "Senet Strategy: Dance of a Thousand Nights";
        ai.blunt = true;
        ai.flat_dmg = em * g::SKILL[1][lvl];
        ai.pattern = s.circle_toward_target(0.0, 1.0, 3.0);
        s.queue_attack(ai, CB_PARTICLE, g::SKILL_HITMARK, g::SKILL_HITMARK);
        s.add_status(ci, SHADOW_DANCE, 9 * 60, true);
        s.st_mut::<State>(ci).phantasm_charges = g::PHANTASM_CHARGES_PER_SKILL;
        // startSeedWindow
        if p1_active(s, ci) {
            s.add_status(ci, SEED_WINDOW, 15 * 60, true);
            s.convert_cores_to_seeds();
        }
        s.set_cd_with_delay(ci, Action::Skill, 9 * 60, 22);
        if s.chars[ci].cons >= 2 && s.st::<State>(ci).ascendant {
            add_veil(s, ci, 2);
        }
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let stacks = consume_veil(s, ci);
        let lvl = s.talent(ci, 2);
        let bonus = if stacks > 0 { stacks as f64 * g::BURST[4][lvl] } else { 0.0 };
        if bonus > 0.0 {
            s.st_mut::<State>(ci).burst_bonus = bonus;
            // covers both hits (104, 149)
            s.add_mod(ci, Mod::owned(BURST_BONUS, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, false, -1), 150);
        }
        let em = s.stat(ci, st::EM);
        let pattern = s.box_on_target(0.0, 0.0, 6.0, 10.0);
        let mut ai1 = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::BURST[0][lvl]);
        ai1.abil = BURST_HIT1;
        ai1.flat_dmg = em * g::BURST[1][lvl];
        ai1.pattern = pattern;
        let mut ai2 = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::BURST[2][lvl]);
        ai2.abil = BURST_HIT2;
        ai2.flat_dmg = em * g::BURST[3][lvl];
        ai2.pattern = pattern;
        s.queue_attack(ai1, 0, 104, 104);
        s.queue_attack(ai2, 0, 149, 149);
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 60);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::CharLive { id: FRAMES_BURST, arg: 0 }, t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn frames(id: u8, _: i16, next: Action) -> i32 {
        debug_assert_eq!(id, FRAMES_TAP);
        match next {
            Action::Burst | Action::Dash => g::BASIC_CHARGE_WINDUP + g::PHANTASM_HIT1,
            _ => g::BASIC_CHARGE_WINDUP + g::PHANTASM_ANIMATION_LENGTH,
        }
    }

    fn frames_live(s: &Sim, ci: usize, id: u8, arg: i16, next: Action) -> i32 {
        if id == FRAMES_BURST {
            if next == Action::Charge {
                return if can_trigger_phantasm(s, ci) { g::BURST_PHANTASM_CHARGE_CANCEL_FRAME } else { g::BURST_NORMAL_CHARGE_CANCEL_FRAME };
            }
            return g::BURST_FRAMES[next as usize];
        }
        let st = s.st::<State>(ci);
        let src = st.cur_src;
        let windup = g::BASIC_CHARGE_WINDUP;
        let dur = arg as i32;
        if phantasm_active(s, ci) {
            let off = st.route.phantasm_start - src;
            return match next {
                Action::Burst | Action::Dash => off + g::PHANTASM_HIT1,
                _ => off + g::PHANTASM_ANIMATION_LENGTH,
            };
        }
        if st.route.release_start > 0 {
            return st.route.release_start - src + g::CHARGE_FRAMES[next as usize];
        }
        match next {
            Action::Burst | Action::Dash => windup + g::SLITHER_MIN_CANCEL_FRAMES,
            Action::Charge => windup + dur + g::CHARGE_FRAMES[Action::Charge as usize],
            _ => windup + dur + g::CHARGE_FRAMES[0],
        }
    }

    fn on_removed(s: &mut Sim, ci: usize, id: u8, next: AnimState) {
        match id {
            REMOVED_TAP => {
                // the source frame of the action removed: a charged attack
                // starting now has already replaced the latest one
                let st = s.st::<State>(ci);
                let src = if st.cur_src == s.f { st.prev_src } else { st.cur_src };
                if st.route.src == src {
                    clear_loop(s, ci);
                }
            }
            _ => {
                if next != AnimState::Charge {
                    clear_loop(s, ci);
                }
            }
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_START_PHANTASM => start_phantasm(s, ci, arg),
            TASK_SLITHER_START => {
                if route_interrupted(s, ci, arg) {
                    return;
                }
                start_slither(s, ci, arg);
            }
            TASK_HELD_FINISH => {
                // finishHeldCharge
                if route_interrupted(s, ci, arg) || phantasm_active(s, ci) || s.st::<State>(ci).route.release_start > 0 {
                    return;
                }
                clear_slither(s, ci);
                // startBasicChargeRelease
                if s.st::<State>(ci).route.src != arg {
                    return;
                }
                let f = s.f;
                s.st_mut::<State>(ci).route.release_start = f;
                queue_basic_release(s, ci);
            }
            TASK_SLITHER_TICK => slither_tick(s, ci, arg),
            TASK_PHANTASM_END => {
                let st = s.st_mut::<State>(ci);
                if st.route.src != arg {
                    return;
                }
                st.route.phantasm_start = 0;
                st.route.phantasm_end = 0;
            }
            TASK_CONSUME_DEW => {
                if s.st::<State>(ci).route.src != arg {
                    return;
                }
                s.consume_dew(1);
                absorb(s, ci);
            }
            TASK_PHANTASM_HIT => phantasm_hit(s, ci, arg / 8, (arg % 8) as usize),
            TASK_ABSORB => absorb(s, ci),
            TASK_VEIL_EXPIRE => {
                let st = s.st_mut::<State>(ci);
                if let Some(v) = st.veil.iter_mut().find(|v| v.is_some_and(|x| x.1 == arg)) {
                    *v = None;
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE == 0 || s.mod_active(ci, PARTICLE_ICD) {
            return;
        }
        s.add_status(ci, PARTICLE_ICD, 12, false);
        let n = if s.rng.float64() < 0.66 { 3.0 } else { 2.0 };
        s.queue_particle(n, Element::Dendro, s.particle_delay[ci]);
    }

    /// The burst's DMG bonus from Veil of Falsehood, on its two hits.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == BURST_BONUS && ev.info.abil == BURST_HIT1 || ev.info.abil == BURST_HIT2 {
            ev.stats[st::DMGP] += s.st::<State>(ci).burst_bonus;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { prev, .. } => {
                if prev != ci {
                    return;
                }
                clear_loop(s, ci);
                s.delete_mod(ci, SHADOW_DANCE);
                s.st_mut::<State>(ci).phantasm_charges = 0;
            }
            Event::EnemyHit(ev) => {
                if ev.info.tag != AttackTag::DirectLunarBloom {
                    return;
                }
                // lunarbloomInit: base DMG on direct Lunar-Bloom
                let em = s.stat(ci, st::EM);
                s.hit_base_bonus += (em * 0.000175).min(0.14);
                // c6Init: its own direct Lunar-Bloom's elevation
                if s.chars[ci].cons >= 6 && s.st::<State>(ci).ascendant && ev.info.actor as usize == ci {
                    s.hit_elevation += 150;
                }
            }
            // p1Init: a core made while the window lasts becomes a seed
            Event::DendroCore { key } => {
                if s.mod_active(ci, SEED_WINDOW) {
                    s.replace_core_with_seed(key);
                }
            }
            // p2Init: Lunar-Bloom opens A4's window
            Event::Reaction(Reaction::LunarBloom, ev) => {
                if (ev.info.actor as usize) < s.chars.len() {
                    s.add_status(ci, P2_DEW_WINDOW, g::P2_DEW_WINDOW_DUR, true);
                }
            }
            _ => {}
        }
    }

    /// c4Init's OnTick: Dendro RES -20% on the enemy near the player while
    /// Nefer is on the field in Shadow Dance.
    fn on_tick(s: &mut Sim, ci: usize) {
        if s.player.active as usize != ci || !s.mod_active(ci, SHADOW_DANCE) || !s.enemy_alive {
            return;
        }
        let (px, py) = s.player_pos;
        let (ex, ey) = s.enemy_pos;
        let (dx, dy) = (px - ex, py - ey);
        if (dx * dx + dy * dy).sqrt() > g::C4_NEARBY_RADIUS as f64 + s.target_radius {
            return;
        }
        s.enemy_add_resist(C4_SHRED, Element::Dendro, -0.2, g::C4_RES_SHRED_LINGER);
    }
}

/// Its Verdant Dew rate mods' amounts: A4 by EM in Slither after a
/// Lunar-Bloom, C4 while on the field in Shadow Dance.
pub fn dew_rate(s: &Sim, ci: usize, id: u8) -> f64 {
    if id == DEW_P2 {
        if !s.mod_active(ci, SHADOW_DANCE) || !s.mod_active(ci, SLITHER) || !s.mod_active(ci, P2_DEW_WINDOW) {
            return 0.0;
        }
        let steps = ((s.stat(ci, st::EM) - 500.0).max(0.0) / 100.0).floor();
        return (steps * 0.1).min(0.5);
    }
    if s.player.active as usize != ci || !s.mod_active(ci, SHADOW_DANCE) {
        return 0.0;
    }
    0.25
}

/// Whether a charged attack with these params can enter Slither and move
/// the player (`check`).
pub fn may_slither(hold: Option<i64>) -> bool {
    hold != Some(0)
}

fn p1_active(s: &Sim, ci: usize) -> bool {
    s.chars[ci].ascension >= 1 && s.st::<State>(ci).ascendant
}

fn can_trigger_phantasm(s: &Sim, ci: usize) -> bool {
    s.mod_active(ci, SHADOW_DANCE) && s.st::<State>(ci).phantasm_charges > 0 && s.dew() > 0
}

fn phantasm_active(s: &Sim, ci: usize) -> bool {
    s.st::<State>(ci).route.phantasm_end > s.f
}

fn route_active(s: &Sim, ci: usize, src: i32) -> bool {
    s.st::<State>(ci).route.src == src && s.player.active as usize == ci && s.current_state() == AnimState::Charge
}

fn route_interrupted(s: &mut Sim, ci: usize, src: i32) -> bool {
    if route_active(s, ci, src) {
        return false;
    }
    clear_loop(s, ci);
    true
}

fn clear_slither(s: &mut Sim, ci: usize) {
    s.delete_mod(ci, SLITHER);
    s.st_mut::<State>(ci).route.slither_src = 0;
}

fn clear_loop(s: &mut Sim, ci: usize) {
    clear_slither(s, ci);
    s.st_mut::<State>(ci).route = Route::default();
}

fn tap_phantasm(s: &mut Sim, ci: usize) -> ActInfo {
    clear_loop(s, ci);
    let src = s.f;
    s.st_mut::<State>(ci).route.src = src;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_START_PHANTASM, arg: src }, g::BASIC_CHARGE_WINDUP);
    let mut info = ActInfo::new(Frames::Char { id: FRAMES_TAP, arg: 0 }, g::BASIC_CHARGE_WINDUP + g::PHANTASM_ANIMATION_LENGTH, 1, AnimState::Charge);
    info.on_removed = OnRemoved::Char(REMOVED_TAP);
    info
}

fn held_charge(s: &mut Sim, ci: usize, hold: i64) -> ActInfo {
    // with Phantasm ready, holding is tapping
    if can_trigger_phantasm(s, ci) {
        return tap_phantasm(s, ci);
    }
    let req = s.abil_stam_cost(ci, Action::Charge);
    s.restore_stam(req);
    clear_loop(s, ci);
    let src = s.f;
    let windup = g::BASIC_CHARGE_WINDUP;
    let dur = hold.clamp(1, g::SLITHER_MAX_DURATION as i64) as i32;
    s.st_mut::<State>(ci).route.src = src;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SLITHER_START, arg: src }, windup);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_HELD_FINISH, arg: src }, windup + dur);
    let mut info = ActInfo::new(Frames::CharLive { id: FRAMES_HELD, arg: dur as i16 }, windup + dur + g::CHARGE_FRAMES[0], 1, AnimState::Charge);
    info.on_removed = OnRemoved::Char(REMOVED_HELD);
    info
}

fn start_slither(s: &mut Sim, ci: usize, src: i32) {
    if !route_active(s, ci, src) || s.mod_active(ci, SLITHER) {
        return;
    }
    s.add_status(ci, SLITHER, -1, false);
    s.st_mut::<State>(ci).route.slither_src = src;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SLITHER_TICK, arg: src }, 0);
}

fn slither_tick(s: &mut Sim, ci: usize, src: i32) {
    // slitherLoopInterrupted
    if route_interrupted(s, ci, src) || s.st::<State>(ci).route.slither_src != src || !s.mod_active(ci, SLITHER) {
        return;
    }
    if can_trigger_phantasm(s, ci) {
        // triggerPhantasmFromLoop
        if !route_active(s, ci, src) || !can_trigger_phantasm(s, ci) {
            return;
        }
        clear_slither(s, ci);
        start_phantasm(s, ci, src);
        return;
    }
    if phantasm_active(s, ci) {
        clear_slither(s, ci);
        return;
    }
    let mut req = g::SLITHER_STAM_TICK_COST * (1.0 + s.stam_percent_mod(Action::Charge));
    if req < 0.0 {
        req = 0.0;
    }
    if s.player.stam < req {
        clear_slither(s, ci);
        return;
    }
    s.slither_step(g::SLITHER_MOVE_DISTANCE);
    s.use_stam(req);
    absorb(s, ci);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SLITHER_TICK, arg: src }, g::SLITHER_MOVE_INTERVAL.max(g::SLITHER_STAM_INTERVAL));
}

fn start_phantasm(s: &mut Sim, ci: usize, src: i32) {
    if s.st::<State>(ci).route.src != src || !can_trigger_phantasm(s, ci) {
        return;
    }
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    st.phantasm_charges -= 1;
    st.route.phantasm_start = f;
    st.route.phantasm_end = f + g::PHANTASM_ANIMATION_LENGTH;
    // queuePhantasmPerformance
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CONSUME_DEW, arg: src }, g::PHANTASM_CONSUME_DEW_FRAME);
    let em = s.stat(ci, st::EM);
    let pattern = s.circle_on_player(0.0, 0.0, 5.0);
    let st = s.st_mut::<State>(ci);
    let slot = if st.perf[0].0 <= st.perf[1].0 { 0 } else { 1 };
    st.perf[slot] = (src, pattern, em);
    let n = if s.chars[ci].cons >= 6 { 6 } else { 5 };
    for (i, &d) in PERF_DELAYS.iter().enumerate().take(n) {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_PHANTASM_HIT, arg: src * 8 + i as i32 }, d);
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_PHANTASM_END, arg: src }, g::PHANTASM_ANIMATION_LENGTH);
}

/// One of the Phantasm Performance's hits: the first commits it (and the
/// Veil of Falsehood multiplier), the others land only while it stays the
/// last committed.
fn phantasm_hit(s: &mut Sim, ci: usize, src: i32, i: usize) {
    if i == 0 {
        if !route_active(s, ci, src) {
            return;
        }
        let bonus = veil_bonus(s, ci);
        let st = s.st_mut::<State>(ci);
        st.commit_src = src;
        st.veil_mult = 1.0 + bonus;
    } else if s.st::<State>(ci).commit_src != src {
        return;
    }
    let st = s.st::<State>(ci);
    let &(_, pattern, em) = st.perf.iter().find(|p| p.0 == src).expect("a Phantasm Performance's area");
    let mult = st.veil_mult;
    let lvl = s.talent(ci, 1);
    let cons = s.chars[ci].cons;
    let c1 = if cons >= 1 { g::C1_BASE_SHADE_SCALE_BONUS } else { 0.0 };
    let nefer = |m: f64, abil: &'static str| {
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, m);
        ai.abil = abil;
        ai
    };
    let lunar = |m: f64, abil: &'static str| {
        let mut ai = AttackInfo::new(ci, AttackTag::DirectLunarBloom, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 0.0, m);
        ai.abil = abil;
        ai.use_em = true;
        ai.ignore_def = 1.0;
        ai
    };
    let mut ai = match i {
        0 => {
            let mut ai = nefer(g::PHANTASM[0][lvl], "Phantasm Performance (Nefer 1)");
            ai.flat_dmg = em * g::PHANTASM[1][lvl];
            ai
        }
        1 => lunar(g::PHANTASM[4][lvl] + c1, "Phantasm Performance (Shade 1)"),
        2 => lunar(g::PHANTASM[5][lvl] + c1, "Phantasm Performance (Shade 2)"),
        3 if cons >= 6 => lunar(g::C6_PHANTASM_HIT2_EM, "Phantasm Performance (Nefer 2)"),
        3 => {
            let mut ai = nefer(g::PHANTASM[2][lvl], "Phantasm Performance (Nefer 2)");
            ai.flat_dmg = em * g::PHANTASM[3][lvl];
            ai
        }
        4 => lunar(g::PHANTASM[6][lvl] + c1, "Phantasm Performance (Shade 3)"),
        _ => lunar(g::C6_PHANTASM_END_EM, "Phantasm Performance (C6 End Hit)"),
    };
    ai.mult *= mult;
    ai.flat_dmg *= mult;
    ai.pattern = pattern;
    let ev = s.snapshot_event(ai, 0);
    s.queue_attack_with_snap(ev, 0, 0);
}

fn queue_basic_release(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::CHARGE[lvl]);
    ai.abil = "Charge Attack";
    ai.pattern = s.box_on_target(0.0, -2.0, 3.0, 9.0);
    s.queue_attack(ai, 0, g::BASIC_CHARGE_HITMARK, g::BASIC_CHARGE_HITMARK);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_ABSORB, arg: 0 }, g::BASIC_CHARGE_HITMARK);
}

/// absorbSeedsOfDeceit: each seed near the player is a Veil stack.
fn absorb(s: &mut Sim, ci: usize) {
    let n = s.absorb_seeds(g::SEED_ABSORB_RADIUS as f64);
    if n > 0 {
        add_veil(s, ci, n);
    }
}

fn veil_count(s: &Sim, ci: usize) -> i32 {
    let st = s.st::<State>(ci);
    st.veil[..st.max_veil].iter().filter(|v| v.is_some()).count() as i32
}

fn veil_bonus(s: &Sim, ci: usize) -> f64 {
    let n = veil_count(s, ci);
    if n <= 0 { 0.0 } else { n as f64 * 0.08 }
}

/// consumeVeilStacks: the stacks, and a new tracker.
fn consume_veil(s: &mut Sim, ci: usize) -> i32 {
    let n = veil_count(s, ci);
    s.st_mut::<State>(ci).veil = [None; 5];
    n
}

fn add_veil(s: &mut Sim, ci: usize, count: i32) {
    if count <= 0 {
        return;
    }
    let mut prev = veil_count(s, ci);
    let dur = g::VEIL_BASE_DURATION + if s.chars[ci].cons >= 2 { g::C2_VEIL_DURATION_BONUS } else { 0 };
    for _ in 0..count {
        // MultipleRefreshNoRemove.Add: an empty stack, or the oldest
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        let mut idx = 0;
        for i in 0..st.max_veil {
            match st.veil[i] {
                None => {
                    idx = i;
                    break;
                }
                Some((src, _)) => {
                    if src < st.veil[idx].map_or(i32::MAX, |x| x.0) {
                        idx = i;
                    }
                }
            }
        }
        st.veil_token += 1;
        let token = st.veil_token;
        st.veil[idx] = Some((f, token));
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_VEIL_EXPIRE, arg: token }, dur);
        let next = veil_count(s, ci);
        // applyVeilThresholdBuff
        let five = s.chars[ci].cons >= 2 && next >= 5 && (prev < 5 || prev == 5);
        let three = next >= 3 && (prev < 3 || prev == 3);
        let amount = if five { 200.0 } else if three { 100.0 } else { 0.0 };
        if amount > 0.0 {
            s.add_stat_mod(ci, VEIL_EM, st::EM, amount, 8 * 60, true);
        }
        prev = next;
    }
}
