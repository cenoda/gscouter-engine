//! The plunges gcsim's character packages share: the low and high plunges
//! from Xianyun's airborne state (their lowPlungeXY and highPlungeXY) and
//! the falling collision hit before them, written once from each
//! character's tables.

use crate::sim::*;
use crate::types::*;

/// A character's airborne plunges, as its gcsim plunge.go defines them.
pub struct Spec {
    pub low_hitmark: i32,
    pub high_hitmark: i32,
    pub collision_hitmark: i32,
    pub low_frames: &'static FrameTable,
    pub high_frames: &'static FrameTable,
    /// The action whose frames are the plunge's CanQueueAfter.
    pub low_queue: Action,
    pub high_queue: Action,
    /// Physical for most; a catalyst's element for catalysts.
    pub element: Element,
    /// PoiseDMG with StrikeTypeBlunt; 0 for StrikeTypeDefault.
    pub low_poise: f64,
    pub high_poise: f64,
    pub low_radius: f64,
    pub high_radius: f64,
    /// The plunge hits' offset ahead of the player (gcsim's
    /// info.Point{Y: 1}, or nil).
    pub offset: f64,
    /// The collision's offset and radius.
    pub collision_offset: f64,
    pub collision_radius: f64,
    pub low: &'static [f64],
    pub high: &'static [f64],
    pub collision: &'static [f64],
}

impl Spec {
    /// The physical plunges of a sword, claymore or polearm user, from its
    /// tables, as most of gcsim's plunge.go write them.
    #[allow(clippy::too_many_arguments)]
    pub const fn physical(
        low_hitmark: i32,
        high_hitmark: i32,
        collision_hitmark: i32,
        low_frames: &'static FrameTable,
        high_frames: &'static FrameTable,
        low_poise: f64,
        high_poise: f64,
        low_radius: f64,
        high_radius: f64,
        low: &'static [f64],
        high: &'static [f64],
        collision: &'static [f64],
    ) -> Spec {
        Spec {
            low_hitmark,
            high_hitmark,
            collision_hitmark,
            low_frames,
            high_frames,
            low_queue: Action::Dash,
            high_queue: Action::Dash,
            element: Element::Physical,
            low_poise,
            high_poise,
            low_radius,
            high_radius,
            offset: 1.0,
            collision_offset: 1.0,
            collision_radius: 1.0,
            low,
            high,
            collision,
        }
    }
}

impl Spec {
    /// A catalyst user's plunges: of its element, StrikeTypeDefault, on the
    /// player, the collision of radius 1.5.
    #[allow(clippy::too_many_arguments)]
    pub const fn catalyst(
        element: Element,
        low_hitmark: i32,
        high_hitmark: i32,
        collision_hitmark: i32,
        low_frames: &'static FrameTable,
        high_frames: &'static FrameTable,
        low_radius: f64,
        high_radius: f64,
        low: &'static [f64],
        high: &'static [f64],
        collision: &'static [f64],
    ) -> Spec {
        Spec {
            element,
            offset: 0.0,
            collision_offset: 0.0,
            collision_radius: 1.5,
            ..Spec::physical(low_hitmark, high_hitmark, collision_hitmark, low_frames, high_frames, 0.0, 0.0, low_radius, high_radius, low, high, collision)
        }
    }
}

/// gcsim's error for a plunge on the ground.
pub fn grounded(s: &Sim, a: Action) -> Option<String> {
    if s.player.airborne != Airborne::Grounded {
        return None;
    }
    match a {
        Action::LowPlunge => Some("low_plunge can only be used while airborne".into()),
        Action::HighPlunge => Some("high_plunge can only be used while airborne".into()),
        _ => None,
    }
}

/// The params gcsim's airborne plunges read.
pub fn unknown_param(a: Action, p: &Params, known: &[&str]) -> Option<String> {
    let name = crate::gcsl::action_name(a);
    p.0.iter().find(|(k, _)| k != "collision" && !known.iter().any(|n| k == *n)).map(|(k, _)| format!("{name} param {k}"))
}

/// Whether the plunge asks for its collision hit.
pub fn wants_collision(p: &Params) -> bool {
    p.get("collision").unwrap_or(0) > 0
}

/// gcsim's plungeCollision: the falling hit, with no gauge.
pub fn collision_info(s: &Sim, ci: usize, spec: &Spec) -> AttackInfo {
    let lvl = s.talent(ci, 0);
    let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, spec.element, 0.0, spec.collision[lvl]);
    ai.abil = "Plunge Collision";
    ai.pattern = s.circle_on_player(0.0, spec.collision_offset, spec.collision_radius);
    ai
}

/// The low or high plunge's hit.
pub fn hit_info(s: &Sim, ci: usize, high: bool, spec: &Spec) -> AttackInfo {
    let lvl = s.talent(ci, 0);
    let (mult, poise, r) = if high { (spec.high[lvl], spec.high_poise, spec.high_radius) } else { (spec.low[lvl], spec.low_poise, spec.low_radius) };
    let mut ai = AttackInfo::new(ci, AttackTag::Plunge, icd::TAG_NONE, icd::GROUP_DEFAULT, spec.element, 25.0, mult);
    ai.abil = if high { "High Plunge" } else { "Low Plunge" };
    ai.blunt = poise > 0.0;
    ai.poise = poise;
    ai.pattern = s.circle_on_player(0.0, spec.offset, r);
    ai
}

pub fn hitmark(spec: &Spec, high: bool) -> i32 {
    if high { spec.high_hitmark } else { spec.low_hitmark }
}

/// The plunge's ActInfo.
pub fn act(spec: &Spec, high: bool) -> ActInfo {
    let (t, q) = if high { (spec.high_frames, spec.high_queue) } else { (spec.low_frames, spec.low_queue) };
    ActInfo::new(Frames::Table(t), t[0], t[q as usize], AnimState::Plunge)
}

/// lowPlungeXY or highPlungeXY as most characters have them: the collision
/// if asked for, then the hit, with the callbacks given to both.
pub fn standard(s: &mut Sim, ci: usize, high: bool, p: &Params, spec: &Spec, cbs: u16) -> ActInfo {
    if wants_collision(p) {
        let ai = collision_info(s, ci, spec);
        let h = spec.collision_hitmark;
        s.queue_attack(ai, cbs, h, h);
    }
    let ai = hit_info(s, ci, high, spec);
    let h = hitmark(spec, high);
    s.queue_attack(ai, cbs, h, h);
    act(spec, high)
}
