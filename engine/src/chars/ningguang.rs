//! Ningguang, from gcsim's internal/characters/ningguang: normal attacks
//! (a random Left, Right or Twirl after the last, Left after a dash) that
//! gather Star Jades while she is on the field, charged attacks that fire
//! them, the skill's Jade Screen (a construct), the burst with the
//! screen's extra gems, A1 (no stamina for a charged attack with jades),
//! A4 (a dash with the screen up gives Geo DMG), C1, C2 and C6, the
//! conditions .ningguang.jadeCount and .ningguang.prevAttack, and her
//! start delay for Xingqiu's N0. gcsim has no plunges for her.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::ningguang as g;
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

pub struct Char;

/// Normal attacks whose hits can be in the air at once, each with its own
/// `done` flag (gcsim's closure).
const SLOTS: usize = 15;

pub struct State {
    jade_count: i32,
    prev_attack: usize,
    c2_reset: i32,
    last_screen: i32,
    /// The skill's hit's center, and its snapshot (the screen's gems use it).
    skill_pos: (f64, f64),
    skill_snap: Option<AttackEvent>,
    /// The normal attacks' done flags, by slot, and the frame after which
    /// each slot's hits have all landed.
    done: [bool; SLOTS],
    free_at: [i32; SLOTS],
    slot: usize,
}

const SCREEN: ModKey = ModKey::new("ningguang-screen");
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const A4: ModKey = ModKey::new("ning-screen");

const CB_PARTICLE: u16 = 1 << 15;

const TASK_SKILL: u8 = 0;
// arg: the screen's key (the frame it was made)
const TASK_NOSCREEN: u8 = 1;

const LEFT: usize = g::ATTACK_TYPE_LEFT as usize;
const RIGHT: usize = g::ATTACK_TYPE_RIGHT as usize;
const TWIRL: usize = g::ATTACK_TYPE_TWIRL as usize;

/// gcsim's attackOptions and chargeOptions (the same).
const OPTIONS: [[usize; 2]; 3] = [[RIGHT, TWIRL], [LEFT, TWIRL], [LEFT, RIGHT]];

const NORMAL_ABIL: [&str; 3] = ["Normal (Left)", "Normal (Right)", "Normal (Twirl)"];
const CHARGE_ABIL: [&str; 3] = ["Charge (Left)", "Charge (Right)", "Charge (Twirl)"];
const GEM_ABIL: [&str; 3] = ["Charge Gem (Left)", "Charge Gem (Right)", "Charge Gem (Twirl)"];

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn travel(p: &Params, default: i32) -> i32 {
    p.get("travel").map_or(default, |t| t as i32)
}

impl Character for Char {
    const KEY: &'static str = "ningguang";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { jade_count: 0, prev_attack: LEFT, c2_reset: -9999, last_screen: 0, skill_pos: (0.0, 0.0), skill_snap: None, done: [false; SLOTS], free_at: [i32::MIN; SLOTS], slot: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        if s.chars[ci].ascension >= 4 {
            s.subscribe(EventKind::Dash, o);
        }
        s.subscribe(EventKind::Swap, o);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let known: &[&str] = match a {
            Action::Attack | Action::Charge => &["travel"],
            Action::Skill => &["noscreen"],
            Action::Burst => &["travel", "jade", "screen"],
            _ => return Some(name.into()),
        };
        if a == Action::Burst && p.get("jade").is_some_and(|j| j > 6) {
            // gcsim indexes burstHitmarks out of range
            return Some("burst jade above 6".into());
        }
        p.0.iter().find(|(k, _)| !known.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<Num, String>> {
        match fields[0].as_str() {
            "jadeCount" => Some(Ok(Num::Int(st(s, ci).jade_count as i64))),
            "prevAttack" => Some(Ok(Num::Int(st(s, ci).prev_attack as i64))),
            _ => None,
        }
    }

    /// AnimationStartDelay: Xingqiu's N0, by her last normal attack.
    fn start_delay(s: &Sim, ci: usize, key: usize) -> Option<i32> {
        (key == 2).then(|| g::ATTACK_LOCKOUT[st(s, ci).prev_attack])
    }

    /// A1: a charged attack with Star Jades costs no stamina.
    fn charge_stam(s: &Sim, ci: usize) -> Option<f64> {
        Some(if s.chars[ci].ascension >= 1 && st(s, ci).jade_count > 0 { 0.0 } else { 50.0 })
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = travel(p, 10);
        let r = if s.chars[ci].cons >= 1 { 3.5 } else { 0.5 };
        let mut next = OPTIONS[st(s, ci).prev_attack][s.rng.intn(2) as usize];
        if s.current_state() == AnimState::Dash {
            next = LEFT;
        }
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::ATTACK[lvl]);
        ai.abil = NORMAL_ABIL[next];
        ai.blunt = true;
        ai.poise = 45.0;
        ai.pattern = s.circle_on_target(r);
        let h = g::ATTACK_HITMARKS[next];
        // the shared done flag of this attack's two hits
        let f = s.f;
        let stt = st_mut(s, ci);
        let slot = stt.slot;
        stt.slot = (slot + 1) % SLOTS;
        let busy = f <= stt.free_at[slot];
        stt.done[slot] = false;
        stt.free_at[slot] = f + h + travel.max(0);
        if busy {
            s.error = Some(format!("ningguang: more than {SLOTS} normal attacks in the air"));
        }
        for _ in 0..2 {
            s.queue_attack(ai, 1 << slot, h, h + travel);
        }
        st_mut(s, ci).prev_attack = next;
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[next];
        ActInfo::new(Frames::Attack { table, atkspd }, table[0], g::ATTACK_LOCKOUT[next], AnimState::Normal)
    }

    fn charge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = travel(p, 10);
        let prev = st(s, ci).prev_attack;
        let mut ty = OPTIONS[prev][s.rng.intn(2) as usize];
        let state = s.current_state();
        if state == AnimState::Normal {
            ty = if prev == RIGHT { RIGHT } else { LEFT };
        }
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::CHARGE[lvl]);
        ai.abil = CHARGE_ABIL[ty];
        ai.blunt = true;
        ai.poise = 45.0;
        ai.pattern = s.circle_on_target(1.5);
        let windup = if matches!(state, AnimState::Normal | AnimState::Charge) { 15 } else { 0 };
        let h = g::CHARGE_HITMARKS[ty] - windup;
        s.queue_attack(ai, 0, h, h + travel);

        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_DEFAULT, Element::Geo, 50.0, g::JADE[lvl]);
        ai.abil = GEM_ABIL[ty];
        ai.blunt = true;
        ai.poise = 30.0;
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = s.circle_on_target(0.5);
        let jades = st(s, ci).jade_count;
        let marks = if jades == 7 { g::CHARGE_C6_HITMARKS } else { g::CHARGE_JADE_HITMARKS };
        let h = marks[ty] - windup;
        for _ in 0..jades {
            s.queue_attack(ai, 0, h, h + travel);
        }
        st_mut(s, ci).jade_count = 0;

        let t = &g::CHARGE_FRAMES[ty];
        let min = *t.iter().min().unwrap();
        ActInfo::new(Frames::Char { id: ty as u8, arg: windup as i16 }, t[0] - windup, min - windup, AnimState::Charge)
    }

    fn frames(id: u8, windup: i16, next: Action) -> i32 {
        g::CHARGE_FRAMES[id as usize][next as usize] - windup as i32
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let (dx, dy) = s.player_dir;
        let (x, y) = s.point_from_player(0.0, 3.0);
        // the hit's snapshot and attack at its hitmark
        s.add_task(Task::Char { c: ci as u8, id: TASK_SKILL, arg: 0 }, g::SKILL_HITMARK);
        st_mut(s, ci).skill_pos = (x, y);
        // the cooldown first, then the screen (C2 checks it)
        s.set_cd(ci, Action::Skill, 720);
        let f = s.f;
        s.new_construct(Construct { owner: ci as u8, key: SCREEN, src: f, expiry: f + 1800, pos: Some([x, y, dx, dy]), limited: true }, true);
        st_mut(s, ci).last_screen = f;
        if p.get("noscreen").is_some_and(|v| v != 0) {
            s.add_task(Task::Char { c: ci as u8, id: TASK_NOSCREEN, arg: 0 }, 1);
        }
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = travel(p, 0);
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Geo, 50.0, g::BURST[lvl]);
        ai.abil = "Starshatter";
        ai.blunt = true;
        ai.poise = 30.0;
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = s.circle_on_target(0.5);
        let jade = p.get("jade").unwrap_or(6);
        for i in 0..jade.max(0) as usize {
            s.queue_attack(ai, 0, 0, g::BURST_HITMARKS[i] + travel);
        }
        // the screen, if it still stands, adds its gems
        let last = st(s, ci).last_screen;
        if s.destroy_construct(last) {
            let screen = p.get("screen").unwrap_or(6);
            ai.abil = "Starshatter (Jade Screen Gems)";
            match st(s, ci).skill_snap {
                Some(mut ev) => {
                    ev.info = ai;
                    ev.callbacks = 0;
                    let d = g::BURST_HITMARKS[g::BURST_HITMARKS.len() - 1] + 30 + travel;
                    for _ in 0..screen {
                        s.queue_attack_with_snap(ev, 0, d);
                    }
                }
                None if screen > 0 => s.error = Some("ningguang: burst on a screen before its snapshot".into()),
                None => {}
            }
        }
        if s.chars[ci].cons >= 6 {
            st_mut(s, ci).jade_count = 7;
        }
        s.consume_energy(ci, 3);
        s.set_cd(ci, Action::Burst, 720);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, _: i32) {
        match id {
            TASK_SKILL => {
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Geo, 25.0, g::SKILL[lvl]);
                ai.abil = "Jade Screen";
                ai.blunt = true;
                ai.poise = 133.2;
                ai.hitlag_halt = 0.05 * 60.0;
                ai.hitlag_factor = 0.05;
                ai.def_halt = true;
                ai.deployable = true;
                let (x, y) = st(s, ci).skill_pos;
                ai.pattern = Pattern::Circle { x, y, r: 5.0 };
                let ev = s.snapshot_event(ai, CB_PARTICLE);
                st_mut(s, ci).skill_snap = Some(ev);
                s.queue_attack_with_snap(ev, CB_PARTICLE, 0);
            }
            TASK_NOSCREEN => {
                let last = st(s, ci).last_screen;
                s.destroy_construct(last);
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 360, true);
            let count = if s.rng.float64() < 0.33 { 4.0 } else { 3.0 };
            s.queue_particle(count, Element::Geo, s.particle_delay[ci]);
        }
        let slots = cbs & !CB_PARTICLE;
        if slots == 0 {
            return;
        }
        // a normal attack's star jade, once, and not off the field
        if s.player.active as usize != ci {
            return;
        }
        let slot = slots.trailing_zeros() as usize;
        let stt = st_mut(s, ci);
        if stt.done[slot] {
            return;
        }
        if stt.jade_count != 7 {
            stt.jade_count = (stt.jade_count + 1).min(3);
        }
        stt.done[slot] = true;
    }

    /// C2: the screen's end resets the skill's cooldown, at most every 6 s.
    fn construct_expired(s: &mut Sim, ci: usize, _: ModKey) {
        if s.chars[ci].cons < 2 {
            return;
        }
        let f = s.f;
        if st(s, ci).c2_reset <= f - 360 && s.cooldown(ci, Action::Skill) > 0 {
            st_mut(s, ci).c2_reset = f;
            s.reset_cd(ci, Action::Skill);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // leaving the field drops the jades
            Event::Swap { prev, .. } => {
                if prev == ci {
                    st_mut(s, ci).jade_count = 0;
                }
            }
            // A4: a dash with the screen up
            Event::Dash => {
                if s.constructs.iter().any(|c| c.limited && c.owner as usize == ci && c.key == SCREEN) {
                    let a = s.player.active as usize;
                    s.add_stat_mod(a, A4, st::GEOP, 0.12, 600, true);
                }
            }
            _ => {}
        }
    }
}
