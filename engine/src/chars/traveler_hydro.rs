//! The hydro Traveler (luminehydro and aetherhydro), from gcsim's
//! internal/characters/traveler/common/hydro: normal and charged attacks,
//! Aquacrest Saber (press, the short holds and the hold with its Dewdrops,
//! Suffusion, Torrent Surge and Spiritbreath Thorn), Rising Waters, A1's
//! Sourcewater Droplets (picked up by the dash's pickup_droplets), A4 and
//! every constellation. gcsim gives it no plunges.
//!
//! gcsim adds each droplet to the gadgets twice (sourcewaterdroplet.New
//! adds it, and A1 adds it again under a new key): it ticks twice a frame,
//! so it lasts 439 frames, not 878; it counts twice toward the limit of
//! 12, so 6 are kept; and `.gadgets.sourcewaterdroplet` counts it twice.
//! Neuvillette and Sigewinne take any droplet, and the Traveler takes
//! theirs: a team with one of them is an error (`profile`).
//!
//! One module holds both: `luminehydro` and `aetherhydro` are `Char<1>`
//! and `Char<0>` (gcsim's gender).

use super::traveler_common;
use crate::content::{Character, Event};
use crate::gamedata::chars::traveler_common_hydro as g;
use crate::sim::*;
use crate::types::*;

pub struct Char<const G: u8>;

pub struct State<const G: u8>(Inner);

pub struct Inner {
    gender: usize,
    /// gcsim's a4Bonus: Torrent Surge's flat damage from Suffusion, taken
    /// by the next skill's Torrent Surge as it is cast.
    a4_bonus: f64,
    /// The current skill's A1 droplets (its makeA1CB's count).
    a1_count: i32,
    story: traveler_common::Story,
}

fn st(s: &Sim, ci: usize) -> &Inner {
    match &s.chars[ci].state {
        CharState::LumineHydro(x) => &x.0,
        CharState::AetherHydro(x) => &x.0,
        _ => unreachable!(),
    }
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut Inner {
    match &mut s.chars[ci].state {
        CharState::LumineHydro(x) => &mut x.0,
        CharState::AetherHydro(x) => &mut x.0,
        _ => unreachable!(),
    }
}

const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const THORN_ICD: ModKey = ModKey::new(g::SPIRITBREATH_THORN_ICD_KEY);
const LOSING_HP_ICD: ModKey = ModKey::new(g::SKILL_LOSING_HPICD_KEY);
const A1_ICD: ModKey = ModKey::new(g::A1_ICD_KEY);
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const C4_SHIELD: ModKey = ModKey::new("travelerhydro-c4");

const CB_PARTICLE: u16 = 1 << 0;
const CB_A1: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;

/// A Dewdrop's release (its char queue task).
const TASK_DEWDROP: u8 = 0;

/// The skill's OnRemoved: C4's Aegis ends.
const REMOVED_SKILL: u8 = 0;

/// The droplet gadget.
const DROPLET: u8 = 0;
/// sourcewaterdroplet's 878 frames, ticked twice a frame.
const DROPLET_DURATION: i32 = 439;
/// The limit of 12 entries, two for each droplet.
const DROPLET_LIMIT: usize = 6;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];
const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];

impl<const G: u8> Character for Char<G> {
    const KEY: &'static str = if G == 1 { "luminehydro" } else { "aetherhydro" };
    type State = State<G>;

    fn new_state(cs: &CharSetup) -> State<G> {
        State(Inner { gender: G as usize, a4_bonus: 0.0, a1_count: 0, story: traveler_common::Story::new(cs) })
    }

    fn init(s: &mut Sim, ci: usize) {
        traveler_common::story_buffs(s, ci);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Skill => p.0.iter().find(|(k, _)| !matches!(k.as_str(), "hold" | "hold_ticks" | "travel")).map(|(k, _)| format!("{name} param {k}")),
            Action::Attack | Action::Charge | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[sex][n] * 60.0;
        ai.def_halt = true;
        let off = g::ATTACK_OFFSETS[sex][n];
        let hb = g::ATTACK_HITBOXES[sex][n];
        ai.pattern = if (sex == 0 && (n == 0 || n == 2)) || (sex == 1 && n == 1) {
            s.box_on_player(0.0, off, hb[0], hb[1])
        } else {
            s.fan_on_player(0.0, off, hb[0], g::ATTACK_FAN_ANGLES[sex][n])
        };
        let h = g::ATTACK_HITMARKS[sex][n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[sex][n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, 0.0);
        let marks = g::CHARGE_HITMARKS[sex];
        for (i, mult) in g::CHARGE[sex].iter().enumerate() {
            ai.mult = mult[lvl];
            ai.abil = CHARGE_NAMES[i];
            ai.pattern = s.circle_on_player(0.0, 0.0, 2.2);
            s.queue_attack(ai, 0, marks[i], marks[i]);
        }
        let t = &g::CHARGE_FRAMES[sex];
        ActInfo::new(Frames::Table(t), t[0], marks[marks.len() - 1], AnimState::Charge)
    }

    /// Aquacrest Saber: hold=1 is the longest hold (22 Dewdrops),
    /// hold_ticks=n one of n (1 the short hold), neither the press.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        let mut hold = p.get("hold") == Some(1);
        let mut ticks = p.get("hold_ticks").unwrap_or(0);
        if hold {
            ticks = 22;
        } else if ticks > 0 {
            hold = true;
        }
        let ticks = ticks.min(22) as i32;
        let travel = p.get("travel").map_or(6, |t| t as i32);
        // makeA1CB's count, new for each use
        st_mut(s, ci).a1_count = 0;
        if !hold {
            return skill_press(s, ci, g::SKILL_PRESS_HITMARKS[sex], g::SKILL_PRESS_SPIRIT_THORN_HITMARK, g::SKILL_PRESS_CD_START, &g::SKILL_PRESS_FRAMES[sex], g::SKILL_PRESS_CAN_QUEUE_AFTER[sex]);
        }
        if ticks == 0 {
            return skill_press(s, ci, g::SKILL_SHORT_HOLD0_TICKS_TORRENT_SURGE_HITMARK, g::SKILL_SHORT_HOLD0_TICKS_SPIRITBREATH_THORN_HITMARK, g::SKILL_SHORT_HOLD0_TICKS_CD_START, &g::SKILL_SHORT_HOLD0_TICKS_FRAMES[sex], g::SKILL_SHORT_HOLD0_TICKS_CAN_QUEUE_AFTER[sex]);
        }
        // skillShortHold (one Dewdrop) and skillHold
        if s.chars[ci].cons >= 4 {
            c4(s, ci);
        }
        let extend = 15 * (ticks - 1);
        let mut i = 0;
        while i <= extend {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_DEWDROP, arg: 0 }, g::SKILL_SHORT_HOLD_FIRST_DEWDROP_RELEASE + i + travel);
            i += 15;
        }
        torrent_surge(s, ci, g::SKILL_SHORT_HOLD_TORRENT_SURGE_HITMARK + extend, g::SKILL_SHORT_HOLD_SPIRITBREATH_THORN_HITMARK + extend);
        s.set_cd_with_delay(ci, Action::Skill, 10 * 60, g::SKILL_SHORT_HOLD_CD_START + extend);
        let t = &g::SKILL_SHORT_HOLD_FRAMES[sex];
        let frames = if extend == 0 { Frames::Table(t) } else { Frames::Char { id: sex as u8, arg: extend as i16 } };
        let mut info = ActInfo::new(frames, t[0] + extend, g::SKILL_SHORT_HOLD_CAN_QUEUE_AFTER[sex] + extend, AnimState::Skill);
        info.on_removed = OnRemoved::Char(REMOVED_SKILL);
        info
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        g::SKILL_SHORT_HOLD_FRAMES[id as usize][next as usize] + arg as i32
    }

    fn on_removed(s: &mut Sim, ci: usize, id: u8, _: AnimState) {
        // c4Remove
        if id == REMOVED_SKILL && s.chars[ci].cons >= 4 && s.shields.iter().any(|x| x.typ == ShieldType::TravelerHydroC4) {
            let f = s.f;
            s.set_shield_expiry(ShieldType::TravelerHydroC4, f + 1);
        }
    }

    /// Rising Waters: the bubble's ticks along the player's facing, every
    /// 0.5 s, snapshot at the cast.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let sex = st(s, ci).gender;
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_TRAVELER_BURST, Element::Hydro, 25.0, g::BURST_DOT[lvl]);
        ai.abil = "Rising Waters";
        let ev = s.snapshot_event(ai, 0);
        let (ticks, speed) = if s.chars[ci].cons >= 2 { (14, 1.05) } else { (8, 1.5) };
        let first = g::BURST_FIRST_HITMARK[sex];
        let (px, py) = s.player_pos;
        let dir = s.player_dir;
        for i in 0..ticks {
            let (x, y) = offset_point((px + 0.5, py + 0.5), (0.0, speed * i as f64), dir);
            let mut e = ev;
            e.info.pattern = Pattern::Circle { x, y, r: 0.15 };
            s.queue_attack_with_snap(e, 0, first + 30 * i);
        }
        s.set_cd(ci, Action::Burst, 20 * 60);
        s.consume_energy(ci, g::CONSUME_ENERGY_FRAME[sex]);
        let t = &g::BURST_FRAMES[sex];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    /// The dash picks up pickup_droplets droplets first (A1), then is the
    /// template's.
    fn own_dash(s: &mut Sim, ci: usize, p: &Params) -> Option<ActInfo> {
        if s.chars[ci].ascension >= 1 {
            let count = p.get("pickup_droplets").unwrap_or(0).max(0);
            a1_pick_up(s, ci, count);
        }
        None
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_DEWDROP => {
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArtHold, icd::TAG_TRAVELER_DEWDROP, icd::GROUP_TRAVELER_DEWDROP, Element::Hydro, 25.0, g::SKILL_DEWDROP[lvl]);
                ai.abil = "Dewdrop (Hold)";
                skill_losing_hp(s, ci, &mut ai);
                ai.pattern = s.box_on_target(0.0, -0.4, 0.3, 1.3);
                let mut cbs = 0;
                if s.chars[ci].ascension >= 1 {
                    cbs |= CB_A1;
                }
                if s.chars[ci].cons >= 4 {
                    cbs |= CB_C4;
                }
                s.queue_attack(ai, cbs, 0, 1);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, (0.3 * 60.0) as i32, true);
            let count = if s.rng.float64() < 0.33 { 4.0 } else { 3.0 };
            s.queue_particle(count, Element::Hydro, s.particle_delay[ci]);
        }
        if cbs & CB_A1 != 0 && st(s, ci).a1_count < 4 && !s.mod_active(ci, A1_ICD) {
            st_mut(s, ci).a1_count += 1;
            new_droplet(s, ci);
            s.add_status(ci, A1_ICD, 60, true);
        }
        if cbs & CB_C4 != 0 && !s.mod_active(ci, C4_ICD) {
            c4(s, ci);
            s.add_status(ci, C4_ICD, 2 * 60, true);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        traveler_common::story_amount(&st(s, ci).story, m, out)
    }

    fn event(_: &mut Sim, _: usize, _: &Event) {}
}

/// Whether a gadget is one of a hydro Traveler's droplets.
pub fn is_droplet(s: &Sim, gd: &Gadget) -> bool {
    gd.owner != CORE && gd.id == DROPLET && s.chars[gd.owner as usize].kind.traveler_hydro()
}

/// skillPress, for the press and the short hold of no Dewdrops.
fn skill_press(s: &mut Sim, ci: usize, hitmark: i32, spirit: i32, cd_start: i32, t: &'static FrameTable, can_queue: i32) -> ActInfo {
    torrent_surge(s, ci, hitmark, spirit);
    s.set_cd_with_delay(ci, Action::Skill, 10 * 60, cd_start);
    if s.chars[ci].cons >= 4 {
        c4(s, ci);
    }
    let mut info = ActInfo::new(Frames::Table(t), t[0], can_queue, AnimState::Skill);
    info.on_removed = OnRemoved::Char(REMOVED_SKILL);
    info
}

/// Torrent Surge (with A4's bonus so far), and Spiritbreath Thorn once
/// every 9 s.
fn torrent_surge(s: &mut Sim, ci: usize, hitmark: i32, spirit: i32) {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::SKILL_PRESS[lvl]);
    ai.abil = "Torrent Surge";
    if s.chars[ci].ascension >= 4 {
        ai.flat_dmg += st(s, ci).a4_bonus;
        st_mut(s, ci).a4_bonus = 0.0;
    }
    let hitbox = s.box_on_player(0.0, 0.0, 1.2, 15.0);
    ai.pattern = hitbox;
    s.queue_attack(ai, CB_PARTICLE, hitmark, hitmark);
    if !s.mod_active(ci, THORN_ICD) {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 0.0, g::SPIRITBREATH_THORN[lvl]);
        ai.abil = "Spiritbreath Thorn";
        ai.def_halt = true;
        ai.pattern = hitbox;
        s.queue_attack(ai, 0, spirit, spirit);
        s.add_status(ci, THORN_ICD, 9 * 60, true);
    }
}

/// Suffusion: above half HP, once every 0.9 s, the Dewdrop gains flat
/// damage from max HP for 4% of current HP; A4 keeps 45% of it.
fn skill_losing_hp(s: &mut Sim, ci: usize, ai: &mut AttackInfo) {
    if s.mod_active(ci, LOSING_HP_ICD) || s.chars[ci].hp_ratio <= 0.5 {
        return;
    }
    ai.flat_dmg = g::DEWDROP_BONUS[s.talent(ci, 1)] * s.max_hp(ci);
    let drain = 0.04 * s.current_hp(ci);
    s.drain_info(ci, drain, false, "Suffusion");
    if s.chars[ci].ascension >= 4 {
        let stv = st_mut(s, ci);
        stv.a4_bonus = (stv.a4_bonus + drain * 0.45).min(5000.0);
    }
    s.add_status(ci, LOSING_HP_ICD, (0.9 * 60.0) as i32, true);
}

/// C4's Aquacrest Aegis: 10% of max HP, renewed in place when it is up.
fn c4(s: &mut Sim, ci: usize) {
    let hp = 0.1 * s.max_hp(ci);
    if let Some(sh) = s.shields.iter_mut().find(|x| x.typ == ShieldType::TravelerHydroC4) {
        sh.hp = hp;
        return;
    }
    let f = s.f;
    s.add_shield(Shield { owner: ci as u8, key: C4_SHIELD, typ: ShieldType::TravelerHydroC4, target: ci as i8, ele: Element::Hydro, hp, expiry: f + 15 * 60, em_bonus: 0.0 });
}

/// newDroplet: 3.5 ahead of the player, at a random point 0.3 to 3 from
/// there; with 6 up, the oldest goes.
fn new_droplet(s: &mut Sim, ci: usize) {
    let (cx, cy) = s.point_from_player(0.0, 3.5);
    let (x, y) = s.random_point(cx, cy, 0.3, 3.0);
    let mut count = 0;
    let mut oldest: Option<usize> = None;
    for (k, gd) in s.gadgets.iter().enumerate() {
        if is_droplet(s, gd) {
            count += 1;
            if oldest.is_none_or(|o| gd.src < s.gadgets[o].src) {
                oldest = Some(k);
            }
        }
    }
    if count == DROPLET_LIMIT {
        s.gadgets.remove(oldest.unwrap());
    }
    s.add_gadget(ci, DROPLET, DROPLET_DURATION, x, y, 1.0);
}

/// a1PickUp: the first `count` droplets, each healing 7% of max HP (C1: 2
/// energy; C6: a heal of the member lowest on HP).
fn a1_pick_up(s: &mut Sim, ci: usize, mut count: i64) {
    let mut k = 0;
    while k < s.gadgets.len() && count > 0 {
        if !is_droplet(s, &s.gadgets[k]) {
            k += 1;
            continue;
        }
        s.gadgets.remove(k);
        count -= 1;
        let src = s.max_hp(ci) * 0.07;
        let bonus = s.stat(ci, st::HEAL);
        s.heal(ci, ci, src, bonus);
        if s.chars[ci].cons >= 1 {
            s.add_energy(ci, 2.0);
        }
        if s.chars[ci].cons >= 6 {
            let mut lowest = ci;
            for i in 0..s.chars.len() {
                if s.chars[i].hp_ratio < s.chars[lowest].hp_ratio {
                    lowest = i;
                }
            }
            let bonus = s.stat(ci, st::HEAL);
            s.heal_info(ci, Some(lowest), true, 0.06, bonus);
        }
    }
}
