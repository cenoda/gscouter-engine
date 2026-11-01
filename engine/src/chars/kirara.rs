//! Kirara, from gcsim's internal/characters/kirara: normal and charged
//! attacks, Meow-teor Kick (pressed, short hold and hold: the kicks, the
//! Urgent Neko Parcel's ticks, the Shield of Safe Transport and the dendro
//! on the player), Secret Art: Surprise Dispatch (the box and the Cat Grass
//! Cardamoms), A1, A4 and every constellation.
//!
//! As gcsim writes them: C2 is co-op only and does nothing; C6's bonus
//! comes from casting the skill or the burst, shield or not. A1's stacks
//! are never reset, so after the first ones a held skill's end always
//! makes its shields from them. The cardamoms' snapshot is taken 34 frames
//! into the burst, so with `mine_delay` below that the hits use the last
//! burst's snapshot (none before the first: no stats). gcsim has no
//! plunges for her.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::kirara as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    /// The Urgent Neko Parcel's hit as snapshot when the skill starts: the
    /// ticks reuse it.
    parcel: Option<AttackEvent>,
    /// The cardamoms' explosion as last snapshot (gcsim's mineSnap).
    mine_snap: Option<AttackEvent>,
    cardamoms: i32,
    a1_stacks: i32,
    /// Shields of Safe Transport waiting their frame to be added (gcsim's
    /// genShield adds them a frame later), by slot.
    pending: [f64; 4],
    next_pending: u8,
}

const KICK_PARTICLE_ICD: ModKey = ModKey::new(g::KICK_PARTICLE_ICD_KEY);
const ROLL_PARTICLE_ICD: ModKey = ModKey::new(g::ROLL_PARTICLE_ICD_KEY);
const A1_ICD: ModKey = ModKey::new(g::A1_ICD_STATUS);
const A4_SKILL: ModKey = ModKey::new("kirara-a4-skill");
const A4_BURST: ModKey = ModKey::new("kirara-a4-burst");
const C4_ICD: ModKey = ModKey::new(g::C4_ICD_STATUS);
const C6: ModKey = ModKey::new(g::C6_STATUS);
const MINE_EXPIRED: ModKey = ModKey::new(g::MINE_EXPIRED);
const SHIELD: ModKey = ModKey::new("kirara-shield-of-safe-transport");

const CB_KICK_PARTICLE: u16 = 1 << 0;
const CB_ROLL_PARTICLE: u16 = 1 << 1;
const CB_A1_STACK: u16 = 1 << 2;

/// generateSkillShield, on her queue.
const TASK_SKILL_SHIELD: u8 = 0;
/// A1's shields as the parcel ends, on her queue.
const TASK_A1: u8 = 1;
/// genShield's add a frame later, with the pending slot.
const TASK_ADD_SHIELD: u8 = 2;
/// An Urgent Neko Parcel tick (the fight's queue).
const TASK_PARCEL_TICK: u8 = 3;
const TASK_MINE_SNAPSHOT: u8 = 4;
const TASK_BOX: u8 = 5;
/// The cardamoms that hit, with their count.
const TASK_MINE_HITS: u8 = 6;
const TASK_MINE_EXPIRE: u8 = 7;

const FRAMES_HOLD: u8 = 0;

impl Character for Char {
    const KEY: &'static str = "kirara";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { parcel: None, mine_snap: None, cardamoms: 0, a1_stacks: 0, pending: [0.0; 4], next_pending: 0 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        if s.chars[ci].ascension >= 4 {
            s.chars[ci].mods.push(Mod::owned(A4_SKILL, ModKind::Attack, o, st::NONE, false, false, -1));
            s.chars[ci].mods.push(Mod::owned(A4_BURST, ModKind::Attack, o, st::NONE, false, false, -1));
        }
        if s.chars[ci].cons >= 4 {
            s.subscribe(EventKind::EnemyDamage, o);
        }
    }

    /// gcsim's validation: the skill takes hold and short_hold, the burst
    /// hits and mine_delay; nothing else is read.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let allowed: &[&str] = match a {
            Action::Attack | Action::Charge => &[],
            Action::Skill => &["hold", "short_hold"],
            Action::Burst => &["hits", "mine_delay"],
            _ => return Some(name.into()),
        };
        p.0.iter().find(|(k, _)| !allowed.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let marks = g::ATTACK_HITMARKS[n];
        let hb = g::ATTACK_HITBOXES[n];
        for (i, &h) in marks.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][i][lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = true;
            let oy = g::ATTACK_OFFSETS[n][i];
            ai.pattern = if n == 1 { s.box_on_player(0.0, oy, hb[0], hb[1]) } else { s.circle_on_player(0.0, oy, hb[0]) };
            s.queue_attack_at(ci, ai, 0, h);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *marks.last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        for (i, mult) in g::CHARGE.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = CHARGE_NAMES[i];
            if i == 2 {
                ai.hitlag_factor = 0.01;
                ai.hitlag_halt = 0.1 * 60.0;
                ai.def_halt = true;
            }
            ai.pattern = s.circle_on_player(0.0, g::CHARGE_OFFSETS[i], 2.8);
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, 0, h, h);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], *g::CHARGE_HITMARKS.last().unwrap(), AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        if p.get("short_hold").unwrap_or(0) != 0 {
            return skill_short_hold(s, ci);
        }
        let hold = p.get("hold").unwrap_or(0);
        if hold > 0 {
            return skill_hold(s, ci, hold.min(600) as i32);
        }
        skill_press(s, ci)
    }

    fn frames(_: u8, dur: i16, next: Action) -> i32 {
        g::SKILL_HOLD_FRAMES[next as usize] - 600 + dur as i32
    }

    /// Secret Art: Surprise Dispatch: the box, and the cardamoms that hit
    /// (`hits`, 2 by default) at `mine_delay` (180 by default); the rest
    /// explode when they expire.
    fn burst(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let mut cardamoms = 6;
        if s.chars[ci].cons >= 1 {
            // C1: one more per 8,000 max HP, up to 4
            cardamoms += ((s.max_hp(ci) / 8000.0) as i32).min(4);
        }
        let mut hits = p.get("hits").unwrap_or(2);
        let delay = p.get("mine_delay").unwrap_or(180) as i32;
        if hits > cardamoms as i64 {
            hits = cardamoms as i64;
        }
        let unsnapped = AttackEvent::new(mine_info(s, ci), 0);
        let st = s.st_mut::<State>(ci);
        st.cardamoms = cardamoms;
        if st.mine_snap.is_none() {
            // no snapshot yet: gcsim's zero Snapshot
            st.mine_snap = Some(unsnapped);
        }
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_MINE_SNAPSHOT, arg: 0 }, g::MINE_SNAPSHOT);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_BOX, arg: 0 }, g::BOX_HITMARK);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_MINE_HITS, arg: hits as i32 }, delay);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_MINE_EXPIRE, arg: 0 }, 80 + 12 * 60);
        if s.chars[ci].cons >= 6 {
            c6(s, ci);
        }
        s.set_cd(ci, Action::Burst, 15 * 60);
        s.consume_energy(ci, 7);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_SKILL_SHIELD => {
                let hp = shield_hp(s, ci);
                gen_shield(s, ci, hp);
                s.apply_self_infusion(AURA_DENDRO, 25.0, 6);
            }
            TASK_A1 => {
                let hp = shield_hp(s, ci) * 0.2 * s.st::<State>(ci).a1_stacks as f64;
                gen_shield(s, ci, hp);
            }
            TASK_ADD_SHIELD => {
                let hp = s.st::<State>(ci).pending[arg as usize];
                let f = s.f;
                s.add_shield(Shield {
                    owner: ci as u8,
                    key: SHIELD,
                    typ: ShieldType::KiraraSkill,
                    target: -1,
                    ele: Element::Dendro,
                    hp,
                    expiry: f + 12 * 60,
                    em_bonus: 0.0,
                });
            }
            TASK_PARCEL_TICK => {
                let Some(mut ev) = s.st::<State>(ci).parcel else { unreachable!() };
                // the parcel follows the player
                ev.info.pattern = s.circle_on_player(0.0, 0.0, 3.0);
                let mut cbs = CB_ROLL_PARTICLE;
                if s.chars[ci].ascension >= 1 {
                    cbs |= CB_A1_STACK;
                }
                s.queue_attack_with_snap(ev, cbs, 0);
            }
            TASK_MINE_SNAPSHOT => {
                let ev = s.snapshot_event(mine_info(s, ci), 0);
                s.st_mut::<State>(ci).mine_snap = Some(ev);
            }
            TASK_BOX => {
                s.add_status(ci, MINE_EXPIRED, 12 * 60, true);
                let lvl = s.talent(ci, 2);
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 50.0, g::BURST[lvl]);
                ai.abil = "Secret Art: Surprise Dispatch";
                // 3 ahead of the player as cast
                ai.pattern = s.circle_on_player(0.0, 3.0, 6.0);
                s.queue_attack(ai, 0, 0, 0);
            }
            TASK_MINE_HITS => {
                let Some(ev) = s.st::<State>(ci).mine_snap else { unreachable!() };
                for _ in 0..arg {
                    s.queue_attack_with_snap(ev, 0, 0);
                }
                let st = s.st_mut::<State>(ci);
                st.cardamoms = (st.cardamoms - arg).max(0);
            }
            TASK_MINE_EXPIRE => {
                let Some(ev) = s.st::<State>(ci).mine_snap else { unreachable!() };
                for i in 0..s.st::<State>(ci).cardamoms {
                    s.queue_attack_with_snap(ev, 0, i * 9 * 2);
                }
                s.st_mut::<State>(ci).cardamoms = 0;
            }
            _ => unreachable!(),
        }
    }

    /// The order gcsim appends them: particles, then A1.
    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_KICK_PARTICLE != 0 && !s.mod_active(ci, KICK_PARTICLE_ICD) {
            s.add_status(ci, KICK_PARTICLE_ICD, 18, true);
            s.queue_particle(3.0, Element::Dendro, s.particle_delay[ci]);
        }
        if cbs & CB_ROLL_PARTICLE != 0 && !s.mod_active(ci, ROLL_PARTICLE_ICD) {
            s.add_status(ci, ROLL_PARTICLE_ICD, 4 * 60, true);
            s.queue_particle(1.0, Element::Dendro, s.particle_delay[ci]);
        }
        if cbs & CB_A1_STACK != 0 {
            let active = s.mod_active(ci, A1_ICD);
            let st = s.st_mut::<State>(ci);
            if !active && st.a1_stacks < 3 {
                st.a1_stacks += 1;
                s.add_status(ci, A1_ICD, 30, true);
            }
        }
    }

    /// C6: every elemental and physical DMG bonus +12%.
    fn mod_amount(_: &Sim, _: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key != C6 {
            return false;
        }
        for k in [st::PYROP, st::HYDROP, st::CRYOP, st::ELECTROP, st::ANEMOP, st::GEOP, st::PHYP, st::DENDROP] {
            out[k] += 0.12;
        }
        true
    }

    /// A4: per 1,000 max HP, the skill's DMG +0.4% and the burst's +0.3%.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if m.key == A4_SKILL {
            if matches!(ev.info.tag, AttackTag::ElementalArt | AttackTag::ElementalArtHold) {
                ev.stats[st::DMGP] += s.max_hp(ci) * 0.001 * 0.004;
            }
        } else if m.key == A4_BURST && ev.info.tag == AttackTag::ElementalBurst {
            ev.stats[st::DMGP] += s.max_hp(ci) * 0.001 * 0.003;
        }
    }

    /// C4: a normal, charged or plunging attack's damage while her shield
    /// is up adds Steed of Skanda on the enemy, once every 3.8 s.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if s.mod_active(ci, C4_ICD) {
            return;
        }
        if !s.shields.iter().any(|x| x.typ == ShieldType::KiraraSkill) {
            return;
        }
        if !matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            return;
        }
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Dendro, 25.0, 2.0);
        ai.abil = "Steed of Skanda";
        ai.def_halt = true;
        ai.pattern = s.circle_on_target(2.0);
        s.queue_attack(ai, 0, 0, 0);
        s.add_status(ci, C4_ICD, 228, true);
    }
}

const CHARGE_NAMES: [&str; 3] = ["Charge 0", "Charge 1", "Charge 2"];

fn mine_info(s: &Sim, ci: usize) -> AttackInfo {
    let lvl = s.talent(ci, 2);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::CARDAMOM[lvl]);
    ai.abil = "Cat Grass Cardamom Explosion";
    ai.def_halt = true;
    ai.deployable = true;
    // gcsim's NewCircleHit from the player on the primary target
    ai.pattern = s.circle_on_target(2.0);
    ai
}

fn shield_hp(s: &Sim, ci: usize) -> f64 {
    let lvl = s.talent(ci, 1);
    g::SHIELD_PP[lvl] * s.max_hp(ci) + g::SHIELD_FLAT[lvl]
}

/// gcsim's genShield: the amount, plus what is left of her shield, up to
/// the maximum, as a shield added a frame later (the fight's queue).
fn gen_shield(s: &mut Sim, ci: usize, mut hp: f64) {
    if let Some(sh) = s.shields.iter().find(|x| x.typ == ShieldType::KiraraSkill) {
        hp += sh.hp;
    }
    let lvl = s.talent(ci, 1);
    hp = hp.min(g::MAX_SHIELD_PP[lvl] * s.max_hp(ci) + g::MAX_SHIELD_FLAT[lvl]);
    let st = s.st_mut::<State>(ci);
    let slot = st.next_pending;
    st.pending[slot as usize] = hp;
    st.next_pending = (slot + 1) % 4;
    s.add_task(Task::Char { c: ci as u8, id: TASK_ADD_SHIELD, arg: slot as i32 }, 1);
}

/// C6: the party's elemental and physical DMG +12% for 15 s.
fn c6(s: &mut Sim, ci: usize) {
    for i in 0..s.chars.len() {
        s.add_mod(i, Mod::owned(C6, ModKind::Stat, Owner::Char(ci as u8), st::NONE, false, true, -1), 15 * 60);
    }
}

fn skill_kick_ai(s: &Sim, ci: usize) -> AttackInfo {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::FLIPCLAW_DMG[lvl]);
    ai.abil = "Flipclaw Strike";
    ai.hitlag_halt = 0.1 * 60.0;
    ai.hitlag_factor = 0.01;
    ai.def_halt = true;
    ai.pattern = s.circle_on_player(0.0, 0.5, 2.6);
    ai
}

/// The Urgent Neko Parcel's hit, snapshot as the skill starts.
fn parcel_snapshot(s: &mut Sim, ci: usize) {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArtHold, icd::TAG_ELEMENTAL_ART_HOLD, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::CAT_DMG[lvl]);
    ai.abil = "Urgent Neko Parcel";
    let ev = s.snapshot_event(ai, 0);
    s.st_mut::<State>(ci).parcel = Some(ev);
}

fn skill_press(s: &mut Sim, ci: usize) -> ActInfo {
    let lvl = s.talent(ci, 1);
    let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Dendro, 25.0, g::SKILL_PRESS[lvl]);
    ai.abil = "Tail-Flicking Flying Kick";
    ai.hitlag_factor = 0.01;
    ai.def_halt = true;
    ai.pattern = s.fan_on_player(0.0, 0.5, 2.5, 270.0);
    let h = g::SKILL_PRESS_HITMARK;
    s.queue_attack(ai, CB_KICK_PARTICLE, h, h);
    if s.chars[ci].cons >= 6 {
        c6(s, ci);
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_SHIELD, arg: 0 }, h);
    s.set_cd_with_delay(ci, Action::Skill, 8 * 60, 12);
    let t = &g::SKILL_PRESS_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
}

fn skill_short_hold(s: &mut Sim, ci: usize) -> ActInfo {
    // one tick
    parcel_snapshot(s, ci);
    s.add_task(Task::Char { c: ci as u8, id: TASK_PARCEL_TICK, arg: 0 }, 17);
    let ai = skill_kick_ai(s, ci);
    let ev = s.snapshot_event(ai, 0);
    s.queue_attack_with_snap(ev, CB_KICK_PARTICLE, g::SKILL_SHORT_HOLD_KICK_HITMARK);
    if s.chars[ci].ascension >= 1 {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, g::SKILL_SHORT_HOLD_CD_START - 1);
    }
    if s.chars[ci].cons >= 6 {
        c6(s, ci);
    }
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_SHIELD, arg: 0 }, 14);
    s.set_cd_with_delay(ci, Action::Skill, 492, g::SKILL_SHORT_HOLD_CD_START);
    let t = &g::SKILL_SHORT_HOLD_FRAMES;
    ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Skill)
}

fn skill_hold(s: &mut Sim, ci: usize, dur: i32) -> ActInfo {
    // a tick every 30 frames from 16 to the end
    parcel_snapshot(s, ci);
    let mut i = 16;
    while i <= dur + 12 {
        s.add_task(Task::Char { c: ci as u8, id: TASK_PARCEL_TICK, arg: 0 }, i);
        i += 30;
    }
    let ai = skill_kick_ai(s, ci);
    let ev = s.snapshot_event(ai, 0);
    s.queue_attack_with_snap(ev, CB_KICK_PARTICLE, g::SKILL_HOLD_KICK_HITMARK - 600 + dur);
    let end = g::SKILL_HOLD_CD_START - 600 + dur;
    if s.chars[ci].ascension >= 1 {
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_A1, arg: 0 }, end);
    }
    if s.chars[ci].cons >= 6 {
        c6(s, ci);
    }
    let cd = 8 * 60 + dur / 30 * 12;
    s.queue_char_task(ci, Task::Char { c: ci as u8, id: TASK_SKILL_SHIELD, arg: 0 }, 14);
    s.set_cd_with_delay(ci, Action::Skill, cd, end);
    let t = &g::SKILL_HOLD_FRAMES;
    ActInfo::new(
        Frames::Char { id: FRAMES_HOLD, arg: dur as i16 },
        t[0] - 600 + dur,
        t[Action::Attack as usize] - 600 + dur,
        AnimState::Skill,
    )
}
