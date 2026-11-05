//! Lauma, from gcsim's internal/characters/lauma, all of it: normal
//! attacks (C6's direct Lunar-Bloom while Pale Hymn lasts), the charged
//! attack and the Spirit Envoy state it enters when ready (no stamina, its
//! cooldown from A4), the pressed and held skill (the held one spends up
//! to three Verdant Dew on a direct Lunar-Bloom and keeps Moon Song),
//! Frostgrove Sanctuary's ticks and RES shred, the burst's Pale Hymn
//! (flat damage on bloom, hyperbloom, burgeon and direct Lunar-Bloom hits),
//! Lunar-Bloom for the fight and its base DMG bonus, A1 (nascent or
//! ascendant), A4 and C1 to C6. gcsim's package has no plunge.
//!
//! gcsim's Spirit Envoy bleeds no stamina: its enter task builds the bleed
//! function and drops it (deerStateStaminaBleed returns the task without
//! running it), so the engine has none either. Pale Hymn and A1 also list
//! Nilou's Bountiful Core, which the engine does not have.

use crate::content::{Character, Event};
use crate::gamedata::chars::lauma as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    ascendant: bool,
    deer_ready: bool,
    deer_src: i32,
    skill_src: i32,
    moon_song: i32,
    moon_song_src: i32,
    c6_count: i32,
    pale: [i32; 3],
    pale_src: [i32; 3],
    /// A4's DMG bonus, from EM at Init.
    a4_dmg: f64,
    /// C1's heal, from EM when the Lunar-Bloom triggered it.
    c1_heal: f64,
}

const C1: ModKey = ModKey::new(g::C1_KEY);
const C1_ICD: ModKey = ModKey::new(g::C1_ICD_KEY);
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_KEY);
const A1: ModKey = ModKey::new(g::A1_KEY);
const A1_ASC: ModKey = ModKey::new("lauma-a1-ascendant");
const A4: ModKey = ModKey::new("lauma-a4");
const C2: ModKey = ModKey::new("lauma-c2-lunarbloom-buff");
const SANCTUARY: ModKey = ModKey::new(g::FROSTGROVE_SANCTUARY_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::FROSTGROVE_SANCTUARY_PARTICLE_ICD_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const MOON_SONG_ICD: ModKey = ModKey::new(g::MOON_SONG_ICD_KEY);
const DEER: ModKey = ModKey::new(g::DEER_STATUS_KEY);
const SHRED_DENDRO: ModKey = ModKey::new("lauma-skill-shred-dendro");
const SHRED_HYDRO: ModKey = ModKey::new("lauma-skill-shred-hydro");
const PALE_KEYS: [ModKey; 3] = [ModKey::new(g::PALE_HYMN_C6_KEY), ModKey::new(g::PALE_HYMN_MOONSONG_KEY), ModKey::new(g::PALE_HYMN_BURST_KEY)];
const PALE_C6: usize = g::PALE_HYMN_C6 as usize;
const PALE_MOONSONG: usize = g::PALE_HYMN_MOONSONG as usize;
const PALE_BURST: usize = g::PALE_HYMN_BURST as usize;

const CB_SHRED: u16 = 1 << 0;
const CB_PARTICLE: u16 = 1 << 1;
const CB_C4: u16 = 1 << 2;
const CB_C6: u16 = 1 << 3;

// arg: the skill's source frame
const TASK_TICK: u8 = 0;
const TASK_HOLD_DEW: u8 = 1;
// arg: the source frame
const TASK_MOON_SONG_END: u8 = 2;
// arg: the source frame * 4 + the Pale Hymn's index
const TASK_PALE_END: u8 = 3;
const TASK_BURST: u8 = 4;
// arg: the deer's duration
const TASK_DEER_ENTER: u8 = 5;
// arg: the deer's source frame
const TASK_DEER_END: u8 = 6;
const TASK_DEER_READY: u8 = 7;
const TASK_C1_HEAL: u8 = 8;

const FRAMES_CHARGE: u8 = 0;
const FRAMES_BURST: u8 = 1;

impl Character for Char {
    const KEY: &'static str = "lauma";
    const MOONSIGN: i32 = 1;
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { ascendant: false, deer_ready: true, deer_src: 0, skill_src: 0, moon_song: 0, moon_song_src: -1, c6_count: 0, pale: [0; 3], pale_src: [0; 3], a4_dmg: 0.0, c1_heal: 0.0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // lunarbloomInit
        s.enable_lunar_bloom();
        let ascendant = s.chars.iter().map(|c| c.moonsign).sum::<i32>() >= 2;
        let (asc, cons) = (s.chars[ci].ascension, s.chars[ci].cons);
        s.st_mut::<State>(ci).ascendant = ascendant;
        if asc >= 1 && ascendant {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(A1_ASC, ModKind::Attack, o, st::NONE, false, false, -1));
            }
        }
        if asc >= 4 {
            let em = s.stat(ci, st::EM);
            s.st_mut::<State>(ci).a4_dmg = (0.004 * em).min(0.32);
            s.chars[ci].mods.push(Mod::owned(A4, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        if cons >= 1 {
            s.subscribe(EventKind::Reaction, o);
        }
        if cons >= 2 && ascendant {
            for i in 0..s.chars.len() {
                s.chars[i].mods.push(Mod::owned(C2, ModKind::ReactBonus, o, st::NONE, false, false, -1));
            }
        }
        // the Lunar-Bloom bonus, A1 (nascent), C6's elevation and Pale Hymn
        s.subscribe(EventKind::EnemyHit, o);
        // chargeInit: Spirit Envoy ends on an action and on a swap
        s.subscribe(EventKind::ActionExec, o);
        s.subscribe(EventKind::Swap, o);
    }

    fn unsupported(a: Action, _: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn invalid(s: &Sim, ci: usize, a: Action, p: &Params) -> Option<String> {
        let deer = s.mod_active(ci, DEER);
        match a {
            Action::Dash if deer => Some("cannot use dash in lauma spirit envoy state".into()),
            Action::Jump if deer => Some("jump in lauma spirit envoy state isn't implemented".into()),
            Action::Skill if p.get("hold").unwrap_or(0) != 0 && s.dew() <= 0 => Some("lauma: Cannot use Skill Hold without Dew".into()),
            _ => crate::content::template_invalid(s, a),
        }
    }

    fn action_stam(s: &Sim, ci: usize, a: Action) -> Option<f64> {
        (a == Action::Charge && s.st::<State>(ci).deer_ready).then_some(0.0)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        if s.chars[ci].cons >= 6 && pale_count(s, ci) > 0 {
            ai.abil = "Normal C6 Pale Hymn";
            ai.tag = AttackTag::DirectLunarBloom;
            ai.durability = 0.0;
            ai.mult = 1.5;
            ai.ignore_def = 1.0;
            ai.use_em = true;
            ai.icd_tag = icd::TAG_NONE;
            consume_pale(s, ci);
        }
        let hb = g::ATTACK_HITBOXES[n];
        ai.pattern = s.box_on_target(0.0, 0.0, hb[0], hb[1]);
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.st::<State>(ci).deer_ready && !s.mod_active(ci, DEER) {
            s.st_mut::<State>(ci).deer_ready = false;
            // enterDeerState
            let dur = g::DEER_DUR + if s.chars[ci].cons >= 1 { 5 * 60 } else { 0 };
            let f = s.f;
            s.st_mut::<State>(ci).deer_src = f;
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_DEER_ENTER, arg: dur }, g::DEER_TRANSFORMATION_FRAME);
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_DEER_END, arg: f }, g::DEER_TRANSFORMATION_FRAME + dur);
            let t = &g::DEER_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Charge);
        }
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.pattern = s.box_on_target(0.0, 0.0, 2.8, 8.0);
        let t = &g::CHARGE_FRAMES;
        s.queue_attack(ai, 0, t[0], g::CHARGE_HITMARK);
        ActInfo::new(Frames::CharLive { id: FRAMES_CHARGE, arg: 0 }, t[0], t[Action::Attack as usize], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        s.add_status(ci, C1, 20 * 60, true);
        s.add_status(ci, A1, 20 * 60, true);
        if s.chars[ci].cons >= 6 {
            s.delete_mod(ci, PALE_KEYS[PALE_C6]);
            let st = s.st_mut::<State>(ci);
            st.pale[PALE_C6] = 0;
            st.pale_src[PALE_C6] = 0;
            st.c6_count = 0;
        }
        s.add_status(ci, SANCTUARY, 15 * 60, true);
        let f = s.f;
        s.st_mut::<State>(ci).skill_src = f;
        for i in 0..g::SKILL_TICKS {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_TICK, arg: f }, g::SKILL_FIRST_TICK_DELAY + g::FROSTGROVE_SANCTUARY_INTERVAL * i);
        }
        let lvl = s.talent(ci, 1);
        let hold = p.get("hold").unwrap_or(0) != 0;
        let (tag, mult, abil, hitmark, t) = if hold {
            (AttackTag::ElementalArtHold, g::SKILL_HOLD1[lvl], "Hymn of Eternal Rest (Hold)", g::SKILL_HOLD_HITMARK, &g::SKILL_FRAMES[1])
        } else {
            (AttackTag::ElementalArt, g::SKILL_PRESS[lvl], "Hymn of Hunting (Press)", g::SKILL_PRESS_HITMARK, &g::SKILL_FRAMES[0])
        };
        let mut ai = AttackInfo::new(ci, tag, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, mult);
        ai.abil = abil;
        ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
        s.queue_attack(ai, CB_SHRED, hitmark, hitmark);
        if hold {
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_HOLD_DEW, arg: 0 }, g::SKILL_CONSUME_DEW);
        }
        s.set_cd_with_delay(ci, Action::Skill, 12 * 60, 13);
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.delete_mod(ci, BURST);
        s.delete_mod(ci, MOON_SONG_ICD);
        s.add_task(Task::Char { c: ci as u8, id: TASK_BURST, arg: 0 }, g::PALE_HYMN_GAIN_FRAME);
        s.consume_energy(ci, 8);
        s.set_cd(ci, Action::Burst, g::PALE_HYMN_DUR);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::CharLive { id: FRAMES_BURST, arg: 0 }, t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn frames_live(s: &Sim, ci: usize, id: u8, _: i16, next: Action) -> i32 {
        let ready = s.st::<State>(ci).deer_ready && next == Action::Charge;
        match id {
            FRAMES_CHARGE if ready => g::CHARGE_RELEASE_FRAME,
            FRAMES_CHARGE => g::CHARGE_FRAMES[next as usize],
            _ if ready => g::BURST_DEER_FRAME,
            _ => g::BURST_FRAMES[next as usize],
        }
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_TICK => {
                if arg != s.st::<State>(ci).skill_src {
                    return;
                }
                let lvl = s.talent(ci, 1);
                let em = s.stat(ci, st::EM);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArtHold, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::FROSTGROVE_SANCTUARY_ATK[lvl]);
                ai.abil = "Frostgrove Sanctuary";
                ai.flat_dmg = em * g::FROSTGROVE_SANCTUARY_EM[lvl];
                ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
                let c4 = if s.chars[ci].cons >= 4 { CB_C4 } else { 0 };
                s.queue_attack(ai, CB_PARTICLE | CB_SHRED | c4, 0, 0);
                // c6OnFrostgroveTick
                if s.chars[ci].cons >= 6 && s.st::<State>(ci).c6_count < 8 {
                    s.st_mut::<State>(ci).c6_count += 1;
                    let mut ai = AttackInfo::new(ci, AttackTag::DirectLunarBloom, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 0.0, 1.85);
                    ai.abil = g::C6_SKILL_HIT_NAME;
                    ai.use_em = true;
                    ai.ignore_def = 1.0;
                    ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
                    s.queue_attack(ai, CB_C6, 16, 16);
                }
            }
            TASK_HOLD_DEW => {
                let n = s.dew().min(3);
                s.consume_dew(n);
                let lvl = s.talent(ci, 1);
                let mut ai = AttackInfo::new(ci, AttackTag::DirectLunarBloom, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 0.0, g::SKILL_HOLD2[lvl] * n as f64);
                ai.abil = "Hymn of Eternal Rest (Hold) (Lunar-Bloom)";
                ai.use_em = true;
                ai.ignore_def = 1.0;
                add_moon_song(s, ci, n);
                ai.pattern = s.circle_on_player(0.0, 0.0, 6.0);
                let d = g::SKILL_HOLD_HITMARK - g::SKILL_CONSUME_DEW;
                s.queue_attack(ai, CB_SHRED, d, d);
            }
            TASK_MOON_SONG_END => {
                let st = s.st_mut::<State>(ci);
                if st.moon_song_src == arg {
                    st.moon_song = 0;
                }
            }
            TASK_PALE_END => {
                let (src, i) = (arg / 4, (arg % 4) as usize);
                let st = s.st_mut::<State>(ci);
                if st.pale_src[i] == src {
                    st.pale[i] = 0;
                }
            }
            TASK_BURST => {
                set_pale(s, ci, PALE_BURST, 18);
                s.add_status(ci, BURST, g::PALE_HYMN_DUR, true);
                // moonSongOnBurst
                if s.st::<State>(ci).moon_song > 0 && s.mod_active(ci, BURST) && !s.mod_active(ci, MOON_SONG_ICD) {
                    let n = s.st::<State>(ci).moon_song;
                    moon_song_to_pale(s, ci, n);
                }
                if s.chars[ci].cons >= 1 {
                    s.add_status(ci, C1, 20 * 60, true);
                }
            }
            TASK_DEER_ENTER => s.add_status(ci, DEER, arg, true),
            TASK_DEER_END => {
                if s.st::<State>(ci).deer_src == arg {
                    end_deer(s, ci);
                }
            }
            TASK_DEER_READY => s.st_mut::<State>(ci).deer_ready = true,
            TASK_C1_HEAL => {
                let active = s.player.active as usize;
                let src = s.st::<State>(ci).c1_heal;
                let bonus = s.stat(ci, st::HEAL);
                s.heal_info(ci, Some(active), false, src, bonus);
            }
            _ => unreachable!(),
        }
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A1_ASC {
            if ev.info.tag == AttackTag::DirectLunarBloom {
                ev.stats[st::CR] += 0.1;
                ev.stats[st::CD] += 0.2;
            }
        } else if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
            ev.stats[st::DMGP] += s.st::<State>(ci).a4_dmg;
        }
    }

    /// C2: direct Lunar-Bloom DMG.
    fn react_bonus(_: &Sim, _: usize, _: usize, _: &Mod, info: &AttackInfo) -> f64 {
        if info.tag == AttackTag::DirectLunarBloom { 0.4 } else { 0.0 }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 198, true);
            let n = if s.rng.float64() < 0.3 { 2.0 } else { 1.0 };
            s.queue_particle(n, Element::Dendro, s.particle_delay[ci]);
        }
        if cbs & CB_SHRED != 0 {
            let v = g::SKILL_RES_SHRED[s.talent(ci, 1)];
            s.enemy_add_resist(SHRED_DENDRO, Element::Dendro, -v, 10 * 60);
            s.enemy_add_resist(SHRED_HYDRO, Element::Hydro, -v, 10 * 60);
        }
        if cbs & CB_C4 != 0 && !s.mod_active(ci, C4_ICD) {
            s.add_energy(ci, 5.0);
            s.add_status(ci, C4_ICD, 5 * 60, true);
        }
        if cbs & CB_C6 != 0 {
            // addC6PaleHymn
            let f = s.f;
            let st = s.st_mut::<State>(ci);
            st.pale[PALE_C6] += 2;
            st.pale_src[PALE_C6] = f;
            s.add_status(ci, PALE_KEYS[PALE_C6], g::PALE_HYMN_DUR, true);
            s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_PALE_END, arg: f * 4 + PALE_C6 as i32 }, g::PALE_HYMN_DUR);
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            Event::EnemyHit(ev) => {
                let tag = ev.info.tag;
                let em = s.stat(ci, st::EM);
                let bloom = matches!(tag, AttackTag::Bloom | AttackTag::Hyperbloom | AttackTag::Burgeon);
                // lunarbloomInit: base DMG on direct Lunar-Bloom
                if tag == AttackTag::DirectLunarBloom {
                    s.hit_base_bonus += (em * 0.000175).min(0.14);
                }
                // A1 (nascent): bloom, hyperbloom and burgeon crit
                let ascendant = s.st::<State>(ci).ascendant;
                if s.chars[ci].ascension >= 1 && !ascendant && bloom && s.mod_active(ci, A1) {
                    s.hit_crit = Some((s.hit_crit.map_or(0.0, |c| c.0) + 0.15, 1.0));
                }
                // C6 (ascendant): direct Lunar-Bloom elevation
                if s.chars[ci].cons >= 6 && ascendant && tag == AttackTag::DirectLunarBloom {
                    s.hit_elevation += (g::C6_ELEVATION_BONUS * 1000.0).round() as u16;
                }
                // Pale Hymn
                if pale_count(s, ci) <= 0 || !(bloom || tag == AttackTag::DirectLunarBloom) {
                    return;
                }
                let lvl = s.talent(ci, 2);
                let c2 = s.chars[ci].cons >= 2;
                s.hit_flat += if bloom {
                    em * g::BLOOM_DMG_INCREASE[lvl] + em * if c2 { 5.0 } else { 0.0 }
                } else {
                    em * g::LUNAR_BLOOM_DMG_INCREASE[lvl] + em * if c2 { 4.0 } else { 0.0 }
                };
                if ev.info.abil == g::C6_SKILL_HIT_NAME {
                    return;
                }
                consume_pale(s, ci);
            }
            // C1: a heal on Lunar-Bloom
            Event::Reaction(Reaction::LunarBloom, ev) if ev.on_enemy() => {
                if !s.mod_active(ci, C1) || s.mod_active(ci, C1_ICD) {
                    return;
                }
                s.add_status(ci, C1_ICD, 114, true);
                let em = s.stat(ci, st::EM);
                s.st_mut::<State>(ci).c1_heal = 5.0 * em;
                s.add_task(Task::Char { c: ci as u8, id: TASK_C1_HEAL, arg: 0 }, g::C1_HIT_MARK);
            }
            Event::ActionExec(a) => {
                if s.mod_active(ci, DEER) && s.player.active as usize == ci && !matches!(a, Action::Jump | Action::Walk) {
                    end_deer(s, ci);
                }
            }
            Event::Swap { prev, .. } => {
                if s.mod_active(ci, DEER) && prev == ci {
                    end_deer(s, ci);
                }
            }
            _ => {}
        }
    }
}

fn pale_count(s: &Sim, ci: usize) -> i32 {
    s.st::<State>(ci).pale.iter().sum()
}

fn consume_pale(s: &mut Sim, ci: usize) {
    let st = s.st_mut::<State>(ci);
    for i in [PALE_C6, PALE_MOONSONG, PALE_BURST] {
        if st.pale[i] > 0 {
            st.pale[i] -= 1;
            return;
        }
    }
}

/// setPaleHymnBurst and setPaleHymnMoonsong.
fn set_pale(s: &mut Sim, ci: usize, i: usize, amount: i32) {
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    st.pale[i] = amount;
    st.pale_src[i] = f;
    s.add_status(ci, PALE_KEYS[i], g::PALE_HYMN_DUR, true);
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_PALE_END, arg: f * 4 + i as i32 }, g::PALE_HYMN_DUR);
}

/// Moon Song becomes Pale Hymn, once a burst.
fn moon_song_to_pale(s: &mut Sim, ci: usize, n: i32) {
    set_pale(s, ci, PALE_MOONSONG, n * 6);
    let d = s.mod_duration(ci, g::BURST_KEY);
    s.add_status(ci, MOON_SONG_ICD, d, true);
    let st = s.st_mut::<State>(ci);
    st.moon_song = 0;
    st.moon_song_src = -1;
}

fn add_moon_song(s: &mut Sim, ci: usize, n: i32) {
    if n <= 0 {
        return;
    }
    if s.mod_active(ci, BURST) && !s.mod_active(ci, MOON_SONG_ICD) {
        moon_song_to_pale(s, ci, n);
        return;
    }
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    st.moon_song = n;
    st.moon_song_src = f;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_MOON_SONG_END, arg: f }, 15 * 60);
}

/// endDeerState: Spirit Envoy's cooldown, shortened by A4.
fn end_deer(s: &mut Sim, ci: usize) {
    s.delete_mod(ci, DEER);
    let red = if s.chars[ci].ascension >= 4 { 1.0 - (s.stat(ci, st::EM) * 0.0002).min(0.2) } else { 1.0 };
    let cd = (4.0 * 60.0 * red) as i32;
    s.add_task(Task::Char { c: ci as u8, id: TASK_DEER_READY, arg: 0 }, cd);
}
