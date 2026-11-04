//! Golden Frostbound Oath, from gcsim's internal/weapons/bow/goldenfrostboundoath.

use crate::content::{Event, WeaponImpl};
use crate::sim::*;
use crate::types::*;

pub struct Weapon;
pub struct State {
    team_buff_src: i32,
}

const DEF_BUFF: ModKey = ModKey::new("golden-frostbound-oath-def");
const GEO_BUFF: ModKey = ModKey::new("golden-frostbound-oath-geo");
const LCR_BUFF: ModKey = ModKey::new("golden-frostbound-oath-lcr");
const TEAM_GEO_BUFF: ModKey = ModKey::new("golden-frostbound-oath-team-geo");
const TEAM_LCR_BUFF: ModKey = ModKey::new("golden-frostbound-oath-team-lcr");

const TASK_TICK: u8 = 0;

impl WeaponImpl for Weapon {
    const KEY: &'static str = "goldenfrostboundoath";
    type State = State;
    fn new_state() -> State {
        State { team_buff_src: -1 }
    }

    fn init(s: &mut Sim, ci: usize) {
        let r = s.chars[ci].refine as f64;
        s.chars[ci].mods.push(Mod::stat(DEF_BUFF, st::DEFP, 0.12 + 0.04 * r, false, false, -1));
        s.subscribe(EventKind::EnemyDamage, Owner::Weapon(ci as u8));
    }

    fn react_bonus(s: &Sim, ci: usize, _: usize, m: &Mod, info: &AttackInfo) -> f64 {
        let lcr = matches!(info.tag, AttackTag::DirectLunarCrystallize | AttackTag::ReactionLunarCrystallize);
        if !lcr {
            return 0.0;
        }
        let r = s.chars[ci].refine as f64;
        if m.key == LCR_BUFF {
            0.3 + 0.1 * r
        } else if m.key == TEAM_LCR_BUFF {
            0.15 + 0.05 * r
        } else {
            0.0
        }
    }

    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if ev.info.actor as usize != ci {
            return;
        }
        let trigger = matches!(
            ev.info.tag,
            AttackTag::ElementalArt | AttackTag::DirectLunarCrystallize | AttackTag::ReactionLunarCrystallize
        );
        if !trigger {
            return;
        }
        let o = Owner::Weapon(ci as u8);
        let r = s.chars[ci].refine as f64;
        s.add_mod(ci, Mod::stat(GEO_BUFF, st::GEOP, 0.3 + 0.1 * r, false, true, -1), 6 * 60);
        s.add_mod(ci, Mod::owned(LCR_BUFF, ModKind::ReactBonus, o, st::NONE, false, true, -1), 6 * 60);

        let f = s.f;
        s.weapon_st_mut::<State>(ci).team_buff_src = f;
        Self::team_buff_ticker(s, ci, f);
    }

    fn task(s: &mut Sim, ci: usize, id: u8, src: i32) {
        if id == TASK_TICK {
            Self::team_buff_ticker(s, ci, src);
        }
    }
}

impl Weapon {
    fn team_buff_ticker(s: &mut Sim, ci: usize, src: i32) {
        if s.weapon_st::<State>(ci).team_buff_src != src || !s.mod_active(ci, GEO_BUFF) {
            return;
        }
        s.queue_char_task(ci, Task::Weapon { c: ci as u8, id: TASK_TICK, arg: src }, 60);

        if s.moondrifts_near(16.0) == 0 {
            return;
        }

        let r = s.chars[ci].refine as f64;
        let o = Owner::Weapon(ci as u8);
        for j in 0..s.chars.len() {
            if j == ci {
                continue;
            }
            s.add_mod(j, Mod::stat(TEAM_GEO_BUFF, st::GEOP, 0.15 + 0.05 * r, false, true, -1), 2 * 60);
            s.add_mod(j, Mod::owned(TEAM_LCR_BUFF, ModKind::ReactBonus, o, st::NONE, false, true, -1), 2 * 60);
        }
    }
}
