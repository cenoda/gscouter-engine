//! Athame Artis, from gcsim's internal/weapons/sword/athameartis.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
#[derive(Default)]
pub struct State {
    /// gcsim's src: the frame the refresh loop now running started.
    src: i32,
    /// The holder's and the party's ATK%, fixed at Init (gcsim's selfVal
    /// and teamBuff).
    self_atk: f64,
    team_atk: f64,
}

const BURST_CD: ModKey = ModKey::new("athame-burst-cd");
const SELF: ModKey = ModKey::new("blade-of-the-daylight-hours-buff");
/// One key for every holder's buff: one already on the active character
/// from another Athame Artis is extended, keeping its amount.
const TEAM: ModKey = ModKey::new("athameartis-team-buff");
const BUFF_DUR: i32 = 3 * 60;

const TASK_REFRESH: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "athameartis";
    type State = State;
    fn new_state() -> State {
        State::default()
    }

    fn init(_: &mut Sim, _: usize) {}

    /// gcsim's Weapon.Init: burst CRIT DMG +16 to 32%; the ATK buffs' amounts
    /// (×1.75 with two Hexerei characters in the team).
    fn post_init(s: &mut Sim, ci: usize) {
        let o = Owner::Weapon(ci as u8);
        s.chars[ci].mods.push(Mod::owned(BURST_CD, ModKind::Attack, o, st::NONE, false, false, -1));
        let r = s.chars[ci].refine as f64;
        let k = if s.hexerei_count() >= 2 { 1.75 } else { 1.0 };
        let st = s.weapon_st_mut::<State>(ci);
        st.self_atk = (0.15 + 0.05 * r) * k;
        st.team_atk = (0.12 + 0.04 * r) * k;
        s.subscribe(EventKind::EnemyHit, o);
    }

    fn attack_mod(s: &Sim, ci: usize, _: usize, _: &Mod, ev: &mut AttackEvent) {
        if ev.info.tag == AttackTag::ElementalBurst {
            ev.stats[st::CD] += 0.12 + 0.04 * s.chars[ci].refine as f64;
        }
    }

    fn task(s: &mut Sim, ci: usize, _: u8, src: i32) {
        refresh(s, ci, src);
    }

    /// A burst hit by the holder on the enemy: its own ATK buff for 3 s
    /// (no hitlag); when that was not running, the party's refresh loop
    /// starts now.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyHit(ev) = *e else { return };
        if ev.info.actor as usize != ci || ev.info.tag != AttackTag::ElementalBurst {
            return;
        }
        let running = s.mod_active(ci, SELF);
        let v = s.weapon_st::<State>(ci).self_atk;
        s.add_mod(ci, Mod::stat(SELF, st::ATKP, v, false, false, -1), BUFF_DUR);
        if !running {
            let src = s.f;
            s.weapon_st_mut::<State>(ci).src = src;
            refresh(s, ci, src);
        }
    }
}

/// gcsim's refreshTeamBuff: while the holder's buff lasts, every second
/// (hitlag) the active character other than the holder gets the party's
/// ATK buff for 3 s (2 s with two Hexerei characters; hitlag), or has its
/// running one extended to that.
fn refresh(s: &mut Sim, ci: usize, src: i32) {
    if s.weapon_st::<State>(ci).src != src || !s.mod_active(ci, SELF) {
        return;
    }
    s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_REFRESH, arg: src }, 60);
    let active = s.player.active as usize;
    if active == ci {
        return;
    }
    let duration = if s.hexerei_count() >= 2 { 2 * 60 } else { BUFF_DUR };
    if s.mod_active(active, TEAM) {
        let dur = s.mod_duration(active, TEAM.name);
        s.extend_mod(active, TEAM, duration - dur);
    } else {
        let v = s.weapon_st::<State>(ci).team_atk;
        s.add_mod(active, Mod::stat(TEAM, st::ATKP, v, false, true, -1), duration);
    }
}
