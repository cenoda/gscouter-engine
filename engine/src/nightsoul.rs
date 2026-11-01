//! Nightsoul, from gcsim's internal/template/nightsoul and
//! pkg/simulation's setupNightsoulBurst: each character's Nightsoul points
//! and Nightsoul's Blessing (a status), the timer that ends a timed
//! blessing (which waits for the end of an action in the states the
//! character names), the OnNightsoulGenerate and OnNightsoulConsume events,
//! and the Nightsoul Burst that elemental damage triggers when a Natlan
//! character is on the team.
//!
//! The state is general: a character with Nightsoul keeps it in its
//! `Char::ns`, calls these methods as gcsim's characters call their
//! nightsoul.State, and gives its callbacks as task ids of its own.

use crate::content::{Event, EventKind, Owner};
use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

/// gcsim's NightsoulBlessingStatus.
pub const NIGHTSOUL_BLESSING: ModKey = ModKey::new("nightsoul-blessing");
/// gcsim's nightsoulBurstICDStatus, a status of the whole fight.
pub const NIGHTSOUL_BURST_ICD: ModKey = ModKey::new("nightsoul-burst-icd");
/// gcsim's nightsoulBurstICD, by the number of Natlan characters.
const NIGHTSOUL_BURST_ICD_FRAMES: [i32; 5] = [0, 18 * 60, 12 * 60, 9 * 60, 9 * 60];

/// The callback of a timed blessing that only ends it (gcsim's nil
/// callback, which is ExitBlessing); any other is a task id of the
/// character's own, run with arg 0.
pub const CB_EXIT: u8 = u8::MAX;

/// A character's Nightsoul (gcsim's nightsoul.State).
#[derive(Clone, Copy, Debug)]
pub struct Nightsoul {
    pub points: f64,
    /// The frame the current timed blessing ends at, -1 for none (gcsim's
    /// ExitStateF).
    pub exit_state_f: i32,
    /// The most points it holds, -1 for no limit.
    pub max_points: f64,
    /// The animation states that delay the blessing's end until they end,
    /// as bits by AnimState (gcsim's SetExtendNsStates).
    pub extend_states: u16,
    /// Whether its OnStateChange subscription exists (gcsim keeps the slot
    /// once subscribed, and Unsubscribe only empties it).
    extend_sub: bool,
    /// The subscription's callback while it waits: the timer's src and its
    /// callback.
    extend_armed: Option<(i32, u8)>,
}

impl Nightsoul {
    pub fn new() -> Nightsoul {
        Nightsoul { points: 0.0, exit_state_f: -1, max_points: -1.0, extend_states: 0, extend_sub: false, extend_armed: None }
    }

    /// gcsim's SetExtendNsStates.
    pub fn set_extend_states(&mut self, states: &[AnimState]) {
        self.extend_states = states.iter().fold(0, |m, s| m | 1 << *s as u16);
    }

    fn clamp(&mut self) {
        if self.max_points > 0.0 && self.points > self.max_points {
            self.points = self.max_points;
        } else if self.points < 0.0 {
            self.points = 0.0;
        }
    }
}

impl Sim {
    /// gcsim's HasBlessing.
    #[inline]
    pub fn ns_has_blessing(&self, ci: usize) -> bool {
        self.mod_active(ci, NIGHTSOUL_BLESSING)
    }

    /// gcsim's Duration: what is left of the blessing (0 when it has no
    /// end).
    pub fn ns_duration(&self, ci: usize) -> i32 {
        match self.chars[ci].mods.iter().find(|m| m.key == NIGHTSOUL_BLESSING) {
            Some(m) if m.expiry() > self.f => m.expiry() - self.f,
            _ => 0,
        }
    }

    #[inline]
    pub fn ns_points(&self, ci: usize) -> f64 {
        self.chars[ci].ns.points
    }

    /// gcsim's SetNightsoulExitTimer: while the blessing is up, it now ends
    /// in `dur` frames with the callback `cb` (`CB_EXIT` or a task id of
    /// the character's), unless the timer is set again or the blessing
    /// ended by ns_exit.
    pub fn ns_set_exit_timer(&mut self, ci: usize, dur: i32, cb: u8) {
        if !self.ns_has_blessing(ci) {
            return;
        }
        if self.ns_duration(ci) != dur {
            self.add_status(ci, NIGHTSOUL_BLESSING, dur, true);
        }
        let src = self.f + dur;
        self.chars[ci].ns.exit_state_f = src;
        self.queue_char_task(ci, Task::NightsoulExit { c: ci as u8, src, cb }, dur);
    }

    /// The timer's task: the callback, or, while the active character is
    /// in one of the states that extend it, the blessing kept without end
    /// until the state changes.
    pub(crate) fn ns_exit_timer(&mut self, ci: usize, src: i32, cb: u8) {
        if self.chars[ci].ns.exit_state_f != src {
            return;
        }
        let state = self.current_state();
        if self.chars[ci].ns.extend_states & 1 << state as u16 == 0 || self.player.active as usize != ci {
            self.ns_callback(ci, cb);
            return;
        }
        // gcsim subscribes "ns-extend-state-<char>" to OnStateChange, in
        // the slot it had if it was subscribed before
        if !self.chars[ci].ns.extend_sub {
            self.chars[ci].ns.extend_sub = true;
            self.subscribe(EventKind::StateChange, Owner::NightsoulExtend(ci as u8));
        }
        self.chars[ci].ns.extend_armed = Some((src, cb));
        self.add_status(ci, NIGHTSOUL_BLESSING, -1, false);
    }

    /// The OnStateChange subscription: the callback if the timer is still
    /// the current one, then Unsubscribe.
    pub(crate) fn ns_extend_event(&mut self, ci: usize) {
        let Some((src, cb)) = self.chars[ci].ns.extend_armed else { return };
        if self.chars[ci].ns.exit_state_f == src {
            self.ns_callback(ci, cb);
        }
        self.chars[ci].ns.extend_armed = None;
    }

    fn ns_callback(&mut self, ci: usize, cb: u8) {
        if cb == CB_EXIT {
            self.ns_exit(ci);
        } else {
            crate::chars::dispatch::task(self.chars[ci].kind, self, ci, cb, 0);
        }
    }

    /// gcsim's EnterTimedBlessing: the blessing with `amount` points, for
    /// `dur` frames (no end when negative), then the callback.
    pub fn ns_enter_timed(&mut self, ci: usize, amount: f64, dur: i32, cb: u8) {
        self.chars[ci].ns.points = amount;
        self.add_status(ci, NIGHTSOUL_BLESSING, dur, true);
        if dur > 0 {
            self.ns_set_exit_timer(ci, dur, cb);
        }
    }

    /// gcsim's EnterBlessing: the blessing with no end (for the characters
    /// ported later that enter it so).
    #[allow(dead_code)]
    pub fn ns_enter(&mut self, ci: usize, amount: f64) {
        self.ns_enter_timed(ci, amount, -1, CB_EXIT);
    }

    /// gcsim's ExitBlessing.
    pub fn ns_exit(&mut self, ci: usize) {
        self.chars[ci].ns.exit_state_f = -1;
        self.delete_mod(ci, NIGHTSOUL_BLESSING);
    }

    /// gcsim's GeneratePoints, which emits OnNightsoulGenerate with the
    /// points gained.
    pub fn ns_generate(&mut self, ci: usize, amount: f64) {
        let ns = &mut self.chars[ci].ns;
        let prev = ns.points;
        ns.points += amount;
        ns.clamp();
        let added = ns.points - prev;
        self.emit(Event::NightsoulGenerate { on: ci, amount: added });
    }

    /// gcsim's ConsumePoints, which emits OnNightsoulConsume with the
    /// points used.
    pub fn ns_consume(&mut self, ci: usize, amount: f64) {
        let ns = &mut self.chars[ci].ns;
        let prev = ns.points;
        ns.points -= amount;
        ns.clamp();
        let used = prev - ns.points;
        self.emit(Event::NightsoulConsume { on: ci, amount: used });
    }

    /// gcsim's ClearPoints (no event).
    pub fn ns_clear(&mut self, ci: usize) {
        self.chars[ci].ns.points = 0.0;
    }

    /// gcsim's State.Condition: .<char>.nightsoul.state, .points and
    /// .duration.
    pub fn ns_condition(&self, ci: usize, fields: &[String]) -> Result<Num, String> {
        match fields.get(1).map(String::as_str) {
            Some("state") => Ok(Num::from_bool(self.ns_has_blessing(ci))),
            Some("points") => Ok(Num::Float(self.ns_points(ci))),
            Some("duration") => Ok(Num::Int(self.ns_duration(ci) as i64)),
            Some(f) => Err(format!("invalid nightsoul condition: {f}")),
            None => Err("bad nightsoul condition: invalid num of fields".into()),
        }
    }

    /// gcsim's setupNightsoulBurst, after every Init: with a Natlan
    /// character on the team, elemental damage on the enemy triggers a
    /// Nightsoul Burst, then waits out an ICD set by how many there are.
    pub(crate) fn nightsoul_burst_setup(&mut self) {
        let count = self
            .chars
            .iter()
            .filter(|c| crate::data::CHARS.iter().any(|d| d.key == crate::gcsl::char_name(c.key) && d.zone == crate::data::ZONE_NATLAN))
            .count();
        if count == 0 {
            return;
        }
        self.nightsoul_burst_icd = NIGHTSOUL_BURST_ICD_FRAMES[count.min(4)];
        self.subscribe(EventKind::EnemyDamage, Owner::NightsoulBurst);
    }

    pub(crate) fn nightsoul_burst_event(&mut self, e: &Event) {
        let Event::EnemyDamage(ev) = *e else { return };
        if self.statuses.iter().any(|s| s.0 == NIGHTSOUL_BURST_ICD && s.1 > self.f) {
            return;
        }
        if !matches!(ev.info.element, Element::Electro | Element::Pyro | Element::Cryo | Element::Hydro | Element::Dendro | Element::Anemo | Element::Geo) {
            return;
        }
        self.emit(Event::NightsoulBurst(ev));
        let icd = self.nightsoul_burst_icd;
        self.add_core_status(NIGHTSOUL_BURST_ICD, icd);
    }
}
