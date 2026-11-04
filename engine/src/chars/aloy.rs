//! Aloy, from gcsim's internal/characters/aloy: normal attacks, aimed
//! shots (physical and fully charged), Frozen Wilds (the Freeze Bomb, its
//! Chillwater Bomblets, the Coil stacks and Rushing Ice), Prophecies of
//! Dawn (the burst), A1 and A4. gcsim gives her no constellations.

use crate::content::{Character, Event};
use crate::gcsimdata::chars::aloy as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

pub struct State {
    coil_icd_expiry: i32,
    last_field_exit: i32,
    coils: i32,
    /// A4's stacks, and the frame of the Rushing Ice they belong to.
    a4_stacks: i32,
    a4_src: i32,
}

const RUSHING_ICE: ModKey = ModKey::new(g::RUSHING_ICE_KEY);
const RUSHING_ICE_MOD: ModKey = ModKey::new("aloy-rushing-ice");
const COIL_MOD: ModKey = ModKey::new("aloy-coil-stacks");
const A1: ModKey = ModKey::new("aloy-a1");
const A4: ModKey = ModKey::new("aloy-strong-strike");

const CB_PARTICLE: u16 = 1 << 0;
const CB_COIL: u16 = 1 << 1;

/// The Coils' end 30 s after she left the field, with that frame.
const TASK_COIL_CLEAR: u8 = 0;
/// A4's stack every second, with its Rushing Ice's frame.
const TASK_A4: u8 = 1;

const NORMAL_NAMES: [&str; 4] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3"];

impl Character for Char {
    const KEY: &'static str = "aloy";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State { coil_icd_expiry: 0, last_field_exit: 0, coils: 0, a4_stacks: 0, a4_src: -1 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let o = Owner::Char(ci as u8);
        // coilMod
        s.add_mod(ci, Mod::owned(COIL_MOD, ModKind::Attack, o, st::NONE, false, false, -1), -1);
        // onExitField
        s.subscribe(EventKind::Swap, o);
    }

    fn unsupported(a: Action, p: &Params) -> Option<String> {
        let name = crate::gcsl::action_name(a);
        let allowed: &[&str] = match a {
            Action::Attack => &["travel"],
            Action::Aim => &["hold", "travel", "weakspot"],
            Action::Skill => &["travel", "bomblets", "bomb_delay"],
            Action::Burst => &[],
            _ => return Some(name.into()),
        };
        p.0.iter().find(|(k, _)| !allowed.contains(&k.as_str())).map(|(k, _)| format!("{name} param {k}"))
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
        let travel = p.get("travel").unwrap_or(10) as i32;
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let icd_tag = if s.mod_active(ci, RUSHING_ICE) { icd::TAG_NORMAL_ATTACK } else { icd::TAG_NONE };
        let marks = g::ATTACK_HITMARKS[n];
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd_tag, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.pattern = s.box_on_target(0.0, 0.0, 0.1, 1.0);
            s.queue_attack(ai, 0, marks[i], marks[i] + travel);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *marks.last().unwrap(), AnimState::Normal);
        s.advance_normal(ci);
        info
    }

    fn aim(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let hold = p.get("hold").unwrap_or(1) as usize;
        let travel = p.get("travel").unwrap_or(10) as i32;
        let weakspot = p.get("weakspot") == Some(1);
        let lvl = s.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::FULLAIM[lvl]);
        ai.abil = "Fully-Charged Aimed Shot";
        ai.deployable = true;
        // the hitlag on weak spot hits only (HitlagOnHeadshotOnly)
        if weakspot {
            ai.weak_point = true;
            ai.hitlag_halt = 0.12 * 60.0;
            ai.hitlag_factor = 0.01;
        }
        if hold < 1 {
            ai.abil = "Aimed Shot";
            ai.element = Element::Physical;
            ai.mult = g::AIM[lvl];
        }
        ai.pattern = s.box_on_target(0.0, -0.5, 0.1, 1.0);
        let h = g::AIMED_HITMARKS[hold];
        s.queue_attack(ai, 0, h, h + travel);
        let t = &g::AIMED_FRAMES[hold];
        ActInfo::new(Frames::Table(t), t[0], h, AnimState::Aim)
    }

    /// Frozen Wilds: the Freeze Bomb and its bomblets, which give Coils.
    fn skill(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        let travel = p.get("travel").unwrap_or(5) as i32;
        let bomblets = p.get("bomblets").unwrap_or(2) as i32;
        let delay = p.get("bomb_delay").unwrap_or(0) as i32;
        let lvl = s.talent(ci, 1);
        let rel = g::SKILL_RELEASE;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL_MAIN[lvl]);
        ai.abil = "Freeze Bomb";
        ai.pattern = s.circle_on_target(4.0);
        s.queue_attack(ai, CB_PARTICLE, rel, rel + travel);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Cryo, 25.0, g::SKILL_BOMBLETS[lvl]);
        ai.abil = "Chillwater Bomblets";
        ai.def_halt = true;
        ai.deployable = true;
        ai.pattern = s.circle_on_target(2.0);
        for i in 0..bomblets {
            s.queue_attack(ai, CB_COIL, rel + travel, rel + travel + delay + (i + 1) * 6);
        }
        s.set_cd_with_delay(ci, Action::Skill, 20 * 60, 19);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], rel, AnimState::Skill)
    }

    /// Prophecies of Dawn, snapshot as it is cast.
    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Cryo, 50.0, g::BURST[lvl]);
        ai.abil = "Prophecies of Dawn";
        ai.pattern = s.circle_on_player(0.0, 6.5, 6.5);
        let ev = s.snapshot_event(ai, 0);
        s.queue_attack_with_snap(ev, 0, g::BURST_HITMARK);
        s.set_cd(ci, Action::Burst, 12 * 60);
        s.consume_energy(ci, 2);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Jump as usize], AnimState::Burst)
    }

    fn task(s: &mut Sim, ci: usize, id: u8, arg: i32) {
        match id {
            TASK_COIL_CLEAR => {
                if s.st::<State>(ci).last_field_exit == arg {
                    s.st_mut::<State>(ci).coils = 0;
                }
            }
            TASK_A4 => {
                let st = s.st_mut::<State>(ci);
                if st.a4_src == arg {
                    st.a4_stacks += 1;
                }
            }
            _ => unreachable!(),
        }
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        // the Freeze Bomb hits once
        if cbs & CB_PARTICLE != 0 {
            s.queue_particle(5.0, Element::Cryo, s.particle_delay[ci]);
        }
        if cbs & CB_COIL != 0 {
            coil_stacks(s, ci);
        }
    }

    fn mod_amount(s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key == A1 {
            out[st::ATKP] += if on == ci { 0.16 } else { 0.08 };
            true
        } else if m.key == A4 {
            out[st::CRYOP] += s.st::<State>(ci).a4_stacks.min(10) as f64 * 0.035;
            true
        } else {
            false
        }
    }

    /// The Coils' and Rushing Ice's normal attack DMG.
    fn attack_mod(s: &Sim, ci: usize, _: usize, m: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag != AttackTag::Normal {
            return;
        }
        let lvl = s.talent(ci, 1);
        if m.key == COIL_MOD {
            let coils = s.st::<State>(ci).coils;
            if coils > 0 {
                ev.stats[st::DMGP] += g::SKILL_COIL_NA_BONUS[coils as usize - 1][lvl];
            }
        } else if m.key == RUSHING_ICE_MOD {
            ev.stats[st::DMGP] += g::SKILL_RUSHING_ICE_NA_BONUS[lvl];
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        if let Event::Swap { prev, .. } = *e {
            if prev != ci {
                return;
            }
            let f = s.f;
            s.st_mut::<State>(ci).last_field_exit = f;
            s.add_task(Task::Char { c: ci as u8, id: TASK_COIL_CLEAR, arg: f }, 30 * 60);
        }
    }

    fn condition(s: &Sim, ci: usize, fields: &[String]) -> Option<Result<crate::gcsl::Num, String>> {
        (fields[0] == "coil").then(|| Ok(crate::gcsl::Num::Int(s.st::<State>(ci).coils as i64)))
    }
}

/// A Coil, once every 6 frames and not in Rushing Ice; the fourth starts
/// Rushing Ice.
fn coil_stacks(s: &mut Sim, ci: usize) {
    if s.st::<State>(ci).coil_icd_expiry > s.f || s.mod_active(ci, RUSHING_ICE) {
        return;
    }
    let f = s.f;
    let st = s.st_mut::<State>(ci);
    st.coils += 1;
    st.coil_icd_expiry = f + 6;
    // A1: ATK +16% for her, +8% for the others, for 10 s
    if s.chars[ci].ascension >= 1 {
        for i in 0..s.chars.len() {
            s.add_mod(i, Mod::owned(A1, ModKind::Stat, Owner::Char(ci as u8), st::ATKP, false, true, -1), g::RUSHING_ICE_DURATION);
        }
    }
    if s.st::<State>(ci).coils == 4 {
        s.st_mut::<State>(ci).coils = 0;
        rushing_ice(s, ci);
    }
}

fn rushing_ice(s: &mut Sim, ci: usize) {
    let d = g::RUSHING_ICE_DURATION;
    s.add_status(ci, RUSHING_ICE, d, true);
    s.add_infusion(ci, RUSHING_ICE_MOD, Element::Cryo, 600, true, &[AttackTag::Normal]);
    s.add_mod(ci, Mod::owned(RUSHING_ICE_MOD, ModKind::Attack, Owner::Char(ci as u8), st::NONE, false, true, -1), d);
    // A4: cryo DMG +3.5% a second, up to 35%
    if s.chars[ci].ascension >= 4 {
        let f = s.f;
        let st = s.st_mut::<State>(ci);
        st.a4_stacks = 1;
        st.a4_src = f;
        s.add_mod(ci, Mod::owned(A4, ModKind::Stat, Owner::Char(ci as u8), st::CRYOP, false, true, -1), d);
        for i in 0..10 {
            s.add_task(Task::Char { c: ci as u8, id: TASK_A4, arg: f }, 60 * (1 + i));
        }
    }
}
