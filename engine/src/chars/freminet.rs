//! Freminet, from gcsim's internal/characters/freminet: normal attacks and
//! their Pers Time frost (which stacks Pressure, and at four stacks turns
//! the next normal attack into the detonation), Pressurized Floe (the
//! thrust, the Spiritbreath Thorn and the Shattering Pressure by its
//! stacks, usable whatever the cooldown during Pers Time), Shadowhunter's
//! Ambush (its hit, its doubled stacks, its skill cooldown, its end when he
//! leaves the field), A1, A4, the airborne plunges and every constellation.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gamedata::chars::freminet as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {
    skill_stacks: i32,
    c4_stacks: i32,
    c6_stacks: i32,
}

const PERS_TIME: ModKey = ModKey::new(g::PERS_TIME_KEY);
const BURST: ModKey = ModKey::new(g::BURST_KEY);
const ALIGNED_ICD: ModKey = ModKey::new(g::SKILL_ALIGNED_ICD_KEY);
const PARTICLE_ICD_THRUST: ModKey = ModKey::new(g::PARTICLE_ICD_KEY_THRUST);
const PARTICLE_ICD_LV4: ModKey = ModKey::new(g::PARTICLE_ICD_KEY_LV4);
const A4: ModKey = ModKey::new(g::A4_KEY);
const C1: ModKey = ModKey::new(g::C1_KEY);
const C4: ModKey = ModKey::new(g::C4_KEY);
const C6: ModKey = ModKey::new(g::C6_KEY);
const C4C6_ICD: ModKey = ModKey::new(g::C4C6_ICD_KEY);

const CB_THRUST_PARTICLE: u16 = 1 << 0;
const CB_LV4_PARTICLE: u16 = 1 << 1;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];
const CRYO_NAMES: [&str; 4] = [
    "Pressurized Floe: Shattering Pressure (Cryo Lvl 0)",
    "Pressurized Floe: Shattering Pressure (Cryo Lvl 1)",
    "Pressurized Floe: Shattering Pressure (Cryo Lvl 2)",
    "Pressurized Floe: Shattering Pressure (Cryo Lvl 3)",
];
const PHYS_NAMES: [&str; 4] = [
    "Pressurized Floe: Shattering Pressure (Physical Lvl 1)",
    "Pressurized Floe: Shattering Pressure (Physical Lvl 2)",
    "Pressurized Floe: Shattering Pressure (Physical Lvl 3)",
    "Pressurized Floe: Shattering Pressure (Physical Lvl 4)",
];

const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

impl Character for Char {
    const KEY: &'static str = "freminet";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // onExitField
        s.subscribe(EventKind::Swap, o);
        // a4 and c4c6: shatter, frozen, superconduct and Stellar-Conduct
        if s.chars[ci].ascension >= 4 || s.chars[ci].cons >= 4 {
            s.subscribe(EventKind::Reaction, o);
        }
        if s.chars[ci].cons >= 1 {
            s.chars[ci].mods.push(Mod::owned(C1, ModKind::Attack, o, st::NONE, false, false, -1));
        }
    }

    /// gcsim validates collision on the plunges and reads no params of the
    /// other actions; he has no charged attack.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        match a {
            Action::Attack | Action::Skill | Action::Burst => None,
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            _ => Some(name.into()),
        }
    }

    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    /// The skill during Pers Time, whatever its cooldown.
    fn action_ready(s: &Sim, ci: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && s.mod_active(ci, PERS_TIME)).then_some(None)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.st::<State>(ci).skill_stacks >= 4 {
            s.chars[ci].normal_counter = 0;
            return detonate(s, ci);
        }
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::ATTACK[n][lvl]);
        ai.abil = NORMAL_NAMES[n];
        ai.blunt = true;
        ai.poise = g::ATTACK_POISE_DMG[n];
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n] * 60.0;
        ai.def_halt = true;
        let hb = g::ATTACK_HITBOXES[n];
        let oy = g::ATTACK_OFFSETS[n];
        let pattern = if n == 2 { s.box_on_player(0.0, oy, hb[0], hb[1]) } else { s.circle_on_player(0.0, oy, hb[0]) };
        ai.pattern = pattern;
        let h = g::ATTACK_HITMARKS[n];
        s.queue_attack(ai, 0, h, h);
        if s.mod_active(ci, PERS_TIME) {
            let burst = s.mod_active(ci, BURST);
            let mut mult = g::SKILL_ADD_NA[s.talent(ci, 1)];
            if burst {
                mult *= 2.0;
            }
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, mult);
            ai.abil = "Pressurized Floe: Pers Time Frost";
            ai.blunt = true;
            ai.poise = 25.0;
            ai.pattern = pattern;
            let d = h + g::ATTACK_FROST_DELAY[n];
            s.queue_attack(ai, 0, d, d);
            let st = s.st_mut::<State>(ci);
            st.skill_stacks = (st.skill_stacks + if burst { 2 } else { 1 }).min(4);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        if s.mod_active(ci, PERS_TIME) {
            return detonate(s, ci);
        }
        s.st_mut::<State>(ci).skill_stacks = 0;
        s.add_status(ci, PERS_TIME, 10 * 60, true);
        let lvl = s.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL_THRUST[lvl]);
        ai.abil = "Pressurized Floe: Upward Thrust";
        ai.blunt = true;
        ai.poise = 75.0;
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = 0.08 * 60.0;
        ai.pattern = s.circle_on_player(0.0, 2.0, 2.5);
        s.queue_attack(ai, CB_THRUST_PARTICLE, 0, g::SKILL_THRUST_HITMARK);
        // skillAligned
        if !s.mod_active(ci, ALIGNED_ICD) {
            s.add_status(ci, ALIGNED_ICD, g::SKILL_ALIGNED_ICD, true);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 0.0, g::SKILL_BREATH[lvl]);
            ai.abil = "Pressurized Floe: Spiritbreath Thorn";
            ai.pattern = s.circle_on_player(0.0, 2.0, 2.5);
            s.queue_attack(ai, 0, 62, 62);
        }
        let cd = if s.mod_active(ci, BURST) { 3 * 60 } else { 600 };
        s.set_cd_with_delay(ci, Action::Skill, cd, 35);
        let t = &g::SKILL_THRUST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        s.add_status(ci, BURST, 10 * 60, true);
        s.reset_cd(ci, Action::Skill);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::BURST[s.talent(ci, 2)]);
        ai.abil = "Shadowhunter's Ambush";
        ai.blunt = true;
        ai.pattern = s.circle_on_player(0.0, 0.0, 5.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        s.set_cd(ci, Action::Burst, 60 * 15);
        s.consume_energy(ci, 4);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        let (bit, icd) = if cbs & CB_THRUST_PARTICLE != 0 {
            (true, PARTICLE_ICD_THRUST)
        } else if cbs & CB_LV4_PARTICLE != 0 {
            (true, PARTICLE_ICD_LV4)
        } else {
            (false, PARTICLE_ICD_LV4)
        };
        if !bit || s.mod_active(ci, icd) {
            return;
        }
        s.add_status(ci, icd, 18, true);
        let n = if s.mod_active(ci, BURST) { 1.0 } else { 2.0 };
        s.queue_particle(n, Element::Cryo, s.particle_delay[ci]);
    }

    fn mod_amount(s: &Sim, ci: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == C4 {
            out[st::ATKP] += s.st::<State>(ci).c4_stacks as f64 * 0.09;
            true
        } else if m.key == C6 {
            out[st::CD] += s.st::<State>(ci).c6_stacks as f64 * 0.12;
            true
        } else {
            false
        }
    }

    /// A4: Shattering Pressure DMG +40% after a shatter; C1: its CRIT Rate
    /// +15%.
    fn attack_mod(_: &Sim, _: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if !ev.info.abil.starts_with(g::PRESSURE_BASE_NAME) {
            return;
        }
        if m.key == A4 {
            ev.stats[st::DMGP] += 0.4;
        } else if m.key == C1 {
            ev.stats[st::CR] += 0.15;
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        match *e {
            // onExitField: his burst ends when he leaves the field
            Event::Swap { prev, .. } => {
                if prev == ci && s.mod_active(ci, BURST) {
                    s.delete_mod(ci, BURST);
                }
            }
            Event::Reaction(r, ev) => {
                if ev.info.actor as usize != ci {
                    return;
                }
                if r == Reaction::Shatter && s.chars[ci].ascension >= 4 {
                    s.add_mod(ci, Mod::owned(A4, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), 5 * 60);
                }
                if matches!(r, Reaction::Shatter | Reaction::Frozen | Reaction::Superconduct | Reaction::StellarConduct) {
                    c4c6(s, ci);
                }
            }
            _ => {}
        }
    }
}

/// detonateSkill: Shattering Pressure by its stacks, A1 and C2.
fn detonate(s: &mut Sim, ci: usize) -> ActInfo {
    let stacks = s.st::<State>(ci).skill_stacks;
    let idx = if stacks == 4 { 1 } else { 0 };
    let hitmark = g::SKILL_PRESSURE_HITMARKS[idx];
    let lvl = s.talent(ci, 1);
    if stacks <= 3 {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL_PRESSURE_CRYO[stacks as usize][lvl]);
        ai.abil = CRYO_NAMES[stacks as usize];
        ai.blunt = true;
        ai.poise = if stacks > 0 { 70.0 } else { 150.0 };
        ai.hitlag_factor = 0.01;
        ai.hitlag_halt = 0.10 * 60.0;
        ai.pattern = s.circle_on_player(0.0, 2.0, 2.5);
        s.queue_attack(ai, 0, 0, hitmark);
    }
    if stacks >= 1 {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, g::SKILL_PRESSURE_PHYS[stacks as usize - 1][lvl]);
        ai.abil = PHYS_NAMES[stacks as usize - 1];
        ai.blunt = true;
        ai.poise = if stacks < 4 { 70.0 } else { 150.0 };
        let mut cb = 0;
        if stacks == 4 {
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = 0.09 * 60.0;
            ai.pattern = s.box_on_player(0.5, 0.5, 3.0, 7.0);
            cb = CB_LV4_PARTICLE;
        } else {
            ai.pattern = s.circle_on_player(0.0, 2.0, 2.5);
        }
        s.queue_attack(ai, cb, 0, hitmark);
    }
    // a1
    if s.chars[ci].ascension >= 1 && stacks != 4 {
        s.reduce_cd(ci, Action::Skill, 60);
    }
    // c2
    if s.chars[ci].cons >= 2 {
        s.add_energy(ci, if stacks == 4 { 3.0 } else { 2.0 });
    }
    s.delete_mod(ci, PERS_TIME);
    s.st_mut::<State>(ci).skill_stacks = 0;
    let t = &g::SKILL_PRESSURE_FRAMES[idx];
    ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Skill)
}

/// c4c6Buff: ATK +9% a stack (2) and, at C6, CRIT DMG +12% a stack (3), once
/// every 0.3 s.
fn c4c6(s: &mut Sim, ci: usize) {
    if s.chars[ci].cons < 4 || s.mod_active(ci, C4C6_ICD) {
        return;
    }
    s.add_status(ci, C4C6_ICD, 18, true);
    if !s.mod_active(ci, C4) {
        s.st_mut::<State>(ci).c4_stacks = 0;
    }
    let st = s.st_mut::<State>(ci);
    st.c4_stacks = (st.c4_stacks + 1).min(2);
    s.add_mod(ci, Mod::owned(C4, ModKind::Stat, Owner::Char(ci as u8), st::ATKP, false, true, -1), 6 * 60);
    if s.chars[ci].cons < 6 {
        return;
    }
    if !s.mod_active(ci, C6) {
        s.st_mut::<State>(ci).c6_stacks = 0;
    }
    let st = s.st_mut::<State>(ci);
    st.c6_stacks = (st.c6_stacks + 1).min(3);
    s.add_mod(ci, Mod::owned(C6, ModKind::Stat, Owner::Char(ci as u8), st::CD, false, true, -1), 6 * 60);
}
