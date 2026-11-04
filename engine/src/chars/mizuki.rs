//! Yumemizuki Mizuki, from gcsim's internal/characters/mizuki: normal and
//! charged attacks, the skill's Dreamdrifter state (its clouds, its Swirl
//! DMG bonus for the party, ended by her skill or her leaving the field),
//! the burst and its snacks (gadgets the player picks up: a shockwave or a
//! heal), A1's extensions, A4's EM, and C1 to C6. gcsim gives her no
//! plunges; they stay errors.
//!
//! A snack is picked up by the player overlapping it, which gcsim checks at
//! the start of each frame (`Sim::collide_gadgets`).

use crate::content::{Character, Event};
use crate::gcsimdata::chars::mizuki as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Clone, Copy)]
struct Snack {
    atk: AttackEvent,
    allow_f: i32,
}

pub struct State {
    particles_left: i32,
    extensions_left: i32,
    cloud: AttackEvent,
    cloud_src: i32,
    cloud_travel: i32,
    c1_em: f64,
    c2_bonus: f64,
    c4_left: i32,
    snacks: [Snack; SNACK_SLOTS],
    next_snack: u8,
}

const SNACK_SLOTS: usize = 8;

const DD: ModKey = ModKey::new(g::DREAM_DRIFTER_STATE_KEY);
const SWIRL_BUFF: ModKey = ModKey::new(g::DREAM_DRIFTER_SWIRL_BUFF_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::SKILL_PARTICLE_GENERATION_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const A1_ICD: ModKey = ModKey::new(g::A1_ICD_KEY);
const A4: ModKey = ModKey::new(g::A4_KEY);
const A4_ICD: ModKey = ModKey::new(g::A4_ICD_KEY);
const C1: ModKey = ModKey::new(g::C1_KEY);
const C2: ModKey = ModKey::new(g::C2_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);

const CB_PARTICLE: u16 = 1 << 0;

const TASK_CLOUD: u8 = 0;
const TASK_C1: u8 = 1;
const TASK_C2: u8 = 2;
const TASK_SNACK: u8 = 3;

const FR_CHARGE: u8 = 0;

fn st(s: &Sim, ci: usize) -> &State {
    s.st::<State>(ci)
}

fn st_mut(s: &mut Sim, ci: usize) -> &mut State {
    s.st_mut::<State>(ci)
}

fn blank() -> AttackEvent {
    AttackEvent::new(AttackInfo::new(0, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::NoElement, 0.0, 0.0), 0)
}

impl Character for Char {
    const KEY: &'static str = "yumemizukimizuki";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State {
            particles_left: 0,
            extensions_left: 0,
            cloud: blank(),
            cloud_src: -1,
            cloud_travel: 0,
            c1_em: 0.0,
            c2_bonus: 0.0,
            c4_left: 0,
            snacks: [Snack { atk: blank(), allow_f: 0 }; SNACK_SLOTS],
            next_snack: 0,
        }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        s.collide_checks = true;
        // the Swirl DMG bonus in Dreamdrifter, on everyone
        for i in 0..s.chars.len() {
            s.chars[i].mods.push(Mod::owned(SWIRL_BUFF, ModKind::ReactBonus, o, st::NONE, false, false, -1));
        }
        s.subscribe(EventKind::Swap, o);
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        if asc >= 1 {
            s.subscribe(EventKind::Reaction, o);
        }
        if asc >= 4 || cons >= 1 || cons >= 6 {
            s.subscribe(EventKind::EnemyHit, o);
        }
        if cons >= 2 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C2, arg: 0 }, g::C2_INTERVAL as i32);
            for i in 0..s.chars.len() {
                if i != ci {
                    s.chars[i].mods.push(Mod::owned(C2, ModKind::Stat, o, st::NONE, false, false, -1));
                }
            }
        }
        if cons >= 6 {
            s.subscribe(EventKind::SpecialReactionAttack, o);
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(C6, ModKind::Attack, o, st::NONE, false, false, -1));
            }
        }
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Skill => p.0.iter().find(|(k, _)| k != "travel").map(|(k, _)| format!("{name} param {k}")),
            Action::Charge | Action::Burst => p.0.first().map(|(k, _)| format!("{name} param {k}")),
            _ => Some(name.into()),
        }
    }

    /// In Dreamdrifter the skill (which ends it) is always ready, and only
    /// dash, swap and burst can be used besides.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        if !s.mod_active(ci, DD) {
            return None;
        }
        if a == Action::Skill {
            return Some(None);
        }
        if !matches!(a, Action::Dash | Action::Swap | Action::Burst) {
            return Some(Some(Failure::NoFailure));
        }
        None
    }

    fn attack(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").unwrap_or(10) as i32;
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.hitlag_factor = 0.05;
        ai.pattern = s.circle_toward_target(0.0, 0.0, 0.7);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h + travel);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let windup = if s.current_state() == AnimState::Idle { 0 } else { 14 };
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge";
        ai.hitlag_factor = 0.05;
        ai.pattern = s.circle_on_target(3.5);
        let h = g::CHARGE_HITMARK - windup;
        s.queue_attack(ai, 0, h, h);
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Char { id: FR_CHARGE, arg: windup as i16 }, t[0] - windup, t[Action::Dash as usize] - windup, AnimState::Charge)
    }

    fn frames(id: u8, arg: i16, next: Action) -> i32 {
        match id {
            FR_CHARGE => g::CHARGE_FRAMES[next as usize] - arg as i32,
            _ => unreachable!(),
        }
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let t = &g::SKILL_FRAMES;
        let info = ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill);
        // pressed in Dreamdrifter, it ends it
        if s.mod_active(ci, DD) {
            cancel(s, ci);
            return info;
        }
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, g::SKILL_ACTIVATE_DURABILITY as f64, g::SKILL[lvl]);
        ai.abil = g::SKILL_ACTIVATE_DMG_NAME;
        ai.poise = g::SKILL_ACTIVATE_POISE as f64;
        ai.hitlag_factor = 0.05;
        ai.pattern = s.circle_on_player(0.0, 0.0, g::SKILL_ACTIVATE_DMG_RADIUS);
        s.queue_attack(ai, CB_PARTICLE, 0, g::SKILL_HITMARK);
        st_mut(s, ci).particles_left = g::SKILL_PARTICLE_GENERATIONS;
        if s.chars[ci].ascension >= 1 {
            st_mut(s, ci).extensions_left = g::DREAM_DRIFTER_EXTENSIONS;
        }
        let travel = p.get("travel").unwrap_or(g::CLOUD_TRAVEL_TIME as i64) as i32;
        // applyDreamDrifterEffect
        s.add_status(ci, DD, g::DREAM_DRIFTER_BASE_DURATION, true);
        // the clouds snapshot now
        let lvl = s.talent(ci, 1);
        let mut cloud = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_MIZUKI_SKILL, Element::Anemo, g::CLOUD_DURABILITY as f64, g::CLOUD_DMG[lvl]);
        cloud.abil = g::CLOUD_DMG_NAME;
        cloud.poise = g::CLOUD_POISE as f64;
        cloud.hitlag_factor = 0.05;
        let snap = s.snapshot_event(cloud, 0);
        let f = s.f;
        let x = st_mut(s, ci);
        x.cloud = snap;
        x.cloud_src = f;
        x.cloud_travel = travel;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CLOUD, arg: f }, g::CLOUD_FIRST_HIT);
        if s.chars[ci].cons >= 1 {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C1, arg: f }, g::SKILL_HITMARK + 2);
        }
        s.set_cd_with_delay(ci, Action::Skill, g::SKILL_CD, g::SKILL_CD_DELAY);
        info
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Anemo, g::BURST_DURABILITY as f64, g::BURST_DMG[lvl]);
        ai.abil = g::BURST_ACTIVATION_DMG_NAME;
        ai.poise = g::BURST_POISE as f64;
        ai.hitlag_factor = 0.05;
        ai.pattern = s.circle_on_target(g::BURST_RADIUS as f64);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.add_status(ci, BURST, g::BURST_DURATION, false);
        if s.chars[ci].cons >= 4 {
            st_mut(s, ci).c4_left = g::C4_ENERGY_GENERATIONS;
        }
        // a snack every 1.5 s from the hit on
        let interval = g::SNACK_INTERVAL as i32;
        let mut i = interval;
        while i <= g::BURST_DURATION {
            s.add_task(Task::Char { c: ci as u8, id: TASK_SNACK, arg: 0 }, g::BURST_HITMARK + i);
            i += interval;
        }
        s.consume_energy(ci, g::BURST_ENERGY_DRAIN_DELAY);
        s.set_cd_with_delay(ci, Action::Burst, g::BURST_CD, g::BURST_CD_DELAY);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_CLOUD => {
                if st(s, ci).cloud_src != arg || !s.mod_active(ci, DD) {
                    return;
                }
                let mut atk = st(s, ci).cloud;
                atk.info.pattern = s.circle_on_target(g::CLOUD_EXPLOSION_RADIUS as f64);
                let travel = st(s, ci).cloud_travel;
                s.queue_attack_with_snap(atk, CB_PARTICLE, travel);
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_CLOUD, arg }, g::CLOUD_HIT_INTERVAL);
            }
            // C1: Twenty-Three Nights' Awaiting on the enemies near her
            TASK_C1 => {
                if st(s, ci).cloud_src != arg || !s.mod_active(ci, DD) {
                    return;
                }
                st_mut(s, ci).c1_em = s.stat(ci, st::EM);
                let area = s.circle_on_player(0.0, 0.0, g::C1_RANGE as f64);
                if s.enemy_alive && s.enemy_within(area) {
                    s.enemy_add_status(C1, g::C1_DURATION, true);
                }
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C1, arg }, g::C1_INTERVAL as i32);
            }
            // C2: the bonus follows her EM every 0.5 s
            TASK_C2 => {
                st_mut(s, ci).c2_bonus = s.stat_ex(ci, st::EM, true) * g::C2_EM_MULTIPLIER;
                s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_C2, arg: 0 }, g::C2_INTERVAL as i32);
            }
            TASK_SNACK => spawn_snack(s, ci),
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && ev.on_enemy() && !s.mod_active(ci, PARTICLE_ICD) && st(s, ci).particles_left > 0 {
            s.add_status(ci, PARTICLE_ICD, g::SKILL_PARTICLE_GENERATION_ICD as i32, false);
            st_mut(s, ci).particles_left -= 1;
            s.queue_particle(1.0, Element::Anemo, s.particle_delay[ci]);
        }
    }

    /// Dreamdrifter's Swirl DMG bonus from her EM, not on the amplified or
    /// catalyzed part of an AoE swirl.
    fn react_bonus(s: &Sim, ci: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        if m.key != SWIRL_BUFF || !s.mod_active(ci, DD) || info.amped || info.catalyzed.is_some() {
            return 0.0;
        }
        let lvl = s.talent(ci, 1);
        match info.tag {
            AttackTag::SwirlCryo | AttackTag::SwirlElectro | AttackTag::SwirlHydro | AttackTag::SwirlPyro => g::SWIRL_DMG[lvl] * s.stat(ci, st::EM) * 0.01,
            AttackTag::DirectStellarSwirl | AttackTag::ReactionStellarSwirl => g::STELLAR_SWIRL_DMG[lvl] * s.stat(ci, st::EM) * 0.01,
            _ => 0.0,
        }
    }

    /// C2: pyro, hydro, cryo and electro DMG in Dreamdrifter.
    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key != C2 || !s.mod_active(ci, DD) {
            return false;
        }
        let b = st(s, ci).c2_bonus;
        out[st::PYROP] += b;
        out[st::HYDROP] += b;
        out[st::ELECTROP] += b;
        out[st::CRYOP] += b;
        true
    }

    /// C6: direct Stellar Swirl DMG's CRIT.
    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == C6 && ev.info.tag == AttackTag::DirectStellarSwirl {
            ev.stats[st::CR] += 0.1;
            ev.stats[st::CD] += 0.2;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::Swap { prev, .. } => {
                if prev == ci && s.mod_active(ci, DD) {
                    cancel(s, ci);
                }
            }
            // A1: her swirls extend Dreamdrifter
            Event::Reaction(Reaction::SwirlPyro | Reaction::SwirlHydro | Reaction::SwirlElectro | Reaction::SwirlCryo | Reaction::StellarSwirl, ev) => {
                if !ev.on_enemy() || ev.info.actor as usize != ci || !s.mod_active(ci, DD) || st(s, ci).extensions_left <= 0 || s.mod_active(ci, A1_ICD) {
                    return;
                }
                s.add_status(ci, A1_ICD, g::A1_ICD as i32, true);
                s.extend_mod(ci, DD, g::DREAM_DRIFTER_DURATION_EXTENSION as i32);
                st_mut(s, ci).extensions_left -= 1;
            }
            Event::EnemyHit(ev) => enemy_hit(s, ci, ev),
            // C6: Stellar Swirl's own CRIT in Dreamdrifter
            Event::SpecialReactionAttack(ev) => {
                if ev.info.tag == AttackTag::ReactionStellarSwirl && s.mod_active(ci, DD) {
                    s.special_atk.stats[st::CR] += 0.1;
                    s.special_atk.stats[st::CD] += 0.2;
                }
            }
            _ => {}
        }
    }

    fn gadget_tick(s: &mut Sim, ci: usize, id: u8, timer: i32) {
        // the snack's time is up: it explodes by itself
        if timer == g::SNACK_DURATION {
            explode(s, ci, id);
        }
    }

    fn gadget_collide(s: &mut Sim, ci: usize, id: u8) {
        if s.f < st(s, ci).snacks[id as usize].allow_f {
            return;
        }
        // outside Dreamdrifter, only within the snack's own size
        if !s.mod_active(ci, DD) {
            let Some(k) = s.gadget_index(ci, id) else { return };
            let (x, y) = (s.gadgets[k].x, s.gadgets[k].y);
            if !s.player_within(Pattern::Circle { x, y, r: g::SNACK_SIZE }) {
                return;
            }
        }
        picked_up(s, ci, id);
    }
}

fn cancel(s: &mut Sim, ci: usize) {
    s.delete_mod(ci, DD);
    st_mut(s, ci).cloud_src = -1;
}

/// The others' OnEnemyHit subscribers in gcsim's Init order: A4, C1, C6.
fn enemy_hit(s: &mut Sim, ci: usize, ev: &AttackEvent) {
    let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
    // A4: the others' pyro, hydro, cryo and electro hits in Dreamdrifter
    if asc >= 4 && ev.info.actor as usize != ci && s.mod_active(ci, DD) && !s.mod_active(ci, A4_ICD) {
        s.add_status(ci, A4_ICD, g::A4_ICD as i32, true);
        if matches!(ev.info.element, Element::Electro | Element::Hydro | Element::Pyro | Element::Cryo) {
            s.add_stat_mod(ci, A4, st::EM, g::A4_EM_BUFF as f64, g::A4_DURATION, true);
        }
    }
    let swirl = matches!(ev.info.tag, AttackTag::SwirlCryo | AttackTag::SwirlElectro | AttackTag::SwirlHydro | AttackTag::SwirlPyro);
    // C1: a swirl on an enemy under the effect
    if cons >= 1 && s.enemy_status_active(C1) && (swirl || matches!(ev.info.tag, AttackTag::DirectStellarSwirl | AttackTag::ReactionStellarSwirl)) && ev.info.flat_dmg + s.hit_flat != 0.0 {
        let name = s.hit_abil.unwrap_or(ev.info.abil);
        let Some(renamed) = c1_name(name) else {
            s.unsupported(&format!("mizuki C1 on {name}"));
            return;
        };
        s.hit_flat += g::C1_MULTIPLIER * st(s, ci).c1_em;
        s.hit_abil = Some(renamed);
        s.enemy_delete_status(C1);
    }
    // C6: swirls crit at a fixed 30% and 100%
    if cons >= 6 && s.mod_active(ci, DD) && swirl {
        s.hit_cr_set = Some(g::C6_CR);
        s.hit_crit = Some((0.0, g::C6_CD));
    }
}

/// gcsim appends " (Mizuki C1)" to the swirl's name.
fn c1_name(abil: &str) -> Option<&'static str> {
    Some(match abil {
        "swirl-pyro" => "swirl-pyro (Mizuki C1)",
        "swirl-hydro" => "swirl-hydro (Mizuki C1)",
        "swirl-electro" => "swirl-electro (Mizuki C1)",
        "swirl-cryo" => "swirl-cryo (Mizuki C1)",
        "swirl-pyro (aoe)" => "swirl-pyro (aoe) (Mizuki C1)",
        "swirl-hydro (aoe)" => "swirl-hydro (aoe) (Mizuki C1)",
        "swirl-electro (aoe)" => "swirl-electro (aoe) (Mizuki C1)",
        "swirl-cryo (aoe)" => "swirl-cryo (aoe) (Mizuki C1)",
        "Stellar Swirl" => "Stellar Swirl (Mizuki C1)",
        "Stellar Swirl Detonation" => "Stellar Swirl Detonation (Mizuki C1)",
        _ => return None,
    })
}

/// A snack where calculateSnackSpawnLocation puts it: on the edge of the
/// closest enemy within 6 of the player toward the player (on the player
/// without one), moved by up to 1 on each axis.
fn spawn_snack(s: &mut Sim, ci: usize) {
    let (px, py) = s.player_pos;
    let (mut x, mut y) = (px, py);
    let area = s.circle_on_player(0.0, 0.0, g::SNACK_SPAWN_ON_ENEMY_RADIUS as f64);
    if s.enemy_alive && s.enemy_within(area) {
        let (ex, ey) = s.enemy_pos;
        (x, y) = (ex, ey);
        if (x, y) != (px, py) {
            let (dx, dy) = (px - ex, py - ey);
            let n = (dx * dx + dy * dy).sqrt();
            x += s.target_radius * dx / n;
            y += s.target_radius * dy / n;
        }
    }
    let v = g::SNACK_SPAWN_LOCATION_VARIANCE;
    let r = s.rng.float64();
    y += r * v * random_sign(s);
    let r = s.rng.float64();
    x += r * v * random_sign(s);
    // newSnack: the shockwave snapshots now
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Anemo, g::SNACK_DURABILITY as f64, g::SNACK_DMG[lvl]);
    ai.abil = g::SNACK_DMG_NAME;
    ai.hitlag_factor = 0.05;
    ai.pattern = Pattern::Circle { x, y, r: g::SNACK_DMG_RADIUS as f64 };
    let atk = s.snapshot_event(ai, 0);
    let allow_f = s.f + g::SNACK_CANT_TRIGGER_DURATION as i32;
    let x0 = st_mut(s, ci);
    let id = x0.next_snack;
    x0.next_snack = (x0.next_snack + 1) % SNACK_SLOTS as u8;
    x0.snacks[id as usize] = Snack { atk, allow_f };
    s.add_gadget(ci, id, g::SNACK_DURATION, x, y, g::SNACK_SIZE * g::SNACK_SIZE_MIZUKI_MULTIPLIER);
    s.set_gadget_collides_player(ci, id);
}

fn random_sign(s: &mut Sim) -> f64 {
    if 0.5 < s.rng.float64() { -1.0 } else { 1.0 }
}

fn explode(s: &mut Sim, ci: usize, id: u8) {
    let atk = st(s, ci).snacks[id as usize].atk;
    s.queue_attack_with_snap(atk, 0, 0);
}

/// The shockwave above 70% HP (and with C4), the heal below (and with C4),
/// C4's energy; then the snack is gone.
fn picked_up(s: &mut Sim, ci: usize, id: u8) {
    let active = s.player.active as usize;
    let (dmg, heal) = if s.chars[ci].cons >= 4 {
        (true, true)
    } else {
        let d = s.current_hp(active) > s.max_hp(active) * g::SNACK_HEAL_TRIGGER_HP_RATIO;
        (d, !d)
    };
    if dmg {
        explode(s, ci, id);
    }
    if heal {
        let lvl = s.talent(ci, 2);
        let mult = if active == ci { 2.0 } else { 1.0 };
        let src = (s.stat(ci, st::EM) * g::SNACK_HEAL_EM[lvl] + g::SNACK_HEAL_FLAT[lvl]) * mult;
        let bonus = s.stat(ci, st::HEAL);
        s.heal(ci, active, src, bonus);
    }
    if s.chars[ci].cons >= 4 && st(s, ci).c4_left > 0 {
        st_mut(s, ci).c4_left -= 1;
        s.add_energy(ci, g::C4_ENERGY as f64);
    }
    s.kill_gadget(ci, id);
}
