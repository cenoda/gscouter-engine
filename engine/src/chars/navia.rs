//! Navia, from gcsim's internal/characters/navia: normal attacks, Rosula
//! Shardshot pressed or held (pulling crystallize shards in while held),
//! its Surging Blade (the arkhe), the burst and its Cannon Fire Support,
//! A1, A4, every constellation, the Xianyun plunges and the `shrapnel`
//! condition. gcsim's package has no charged attack: it is an error, as
//! in gcsim.
//! Crystal Shrapnel comes from crystallize shields and, once a second,
//! from Lunar-Crystallize on the enemy.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsl::Num;
use crate::gamedata::chars::navia as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

/// A shard the held skill pulls in: gcsim's AddShieldKillShard runs on the
/// shard when it arrives, whatever happened to it meanwhile.
#[derive(Clone, Copy)]
struct Pull {
    src: i32,
    earliest_pickup: i32,
    shield: Shield,
}

#[derive(Default)]
pub struct State {
    shrapnel: i32,
    /// Surging Blade, between its trigger and its attack: where it lands
    /// and the shrapnel its buffs count.
    blade_pos: (f64, f64),
    blade_count: i32,
    /// The burst's attack info, for its ticks.
    burst_ai: Option<AttackInfo>,
    /// Shards on their way, by slot (a task's arg).
    pulls: [Option<Pull>; 16],
    next_pull: usize,
}

const A1: ModKey = ModKey::new(g::A1_KEY);
const LCR_SHRAPNEL_ICD: ModKey = ModKey::new("navia-lcr-shrapnel-icd");
const A4: ModKey = ModKey::new("navia-a4");
const ARKHE_ICD: ModKey = ModKey::new(g::ARKHE_ICD_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const BURST_ICD: ModKey = ModKey::new(g::BURST_ICD_KEY);
const C2_ICD: ModKey = ModKey::new(g::C2_ICD_KEY);
const C4: ModKey = ModKey::new("navia-c4-shred");

const CB_PARTICLE: u16 = 1 << 0;
const CB_C2: u16 = 1 << 1;
const CB_BURST: u16 = 1 << 2;
const CB_C4: u16 = 1 << 3;

const TASK_FIRE: u8 = 0;
const TASK_BLADE_ICD: u8 = 1;
const TASK_BLADE: u8 = 2;
const TASK_BURST_START: u8 = 3;
const TASK_BURST_TICK: u8 = 4;
// arg: the firing frame
const TASK_PULL: u8 = 5;
// arg: the pull's slot
const TASK_PICKUP: u8 = 6;

const FR_SKILL: u8 = 0;

/// The airborne plunges (Xianyun's), from gcsim's plunge.go: the hits and
/// the collision Y 1 ahead, the collision of radius 1.
const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

/// The skill's frames argument: the hold, whether it was held, and whether
/// it fired with 3 or more shrapnel (set when it fires).
fn skill_arg(hold: i32, held: bool, three: bool) -> i16 {
    (hold * 4 + if held { 2 } else { 0 } + if three { 1 } else { 0 }) as i16
}

impl Character for Char {
    const KEY: &'static str = "navia";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // the held skill pulls shards by where they lie
        s.shard_positions = true;
        // skill charges
        let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
        cd.available = 2;
        cd.max_extra = 1;
        // A4: ATK +20% per pyro, electro, cryo or hydro member, up to 2
        if s.chars[ci].ascension >= 4 {
            let n = s.chars.iter().filter(|c| matches!(c.element, Element::Pyro | Element::Electro | Element::Cryo | Element::Hydro)).count().min(2);
            s.chars[ci].mods.push(Mod::stat(A4, st::ATKP, 0.2 * n as f64, false, false, -1));
        }
        // shrapnel from Lunar-Crystallize and from crystallize shields
        // picked up (shrapnelInit)
        s.subscribe(EventKind::Reaction, o);
        s.subscribe(EventKind::Shielded, o);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            Action::Attack | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    /// Plunges only while airborne (Xianyun's jump).
    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        (fields[0] == "shrapnel").then(|| Ok(Num::Int(s.st::<State>(ci).shrapnel as i64)))
    }

    /// A1: geo on normal, charged and plunging attacks, which nothing
    /// overrides, while its buff lasts.
    fn snapshot(s: &Sim, ci: usize, info: &mut AttackInfo) {
        if s.mod_active(ci, A1) && matches!(info.tag, AttackTag::Normal | AttackTag::Plunge | AttackTag::Extra) {
            info.element = Element::Geo;
        }
    }

    /// A1: normal, charged and plunging attacks DMG +40%.
    fn attack_mod(_: &Sim, _: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            ev.stats[st::DMGP] += 0.4;
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::AUTO[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.blunt = true;
            ai.poise = g::ATTACK_POISE_DMG[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            ai.deployable = n == 2;
            let (off, hb) = (g::ATTACK_OFFSETS[n], g::ATTACK_HITBOXES[n]);
            ai.pattern = if n == 0 { s.circle_on_player(0.0, off, hb[0]) } else { s.box_on_player(0.0, off, hb[0], hb[1]) };
            // snapshot at the earliest cancel (N3's gadget), so that the
            // infusion is snapped
            s.queue_attack(ai, 0, g::ATTACK_EARLIEST_CANCEL[n], g::ATTACK_HITMARKS[n][i]);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], g::ATTACK_EARLIEST_CANCEL[n], AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let mut hold = p.get("hold").unwrap_or(0).max(0) as i32;
        let mut firing = g::SKILL_PRESS_CD_START;
        let held = hold > 0;
        if held {
            hold = hold.min(g::SKILL_MAX_HOLD_DURATION) - 1;
            firing = g::SKILL_HOLD_CD_START + hold;
            // at 0.2 s the press becomes a hold and the suction begins
            let firing_f = s.f + firing;
            let mut i = 12;
            while i < firing {
                s.add_task(Task::Char { c: ci as u8, id: TASK_PULL, arg: firing_f }, i);
                i += 30;
            }
        }
        s.set_cd_with_delay(ci, Action::Skill, 9 * 60, firing);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_FIRE, arg: 0 }, firing);
        let hi = held as usize;
        let len = g::SKILL_FRAMES[hi][1][0] + hold;
        let queue = g::SKILL_FRAMES[hi][0][Action::Jump as usize] + hold;
        ActInfo::new(Frames::Char { id: FR_SKILL, arg: skill_arg(hold, held, false) }, len, queue, AnimState::Skill)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        debug_assert_eq!(id, FR_SKILL);
        let (hold, hi, si) = ((arg / 4) as i32, ((arg >> 1) & 1) as usize, (arg & 1) as usize);
        g::SKILL_FRAMES[hi][si][next as usize] + hold
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 50.0, g::BURST[0][lvl]);
        ai.abil = "As the Sunlit Sky's Singing Salute";
        ai.blunt = true;
        ai.poise = 100.0;
        ai.pattern = s.box_on_player(0.0, 0.0, 5.0, 12.0);
        let cbs = burst_cbs(s, ci);
        s.queue_attack(ai, cbs, g::BURST_HITMARK, g::BURST_HITMARK);
        // the Cannon Fire Support's info
        ai.abil = "Cannon Fire Support";
        ai.icd_tag = icd::TAG_ELEMENTAL_BURST;
        ai.icd_group = icd::GROUP_NAVIA_BURST;
        ai.poise = 50.0;
        ai.durability = 25.0;
        ai.mult = g::BURST[1][lvl];
        s.st_mut::<State>(ci).burst_ai = Some(ai);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BURST_START, arg: 0 }, g::BURST_DELAY);
        s.consume_energy(ci, 12);
        s.set_cd(ci, Action::Burst, 15 * 60);
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
            TASK_FIRE => fire(s, ci),
            TASK_BLADE_ICD => {
                s.add_status(ci, ARKHE_ICD, 7 * 60, true);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BLADE, arg: 0 }, 36);
            }
            TASK_BLADE => {
                let lvl = s.talent(ci, 1);
                let (pos, count) = (s.st::<State>(ci).blade_pos, s.st::<State>(ci).blade_count);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 0.0, g::SKILLBLADE[lvl]);
                ai.abil = "Surging Blade";
                ai.pattern = Pattern::Circle { x: pos.0, y: pos.1, r: 3.0 };
                let mut ev = s.snapshot_event(ai, 0);
                add_shrapnel_buffs(s, ci, &mut ev.stats, count);
                s.queue_attack_with_snap(ev, 0, 0);
            }
            // the cannon ticks over 12 s: every 42 frames, 48 after the
            // 2nd, 5th, 8th, 11th and 14th
            TASK_BURST_START => {
                s.add_status(ci, BURST, g::BURST_DURATION, false);
                let (mut i, mut tick) = (0, 0);
                while i <= g::BURST_DURATION {
                    tick += 1;
                    s.add_task(Task::Char { c: ci as u8, id: TASK_BURST_TICK, arg: 0 }, i);
                    i += if tick % 3 == 2 { 48 } else { 42 };
                }
            }
            TASK_BURST_TICK => {
                let Some(mut ai) = s.st::<State>(ci).burst_ai else { return };
                let (x, y) = cannon_pos(s);
                ai.pattern = Pattern::Circle { x, y, r: 3.0 };
                let cbs = burst_cbs(s, ci);
                s.queue_attack(ai, cbs, 0, 9);
            }
            TASK_PULL => pull_shards(s, ci, arg),
            TASK_PICKUP => {
                let Some(p) = s.st_mut::<State>(ci).pulls[arg as usize].take() else { return };
                // gcsim's AddShieldKillShard
                if s.f < p.earliest_pickup {
                    return;
                }
                if let Some(k) = s.shards.iter().position(|sh| sh.src == p.src) {
                    s.shards.remove(k);
                }
                let mut shield = p.shield;
                shield.expiry = s.f + (15.1 * 60.0) as i32;
                s.add_shield(shield);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 12, true);
            let count = if s.rng.float64() < 0.5 { 4.0 } else { 3.0 };
            s.queue_particle(count, Element::Geo, s.particle_delay[ci]);
        }
        // C2: one Cannon Fire Support near the hit, per shot
        if cbs & CB_C2 != 0 && !s.mod_active(ci, C2_ICD) {
            s.add_status(ci, C2_ICD, 15, true);
            let lvl = s.talent(ci, 2);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_NAVIA_BURST, Element::Geo, 25.0, g::BURST[1][lvl]);
            ai.abil = "Cannon Fire Support (C2)";
            ai.blunt = true;
            ai.poise = 50.0;
            let (x, y) = s.random_point(s.enemy_pos.0, s.enemy_pos.1, 0.0, 1.2);
            ai.pattern = Pattern::Circle { x, y, r: 3.0 };
            let cbs = burst_cbs(s, ci);
            s.queue_attack(ai, cbs, 0, 30);
        }
        // a cannon hit: a shrapnel, once every 2.4 s
        if cbs & CB_BURST != 0 && !s.mod_active(ci, BURST_ICD) {
            s.add_status(ci, BURST_ICD, 144, true);
            let st = s.st_mut::<State>(ci);
            st.shrapnel = (st.shrapnel + 1).min(6);
        }
        // C4: geo RES -20% for 8 s
        if cbs & CB_C4 != 0 {
            s.enemy_add_resist(C4, Element::Geo, -0.2, 8 * 60);
        }
    }

    /// A crystallize shield picked up, or Lunar-Crystallize on the enemy
    /// (once a second): a shrapnel, up to 6.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Reaction(Reaction::LunarCrystallize, ev) if ev.on_enemy() => {
                if s.mod_active(ci, LCR_SHRAPNEL_ICD) {
                    return;
                }
                s.add_status(ci, LCR_SHRAPNEL_ICD, 60, true);
                let st = s.st_mut::<State>(ci);
                st.shrapnel = (st.shrapnel + 1).min(6);
            }
            Event::Shielded(sh) if sh.typ == ShieldType::Crystallize => {
                let st = s.st_mut::<State>(ci);
                st.shrapnel = (st.shrapnel + 1).min(6);
            }
            _ => {}
        }
    }
}

/// The burst's callbacks: its shrapnel, and C4.
fn burst_cbs(s: &Sim, ci: usize) -> u16 {
    CB_BURST | if s.chars[ci].cons >= 4 { CB_C4 } else { 0 }
}

/// The shot's buffs from its shrapnel: DMG +15% per stack over 3, C2's
/// CRIT Rate +12% per stack up to 3, C6's CRIT DMG +45% per stack over 3.
fn add_shrapnel_buffs(s: &Sim, ci: usize, stats: &mut Stats, count: i32) {
    let excess = (count - 3).max(0) as f64;
    stats[st::DMGP] += 0.15 * excess;
    if s.chars[ci].cons >= 2 {
        stats[st::CR] += 0.12 * count.min(3) as f64;
    }
    if s.chars[ci].cons >= 6 {
        stats[st::CD] += 0.45 * excess;
    }
}

/// The skill fires: the shotgun's shots that reach the enemy decide its
/// multiplier, then Surging Blade, A1 and C1, and the shrapnel is spent.
fn fire(s: &mut Sim, ci: usize) {
    let shrapnel = s.st::<State>(ci).shrapnel;
    // the frames of the skill that fired, if it still runs
    if shrapnel >= 3 {
        if let Some(a) = &mut s.player.anim {
            if a.char as usize == ci {
                if let Frames::Char { id: FR_SKILL, arg } = &mut a.frames {
                    *arg |= 1;
                }
            }
        }
    }
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, 0.0);
    ai.abil = "Rosula Shardshot";
    ai.blunt = true;
    ai.poise = 150.0;
    ai.pattern = Pattern::Single;
    let mut ev = s.snapshot_event(ai, 0);
    add_shrapnel_buffs(s, ci, &mut ev.stats, shrapnel);
    let area = s.box_on_player(-0.20568, -0.043841, 4.0722, 11.5461);
    if s.enemy_alive && s.enemy_within(area) {
        let shots = 5 + shrapnel.min(3) * 2;
        let mut hits = 0;
        for h in &g::HITSCANS[..shots as usize] {
            let (dx, dy) = degrees_to_direction(h[1]);
            let (ox, oy) = rotate(h[2], h[3], dx, dy);
            if s.hits_enemy(s.box_on_player(ox, oy, h[0], g::BULLET_BOX_LENGTH)) == Some(true) {
                hits += 1;
            }
        }
        ev.info.mult = g::SKILLSHOTGUN[lvl] * g::SKILL_MULTIPLIER[hits];
        ev.callbacks = CB_PARTICLE | if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
        s.queue_attack_with_snap(ev, ev.callbacks, g::TRAVEL_DELAY);
    }
    surging_blade(s, ci, shrapnel);
    // A1: geo and DMG +40% on normal, charged and plunging attacks for 4 s
    if s.chars[ci].ascension >= 1 {
        s.add_mod(ci, Mod::owned(A1, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 4 * 60);
    }
    // C1: per shrapnel spent (up to 3), the burst's cooldown -1 s and 3
    // energy
    if s.chars[ci].cons >= 1 {
        let n = shrapnel.min(3);
        s.reduce_cd(ci, Action::Burst, n * 60);
        s.add_energy(ci, (n * 3) as f64);
    }
    // C6 keeps what is over three
    let left = if s.chars[ci].cons < 6 { 0 } else { (shrapnel - 3).max(0) };
    s.st_mut::<State>(ci).shrapnel = left;
}

/// Surging Blade, once every 7 s: on the enemy if it is in the shotgun's
/// area, else 3.6 ahead of the player; its cooldown 28 frames later and
/// its attack 36 after that.
fn surging_blade(s: &mut Sim, ci: usize, count: i32) {
    if s.mod_active(ci, ARKHE_ICD) {
        return;
    }
    let area = s.box_on_player(-0.20568, -0.043841, 4.0722, 11.5461);
    let pos = if s.enemy_alive && s.enemy_within(area) { s.enemy_pos } else { offset_point(s.player_pos, (0.0, 3.6), s.player_dir) };
    let st = s.st_mut::<State>(ci);
    st.blade_pos = pos;
    st.blade_count = count;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BLADE_ICD, arg: 0 }, 28);
}

/// gcsim's calcCannonPos: a random enemy within 10 of the player (Intn of
/// one draws too), then a random point within 1.2 of it; without one, a
/// random point 1 to 6 from 4 ahead of the player.
fn cannon_pos(s: &mut Sim) -> (f64, f64) {
    let area = s.circle_on_player(0.0, 0.0, 10.0);
    if s.enemy_alive && s.enemy_within(area) {
        s.rng.intn(1);
        let (x, y) = s.enemy_pos;
        return s.random_point(x, y, 0.0, 1.2);
    }
    let (x, y) = offset_point(s.player_pos, (0.0, 4.0), s.player_dir);
    s.random_point(x, y, 1.0, 6.0)
}

/// gcsim's pullCrystals: each shard within 12 of the player comes at 0.4
/// a frame and is picked up on arrival, if it arrives before the shot and
/// not before it can be picked up.
fn pull_shards(s: &mut Sim, ci: usize, firing_f: i32) {
    let area = s.circle_on_player(0.0, 0.0, 12.0);
    let (px, py) = s.player_pos;
    for k in 0..s.shards.len() {
        let sh = s.shards[k];
        if area.contains_point(sh.pos.0, sh.pos.1) != Some(true) {
            continue;
        }
        let d = ((sh.pos.0 - px).powi(2) + (sh.pos.1 - py).powi(2)).sqrt();
        let travel = (d / 0.4).ceil() as i32;
        if s.f + travel >= firing_f || s.f + travel < sh.earliest_pickup {
            continue;
        }
        let st = s.st_mut::<State>(ci);
        let slot = st.next_pull;
        st.next_pull = (slot + 1) % st.pulls.len();
        st.pulls[slot] = Some(Pull { src: sh.src, earliest_pickup: sh.earliest_pickup, shield: sh.shield });
        s.add_task(Task::Char { c: ci as u8, id: TASK_PICKUP, arg: slot as i32 }, travel);
    }
}
