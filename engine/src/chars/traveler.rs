//! The sword normal and charged attacks that the anemo, geo and dendro
//! travelers' packages repeat with the same tables (the story-quest buffs
//! are `traveler_common`'s). Aether is gcsim's gender 0, Lumine 1; an element's module is
//! generic over it (`G`), and the two keys are its two instances
//! (`aethergeo.rs`, `luminegeo.rs` and the others).

use crate::sim::*;
use crate::types::*;

/// The normal and charged attack tables, the same in every element's
/// package.
pub struct Normals {
    pub attack: &'static [&'static [f64]],
    pub frames: &'static [&'static [FrameTable]],
    pub hitboxes: &'static [&'static [&'static [f64]]],
    pub halt: &'static [&'static [f64]],
    pub hitmarks: &'static [&'static [i32]],
    pub offsets: &'static [&'static [f64]],
    pub fans: &'static [&'static [f64]],
    pub charge: &'static [&'static [&'static [f64]]],
    pub charge_frames: &'static [FrameTable],
    pub charge_hitmarks: &'static [&'static [i32]],
}

#[macro_export]
macro_rules! traveler_normals {
    ($g:ident) => {
        $crate::chars::traveler::Normals {
            attack: $g::ATTACK,
            frames: $g::ATTACK_FRAMES,
            hitboxes: $g::ATTACK_HITBOXES,
            halt: $g::ATTACK_HITLAG_HALT_FRAME,
            hitmarks: $g::ATTACK_HITMARKS,
            offsets: $g::ATTACK_OFFSETS,
            fans: $g::ATTACK_FAN_ANGLES,
            charge: $g::CHARGE,
            charge_frames: $g::CHARGE_FRAMES,
            charge_hitmarks: $g::CHARGE_HITMARKS,
        }
    };
}

/// gcsim's Attack, up to the element's own part (geo's A4, anemo's A1),
/// which `extra` runs after the hit is queued and before the counter
/// advances.
pub fn attack(s: &mut Sim, ci: usize, gender: usize, t: &Normals, extra: impl FnOnce(&mut Sim)) -> ActInfo {
    let n = s.chars[ci].normal_counter as usize;
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, t.attack[n][lvl]);
    ai.abil = NORMAL_NAMES[n];
    ai.hitlag_factor = 0.01;
    ai.hitlag_halt = t.halt[gender][n] * 60.0;
    ai.def_halt = true;
    let off = t.offsets[gender][n];
    let hb = t.hitboxes[gender][n];
    ai.pattern = if (gender == 0 && (n == 0 || n == 2)) || (gender == 1 && n == 1) {
        s.box_on_player(0.0, off, hb[0], hb[1])
    } else {
        s.fan_on_player(0.0, off, hb[0], t.fans[gender][n])
    };
    let h = t.hitmarks[gender][n];
    s.queue_attack(ai, 0, h, h);
    extra(s);
    let atkspd = s.stat(ci, st::ATKSPD);
    let table = &t.frames[gender][n];
    let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], h, AnimState::Normal);
    s.advance_normal(ci);
    info
}

const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];

/// gcsim's ChargeAttack: two hits in a circle of 2.2 around the player.
pub fn charge(s: &mut Sim, ci: usize, gender: usize, t: &Normals) -> ActInfo {
    let lvl = s.talent(ci, 0);
    let hm = t.charge_hitmarks[gender];
    for (i, mult) in t.charge[gender].iter().enumerate() {
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
        ai.abil = CHARGE_NAMES[i];
        ai.pattern = s.circle_on_player(0.0, 0.0, 2.2);
        s.queue_attack(ai, 0, hm[i], hm[i]);
    }
    let table = &t.charge_frames[gender];
    ActInfo::new(Frames::Table(table), table[0], hm[hm.len() - 1], AnimState::Charge)
}

/// gcsim's AnimationStartDelay for AnimationXingqiuN0StartDelay is in the
/// generated data (8 for Aether, 7 for Lumine); nothing to do here.
pub fn unsupported(a: Action) -> Option<String> {
    match a {
        Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
        _ => Some(crate::gcsl::action_name(a).into()),
    }
}
