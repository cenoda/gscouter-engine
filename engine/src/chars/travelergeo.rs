//! The geo traveler (aethergeo, luminegeo), from gcsim's
//! internal/characters/traveler/common/geo: normal and charged attacks with
//! A4's Frenzied Rockslide, the skill (tap and short hold, its meteorite
//! with C2's Rockcore Meltdown when it goes), the burst's shockwaves and
//! walls (C1's crit rate inside them, C4's energy), A1's cooldown, C6's
//! durations, and the story-quest buffs.

use crate::chars::traveler::{self, Normals};
use crate::chars::CharState;
use crate::content::Character;
use crate::gcsimdata::chars::traveler_common_geo as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Traveler<const G: usize>;

/// Aether's and Lumine's states are distinct types holding the same.
pub struct State<const G: usize>(Inner);

pub struct Inner {
    story: Stats,
    skill_cd: i32,
    /// The circle of radius 7 around the player as the last burst was
    /// used (gcsim's burstArea), which C1 reads.
    burst_area: Option<Pattern>,
    c1_tick_count: i32,
    /// gcsim's "traveler-c4-src" tag on the enemy, and the energy C4 gave
    /// for the current burst.
    c4_tag: i32,
    c4_count: i32,
    c4_src: i32,
    /// The current skill gave its particles (gcsim's makeParticleCB's
    /// done).
    particle_done: bool,
    /// Where the next meteorite goes and its direction: the enemy's
    /// position and the player's facing as the skill was used.
    stone_at: (f64, f64, f64, f64),
    /// gcsim's Tags["wall"] (C1).
    wall: i64,
    /// The meteorites standing: their frame (key) and position, for C2's
    /// hit where one goes.
    stones: Vec<(i32, f64, f64)>,
}

const N: Normals = crate::traveler_normals!(g);

const STONE: ModKey = ModKey::new("TravellerSkill");
const WALL: ModKey = ModKey::new("TravellerBurst");
const C1: ModKey = ModKey::new("geo-traveler-c1");

const CB_PARTICLE: u16 = 1 << 0;
const CB_C4: u16 = 1 << 1;

const TASK_STONE: u8 = 0;
const TASK_WALLS: u8 = 1;
// arg: the tick number
const TASK_C1: u8 = 2;

fn st(s: &Sim, ci: usize) -> &Inner {
    match &s.chars[ci].state {
        CharState::Aethergeo(x) => &x.0,
        CharState::Luminegeo(x) => &x.0,
        _ => unreachable!(),
    }
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut Inner {
    match &mut s.chars[ci].state {
        CharState::Aethergeo(x) => &mut x.0,
        CharState::Luminegeo(x) => &mut x.0,
        _ => unreachable!(),
    }
}

impl<const G: usize> Character for Traveler<G> {
    const KEY: &'static str = if G == 0 { "aethergeo" } else { "luminegeo" };
    type State = State<G>;

    fn new_state(cs: &CharSetup) -> State<G> {
        State(Inner { story: traveler::story_stats(cs), skill_cd: 8 * 60, burst_area: None, c1_tick_count: 15, c4_tag: 0, c4_count: 0, c4_src: 0, particle_done: false, stone_at: (0.0, 0.0, 0.0, 1.0), wall: 0, stones: Vec::new() })
    }

    fn init(s: &mut Sim, ci: usize) {
        traveler::add_story(s, ci);
        // A1: the skill's cooldown 2 s shorter
        if s.chars[ci].ascension >= 1 {
            st_mut(s, ci).skill_cd -= 2 * 60;
        }
        if s.chars[ci].cons >= 6 {
            st_mut(s, ci).c1_tick_count = 20;
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        // gcsim indexes its four walls with construct_limit
        if a == Action::Burst && p.get("construct_limit").is_some_and(|v| v > 4) {
            return Some("burst[construct_limit] above 4".into());
        }
        traveler::unsupported(a)
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match (fields[0].as_str(), fields.get(1).map(|f| f.as_str())) {
            ("tags", Some("wall")) => Some(Ok(Num::Int(st(s, ci).wall))),
            _ => None,
        }
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        traveler::attack(s, ci, G, &N, |s| a4(s, ci, G))
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        traveler::charge(s, ci, G, &N)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let short = p.get("short_hold").unwrap_or(0).clamp(0, 1) as usize;
        let no_meteorite = p.get("no_meteorite") == Some(1);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 50.0, g::SKILL[lvl]);
        ai.abil = "Starfell Sword";
        ai.blunt = true;
        ai.poise = 133.2;
        ai.hitlag_factor = 0.05;
        ai.hitlag_halt = 0.05 * 60.0;
        ai.def_halt = true;
        ai.deployable = true;
        // a circle of radius 3 where the enemy is as the skill is used
        ai.pattern = s.circle_on_target(3.0);
        s.queue_attack(ai, CB_PARTICLE, 24, g::SKILL_HITMARK[short]);
        let (x, y) = s.enemy_pos;
        let (dx, dy) = s.player_dir;
        let t = st_mut(s, ci);
        t.particle_done = false;
        t.stone_at = (x, y, dx, dy);
        if !no_meteorite {
            s.add_task(Task::Char { c: ci as u8, id: TASK_STONE, arg: 0 }, g::SKILL_HITMARK[short]);
        }
        let cd = st(s, ci).skill_cd;
        s.set_cd_with_delay(ci, Action::Skill, cd, g::SKILL_CD_START[short]);
        let t = &g::SKILL_FRAMES[short][G];
        ActInfo::new(Frames::Table(t), t[0], g::SKILL_CAN_QUEUE_AFTER[short][G], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hits = p.get("hits").unwrap_or(4);
        let limit = p.get("construct_limit").unwrap_or(4) as i32;
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_TRAVELER_WAKE_OF_EARTH, icd::GROUP_DEFAULT, Element::Geo, 50.0, g::BURST[lvl]);
        ai.abil = "Wake of Earth";
        ai.blunt = true;
        ai.poise = 100.0;
        let area = s.circle_on_player(0.0, 0.0, 7.0);
        st_mut(s, ci).burst_area = Some(area);
        let Pattern::Circle { x, y, .. } = area else { unreachable!() };
        ai.pattern = Pattern::Circle { x, y, r: 6.0 };
        let c4 = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
        // the shockwaves' C4 counts per burst
        let f = s.f;
        let t = st_mut(s, ci);
        t.c4_count = 0;
        t.c4_src = f;
        let ev = s.snapshot_event(ai, c4);
        for i in 0..hits as i32 {
            s.queue_attack_with_snap(ev, c4, g::BURST_HITMARK + (i + 1) * 15);
        }
        s.add_task(Task::Char { c: ci as u8, id: TASK_WALLS, arg: limit }, g::BURST_START);
        s.set_cd_with_delay(ci, Action::Burst, 900, g::BURST_START);
        s.consume_energy(ci, 37);
        let t = &g::BURST_FRAMES[G];
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            // the meteorite, where the enemy was and facing as the player
            // did when the skill was used
            TASK_STONE => {
                let (x, y, dx, dy) = st(s, ci).stone_at;
                let mut dur = 30 * 60;
                if s.chars[ci].cons >= 6 {
                    dur += 600;
                }
                let f = s.f;
                st_mut(s, ci).stones.push((f, x, y));
                s.new_construct(Construct { owner: ci as u8, key: STONE, src: f, expiry: f + dur, pos: Some([x, y, dx, dy]), limited: true }, false);
            }
            // the four walls, at +-2.75, +-6.67 from the player, from the
            // top right clockwise (arg: how many)
            TASK_WALLS => {
                if s.chars[ci].cons >= 1 {
                    st_mut(s, ci).wall = 1;
                    s.add_task(Task::Char { c: ci as u8, id: TASK_C1, arg: 1 }, 60);
                }
                let mut dur = 15 * 60;
                if s.chars[ci].cons >= 6 {
                    dur += 300;
                }
                const ANGLES: [f64; 4] = [22.5, 112.5, 202.5, 292.5];
                const OFFSETS: [(f64, f64); 4] = [(2.75, 6.67), (2.75, -6.67), (-2.75, -6.67), (-2.75, 6.67)];
                let f = s.f;
                for i in 0..arg.max(0) as usize {
                    let (sx, sy) = degrees_to_direction(ANGLES[i]);
                    let (dx, dy) = rotate(sx, sy, s.player_dir.0, s.player_dir.1);
                    let (x, y) = s.point_from_player(OFFSETS[i].0, OFFSETS[i].1);
                    s.new_construct(Construct { owner: ci as u8, key: WALL, src: f, expiry: f + dur, pos: Some([x, y, dx, dy]), limited: false }, false);
                }
            }
            TASK_C1 => {
                if !s.constructs.iter().any(|c| c.owner as usize == ci && c.key == WALL) {
                    return;
                }
                if arg > st(s, ci).c1_tick_count {
                    return;
                }
                let area = st(s, ci).burst_area.unwrap();
                if s.player_within(area) {
                    let active = s.player.active as usize;
                    s.add_stat_mod(active, C1, st::CR, 0.1, 120, true);
                }
                s.add_task(Task::Char { c: ci as u8, id: TASK_C1, arg: arg + 1 }, 60);
            }
            _ => unreachable!(),
        }
    }

    fn construct_expired(s: &mut Sim, ci: usize, key: ModKey) {
        if key == WALL {
            if s.chars[ci].cons >= 1 {
                st_mut(s, ci).wall = 0;
            }
            return;
        }
        // the meteorite that went: the one of ours no longer standing
        let t = st(s, ci);
        let Some(i) = t.stones.iter().position(|&(src, ..)| !s.constructs.iter().any(|c| c.owner as usize == ci && c.key == STONE && c.src == src)) else { return };
        let (_, x, y) = st_mut(s, ci).stones.remove(i);
        // C2: Rockcore Meltdown where it stood
        if s.chars[ci].cons >= 2 {
            let lvl = s.talent(ci, 1);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Geo, 50.0, g::SKILL[lvl]);
            ai.abil = "Rockcore Meltdown";
            ai.blunt = true;
            ai.poise = 133.2;
            ai.hitlag_factor = 0.05;
            ai.hitlag_halt = 0.05 * 60.0;
            ai.def_halt = true;
            ai.deployable = true;
            ai.pattern = Pattern::Circle { x, y, r: 3.0 };
            s.queue_attack(ai, 0, 0, 0);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == traveler::STORY {
            traveler::story_amount(&st(s, ci).story, out);
            return true;
        }
        false
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if !ev.on_enemy() {
            return;
        }
        // three or four particles, once per skill
        if cbs & CB_PARTICLE != 0 && !st(s, ci).particle_done {
            st_mut(s, ci).particle_done = true;
            let n = if s.rng.float64() < 0.33 { 4.0 } else { 3.0 };
            s.queue_particle(n, Element::Geo, s.particle_delay[ci]);
        }
        // C4: 5 energy per enemy hit by a shockwave, once per burst on an
        // enemy (gcsim's tag on it holds the burst's frame; a burst at
        // frame 0 does not mark it)
        if cbs & CB_C4 != 0 {
            let src = st(s, ci).c4_src;
            let t = st_mut(s, ci);
            if (t.c4_tag == src && src > 0) || t.c4_count >= 5 {
                return;
            }
            t.c4_tag = src;
            t.c4_count += 1;
            s.add_energy(ci, 5.0);
        }
    }
}

/// A4: the last hit of the combo is followed by a collapse in front of the
/// player.
fn a4(s: &mut Sim, ci: usize, gender: usize) {
    if s.chars[ci].ascension < 4 || s.chars[ci].normal_counter != s.chars[ci].normal_hit_num - 1 {
        return;
    }
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, 0.6);
    ai.abil = "Frenzied Rockslide (A4)";
    ai.blunt = true;
    ai.poise = 13.5;
    ai.pattern = s.circle_on_player(0.0, 1.2, 2.4);
    s.queue_attack_at(ci, ai, 0, g::A4_HITMARK[gender]);
}
