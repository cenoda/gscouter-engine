//! Amber, from gcsim's internal/characters/amber: normal attacks, aimed
//! shots (`hold` 0 or 1, `travel`, `weakspot`, and C2's `bunny`, which
//! shoots the first Baron Bunny), the skill's Baron Bunny (`hold` delays
//! it), the burst's Fiery Rain, A1, A4 and every constellation.
//!
//! Baron Bunny is a hittable gadget 1.4 ahead of the player, of radius 1,
//! for 484 frames: it explodes when it expires or is killed (C2's manual
//! detonation, an overload by her charged attack around it) with the
//! snapshot and the circle on the player taken as it landed. Like gcsim's
//! Bunny it has its own reactable and ICD: an attack that reaches it with
//! cryo, after the bunny's own ICD, attaches cryo to it, which decays and
//! which absorb checks read. On that cryo, pyro melts (only the event), an
//! anemo hit swirls (the swirl's aoe, a circle of 5 on the bunny that skips
//! it, reaches the enemy) and electro superconducts (a circle of 3 on the
//! bunny), each with the bunny's own GCD, the events marked as on a gadget.
//! Geo crystallizes it, its shard drawn around the bunny. Freeze on the
//! bunny, and Stellar reactions there, are not ported and are an error
//! that names them.

use crate::content::{Character, Event, EventKind, Owner};
use crate::gamedata::chars::amber as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

/// One (character, ICD tag, group) of the bunny's own ICD (gcsim's
/// icdState: the element application counter and whether its reset timer
/// runs).
#[derive(Clone, Copy)]
struct BunnyIcd {
    char: u8,
    tag: IcdTag,
    group: IcdGroup,
    counter: usize,
    on_timer: bool,
}

struct Bunny {
    /// Its gadget id.
    id: u8,
    /// The explosion, snapshot as it landed.
    ev: AttackEvent,
    icd: Vec<BunnyIcd>,
    /// Its reactable's swirl cryo and superconduct GCDs (-1 for none).
    swirl_gcd: i32,
    superconduct_gcd: i32,
    crystallize_gcd: i32,
}

#[derive(Default)]
pub struct State {
    /// The bunnies out, in the order they came (gcsim's c.bunnies).
    bunnies: Vec<Bunny>,
    next_id: u8,
    /// The swirl aoes of bunnies, by the frame they land and the bunny,
    /// which they skip (gcsim's IgnoredKeys).
    swirl_skip: Vec<(i32, u8)>,
}

const A1: ModKey = ModKey::new("amber-a1");
const A4: ModKey = ModKey::new("amber-a4");
const C2: ModKey = ModKey::new("amber-c2");
const C6: ModKey = ModKey::new("amber-c6");

const CB_A4: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;

const TASK_BUNNY: u8 = 0;
const TASK_MANUAL: u8 = 1;
/// arg: the bunny's id << 16 | the ICD entry.
const TASK_ICD_RESET: u8 = 2;

const FR_SKILL: u8 = 0;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];

fn bunny_index(s: &Sim, ci: usize, id: u8) -> Option<usize> {
    s.st::<State>(ci).bunnies.iter().position(|b| b.id == id)
}

/// gcsim's Bunny.explode: the explosion a frame later, and the bunny is
/// gone from the list.
fn explode(s: &mut Sim, ci: usize, id: u8) {
    let Some(i) = bunny_index(s, ci, id) else { return };
    let b = s.st_mut::<State>(ci).bunnies.remove(i);
    s.queue_attack_with_snap(b.ev, CB_PARTICLE, 1);
}

/// gcsim's Gadget.Kill: the bunny is removed and explodes, as a manual
/// detonation.
fn kill(s: &mut Sim, ci: usize, id: u8) {
    if let Some(i) = bunny_index(s, ci, id) {
        s.st_mut::<State>(ci).bunnies[i].ev.info.abil = g::MANUAL_EXPLOSION_ABIL;
    }
    s.kill_gadget(ci, id);
    explode(s, ci, id);
}

/// WillApplyEle on bunny `i`.
fn bunny_will_apply(s: &mut Sim, ci: usize, i: usize, info: &AttackInfo) -> f64 {
    if info.icd_tag == icd::TAG_NONE {
        return 1.0;
    }
    let st = s.st_mut::<State>(ci);
    let b = &mut st.bunnies[i];
    let id = b.id;
    let k = match b.icd.iter().position(|x| x.char == info.actor && x.tag == info.icd_tag && x.group == info.icd_group) {
        Some(k) => k,
        None => {
            b.icd.push(BunnyIcd { char: info.actor, tag: info.icd_tag, group: info.icd_group, counter: 0, on_timer: false });
            b.icd.len() - 1
        }
    };
    let start = !b.icd[k].on_timer;
    b.icd[k].on_timer = true;
    let val = b.icd[k].counter;
    b.icd[k].counter += 1;
    if start {
        s.add_task(Task::Char { c: ci as u8, id: TASK_ICD_RESET, arg: (id as i32) << 16 | k as i32 }, info.icd_group.reset_timer() - 1);
    }
    let seq = info.icd_group.ele_seq();
    if val < seq.len() { seq[val] } else { seq[seq.len() - 1] }
}

/// gcsim's Bunny.React on the cryo bunny `i` (gadget `k`): melt, cryo
/// swirl and superconduct; the rest is an error.
fn react(s: &mut Sim, ci: usize, i: usize, k: usize, orig: &AttackEvent, dur: f64) {
    let mut ev = *orig;
    ev.info.durability = dur;
    ev.on_gadget = true;
    let who = crate::gcsl::char_name(s.chars[ev.info.actor as usize].key);
    let frozen = s.gadgets[k].aura.get(AURA_FROZEN) >= ZERO_DUR;
    let (gx, gy) = (s.gadgets[k].x, s.gadgets[k].y);
    match ev.info.element {
        Element::Pyro if !frozen => {
            // TryMelt: only the reaction, on the bunny
            let consumed = s.gadgets[k].aura.reduce(AURA_CRYO, dur, 2.0);
            ev.info.durability = (ev.info.durability - consumed).max(0.0);
            ev.reacted = true;
            ev.info.amped = true;
            ev.info.amp_mult = 2.0;
            s.emit(Event::Reaction(Reaction::Melt, &ev));
        }
        Element::Anemo if !frozen && !s.stellar.swirl => {
            let rd = s.gadgets[k].aura.reduce(AURA_CRYO, dur, 0.5);
            let aoe_dur = if rd < dur { 1.25 * (0.5 * rd - 1.0) + 25.0 } else { 1.25 * (dur - 1.0) + 25.0 };
            ev.info.durability -= rd;
            ev.reacted = true;
            s.emit(Event::Reaction(Reaction::SwirlCryo, &ev));
            let f = s.f;
            let b = &mut s.st_mut::<State>(ci).bunnies[i];
            if b.swirl_gcd == -1 || f >= b.swirl_gcd {
                b.swirl_gcd = f + 6;
                let id = b.id;
                // the self hit lands on the bunny alone, with no gauge:
                // nothing; the aoe skips the bunny
                let mut ai = AttackInfo::new(ev.info.actor as usize, AttackTag::SwirlCryo, icd::TAG_SWIRL_CRYO, icd::GROUP_REACTION_A, Element::Cryo, aoe_dur, 0.0);
                ai.abil = "swirl-cryo (aoe)";
                ai.ignore_def = 1.0;
                ai.pattern = Pattern::Circle { x: gx, y: gy, r: 5.0 };
                let mut atk = s.reaction_attack(ai);
                atk.info.flat_dmg *= 0.6;
                s.st_mut::<State>(ci).swirl_skip.push((f + 1 + 4, id));
                s.queue_attack_with_snap(atk, 0, 1 + 4);
            }
        }
        Element::Electro if !frozen && !s.stellar.conduct => {
            let consumed = s.gadgets[k].aura.reduce(AURA_CRYO, dur, 1.0);
            ev.info.durability = (ev.info.durability - consumed).max(0.0);
            ev.reacted = true;
            s.emit(Event::Reaction(Reaction::Superconduct, &ev));
            let f = s.f;
            let b = &mut s.st_mut::<State>(ci).bunnies[i];
            if b.superconduct_gcd == -1 || f >= b.superconduct_gcd {
                b.superconduct_gcd = f + 6;
                let mut ai = AttackInfo::new(ev.info.actor as usize, AttackTag::SuperconductDamage, icd::TAG_SUPERCONDUCT_DAMAGE, icd::GROUP_REACTION_A, Element::Cryo, 0.0, 0.0);
                ai.abil = "superconduct";
                ai.ignore_def = 1.0;
                ai.pattern = Pattern::Circle { x: gx, y: gy, r: 3.0 };
                let mut atk = s.reaction_attack(ai);
                atk.info.flat_dmg *= 1.5;
                s.queue_attack_with_snap(atk, 0, 1);
            }
        }
        Element::Geo => {
            // TryCrystallizeCryo, with the bunny's own GCD; the shard lies
            // around the bunny
            if s.gadgets[k].aura.get(AURA_CRYO) <= ZERO_DUR {
                s.unsupported(&format!("a reaction on Amber's Baron Bunny ({who} {})", ev.info.abil));
                return;
            }
            let f = s.f;
            let b = &mut s.st_mut::<State>(ci).bunnies[i];
            if b.crystallize_gcd != -1 && f < b.crystallize_gcd {
                return;
            }
            b.crystallize_gcd = f + 60;
            let r = s.gadgets[k].r;
            s.add_task(Task::SpawnShardAt { actor: ev.info.actor, ele: Element::Cryo, at: (gx, gy, r) }, 23);
            let consumed = s.gadgets[k].aura.reduce(AURA_CRYO, dur, 0.5);
            ev.info.durability = (ev.info.durability - consumed).max(0.0);
            ev.reacted = true;
            s.emit(Event::Reaction(Reaction::CrystallizeCryo, &ev));
        }
        _ => s.unsupported(&format!("a reaction on Amber's Baron Bunny ({who} {})", ev.info.abil)),
    }
}

impl Character for Char {
    const KEY: &'static str = "amber";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // C4: two skill charges
        if s.chars[ci].cons >= 4 {
            let cd = &mut s.chars[ci].cd[crate::sim::Char::cd_index(Action::Skill)];
            cd.available = 2;
            cd.max_extra = 1;
        }
        // A1: Fiery Rain's CRIT Rate (and its 30% wider area)
        if s.chars[ci].ascension >= 1 {
            s.add_mod(ci, Mod::owned(A1, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        }
        // C2: the manual detonation's DMG, and overload by her charged
        // attack kills the bunnies around it
        if s.chars[ci].cons >= 2 {
            s.add_mod(ci, Mod::owned(C2, ModKind::Attack, o, st::NONE, false, false, -1), -1);
            s.subscribe(EventKind::Reaction, o);
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let known: &[&str] = match a {
            Action::Attack => &["travel"],
            Action::Aim => &["hold", "travel", "weakspot", "bunny"],
            Action::Skill => &["hold"],
            Action::Burst => &[],
            _ => return Some(crate::gcsl::action_name(a).into()),
        };
        let name = crate::gcsl::action_name(a);
        p.0.iter().find(|(k, _)| !known.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
    }

    /// gcsim's error for an aim with another hold.
    fn invalid(s: &Sim, _: usize, a: Action, p: &Params) -> Option<String> {
        if a == Action::Aim {
            if let Some(h) = p.get("hold") {
                if !(0..=1).contains(&h) {
                    return Some(format!("invalid hold param supplied, got {h}"));
                }
            }
        }
        crate::content::template_invalid(s, a)
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_AMBER, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").map_or(10, |t| t as i32);
        let weakspot = p.get("weakspot") == Some(1);
        let bunny = p.get("bunny").unwrap_or(0) != 0;
        let h = g::AIMED_HITMARKS[hold];
        let t = &g::AIMED_FRAMES[hold];
        let info = ActInfo::new(Frames::Table(t), t[0], h, AnimState::Aim);
        // C2: a fully charged shot at the first bunny, which deals nothing
        if s.chars[ci].cons >= 2 && bunny && hold == 1 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_MANUAL, arg: 0 }, h + travel);
            return info;
        }
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_AMBER, Element::Pyro, 50.0, g::FULLAIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        ai.weak_point = weakspot;
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let a4 = if s.chars[ci].ascension >= 4 { CB_A4 } else { 0 };
        s.queue_attack(ai, a4, h, h + travel);
        if s.chars[ci].cons >= 1 {
            ai.mult *= 0.2;
            ai.abil = "Aimed Shot (C1)";
            s.queue_attack(ai, 0, h + g::C1_DELAY, h + g::C1_DELAY + travel);
        }
        info
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(0) as i32;
        s.add_task(Task::Char { c: ci as u8, id: TASK_BUNNY, arg: 0 }, g::BUNNY_LAND + hold);
        let cd = if s.chars[ci].cons >= 4 { 720 } else { 900 };
        s.set_cd_with_delay(ci, Action::Skill, cd, g::SKILL_START + hold);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Char { id: FR_SKILL, arg: hold as i16 }, t[0] + hold, g::SKILL_START + hold, AnimState::Skill)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_SKILL => g::SKILL_FRAMES[next as usize] + arg as i32,
            _ => unreachable!(),
        }
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_AMBER, Element::Pyro, 25.0, g::BURST_TICK[lvl]);
        ai.abil = "Fiery Rain";
        let ev = s.snapshot_event(ai, 0);
        let r = if s.chars[ci].ascension >= 1 { 2.0 * 1.3 } else { 2.0 };
        let (cx, cy) = s.enemy_pos;
        // arrows every 0.4 s, every 0.6 s and every 0.2 s for 2 s, each at
        // a random point around the enemy
        for (step, min) in [(24, r), (36, r), (12, 0.1)] {
            let mut i = step;
            while i <= 120 {
                let (x, y) = s.random_point(cx, cy, min, r);
                let mut e = ev;
                e.info.pattern = Pattern::Circle { x, y, r };
                s.queue_attack_with_snap(e, 0, g::BURST_START + i);
                i += step;
            }
        }
        if s.chars[ci].cons >= 6 {
            for i in 0..s.chars.len() {
                s.add_mod(i, Mod::stat(C6, st::ATKP, 0.15, false, true, -1), 900);
            }
        }
        s.set_cd_with_delay(ci, Action::Burst, 720, 56);
        s.consume_energy(ci, 59);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_BUNNY => {
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 50.0, g::BUNNY_EXPLODE[lvl]);
                ai.abil = "Baron Bunny";
                ai.blunt = true;
                ai.poise = 260.0;
                ai.pattern = s.circle_on_player(0.0, 0.0, 3.0);
                let ev = s.snapshot_event(ai, CB_PARTICLE);
                let (x, y) = s.point_from_player(0.0, 1.4);
                let st = s.st_mut::<State>(ci);
                let id = st.next_id;
                st.next_id = st.next_id.wrapping_add(1);
                st.bunnies.push(Bunny { id, ev, icd: Vec::new(), swirl_gcd: -1, superconduct_gcd: -1, crystallize_gcd: -1 });
                s.add_hittable_gadget(ci, id, 484, x, y, 1.0, false);
            }
            TASK_MANUAL => {
                // only the first bunny, when there is one
                if let Some(b) = s.st::<State>(ci).bunnies.first() {
                    let id = b.id;
                    kill(s, ci, id);
                }
            }
            TASK_ICD_RESET => {
                let (id, k) = ((arg >> 16) as u8, (arg & 0xffff) as usize);
                if let Some(i) = bunny_index(s, ci, id) {
                    let e = &mut s.st_mut::<State>(ci).bunnies[i].icd[k];
                    e.counter = 0;
                    e.on_timer = false;
                }
            }
            _ => unreachable!(),
        }
    }

    /// A bunny that expired explodes.
    fn gadget_tick(s: &mut Sim, ci: usize, id: u8, _: i32) {
        if s.gadget_index(ci, id).is_none() && bunny_index(s, ci, id).is_some() {
            explode(s, ci, id);
        }
    }

    /// gcsim's Bunny.HandleAttack: the bunny's ICD, then its reactions
    /// (none ported) on the cryo it holds, then cryo attaches.
    fn gadget_hit(s: &mut Sim, ci: usize, id: u8, ev: &AttackEvent, lands: Option<bool>) {
        let Some(i) = bunny_index(s, ci, id) else { return };
        if ev.info.durability <= 0.0 || ev.info.element == Element::Physical {
            return;
        }
        let Some(k) = s.gadget_index(ci, id) else { return };
        let has_cryo = s.gadgets[k].aura.get(AURA_CRYO) > ZERO_DUR || s.gadgets[k].aura.get(AURA_FROZEN) > ZERO_DUR;
        let reacts = has_cryo && matches!(ev.info.element, Element::Electro | Element::Pyro | Element::Hydro | Element::Anemo | Element::Geo);
        match lands {
            Some(true) => {}
            Some(false) => return,
            None => {
                if ev.info.element == Element::Cryo || reacts {
                    let who = crate::gcsl::char_name(s.chars[ev.info.actor as usize].key);
                    s.unsupported(&format!("hit pattern {who} {}", ev.info.abil));
                }
                return;
            }
        }
        // its own swirl's aoe skips it
        if ev.info.tag == AttackTag::SwirlCryo {
            let f = s.f;
            let st = s.st_mut::<State>(ci);
            st.swirl_skip.retain(|x| x.0 >= f);
            if st.swirl_skip.contains(&(f, id)) {
                return;
            }
        }
        let dur = ev.info.durability * bunny_will_apply(s, ci, i, &ev.info);
        if dur <= 0.0 {
            return;
        }
        if reacts {
            react(s, ci, i, k, ev, dur);
            return;
        }
        if ev.info.element == Element::Cryo && dur >= ZERO_DUR {
            s.gadgets[k].aura.attach_normal(AURA_CRYO, dur, ev.info.actor as usize);
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        // A4: a weak point hit raises her ATK by 15% for 10 s
        if cbs & CB_A4 != 0 && ev.info.weak_point {
            s.add_mod(ci, Mod::stat(A4, st::ATKP, 0.15, false, true, -1), 600);
        }
        if cbs & CB_PARTICLE != 0 {
            s.queue_particle(4.0, Element::Pyro, s.particle_delay[ci]);
        }
    }

    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A1 {
            if ev.info.tag == AttackTag::ElementalBurst {
                ev.stats[st::CR] += 0.1;
            }
        } else if ev.info.tag == AttackTag::ElementalArt && ev.info.abil == g::MANUAL_EXPLOSION_ABIL {
            ev.stats[st::DMGP] += 2.0;
        }
    }

    /// C2: an overload by her charged attack kills the bunnies within 3
    /// of the enemy.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(Reaction::Overload, ev) = *e else { return };
        if !ev.on_enemy() || s.st::<State>(ci).bunnies.is_empty() || ev.info.actor as usize != ci || ev.info.tag != AttackTag::Extra {
            return;
        }
        let (ex, ey) = s.enemy_pos;
        let area = Pattern::Circle { x: ex, y: ey, r: 3.0 };
        let ids: Vec<u8> = s.st::<State>(ci).bunnies.iter().map(|b| b.id).collect();
        for id in ids {
            let Some(k) = s.gadget_index(ci, id) else { continue };
            let (x, y) = (s.gadgets[k].x, s.gadgets[k].y);
            if area.contains_point(x, y) == Some(true) {
                kill(s, ci, id);
            }
        }
    }
}
