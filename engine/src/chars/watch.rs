//! gcsim's pattern for bursts that fire on the active character's normal
//! attacks, as Xingqiu's and Yelan's do: on a change to the normal attack
//! state, after the active character's start delay for the burst's key,
//! and then every second while normal attacks go on.

use crate::sim::*;
use crate::types::*;

pub struct Watcher {
    /// The burst's status, and its ICD status.
    pub status: ModKey,
    pub icd: ModKey,
    /// gcsim's AnimationDelayKey for the burst.
    pub delay_key: usize,
    /// The character's task ids for the trigger and the ticks.
    pub trigger_task: u8,
    pub tick_task: u8,
    pub fire: fn(&mut Sim, usize),
}

impl Watcher {
    /// OnStateChange: the ticks' source frame, if it triggered at once.
    pub fn on_state(&self, s: &mut Sim, ci: usize, next: AnimState) -> Option<i32> {
        if next != AnimState::Normal {
            return None;
        }
        let delay = s.start_delay(self.delay_key);
        if delay > 0 {
            s.add_task(Task::Char { c: ci as u8, id: self.trigger_task, arg: 0 }, delay);
            None
        } else {
            self.trigger(s, ci)
        }
    }

    /// The trigger: fires and starts the ticks, whose source frame it
    /// returns, unless the burst is down or on its ICD.
    pub fn trigger(&self, s: &mut Sim, ci: usize) -> Option<i32> {
        if !s.mod_active(ci, self.status) || s.mod_active(ci, self.icd) {
            return None;
        }
        (self.fire)(s, ci);
        let f = s.f;
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: self.tick_task, arg: f }, 60);
        Some(f)
    }

    /// A tick of the ticks started at src, current the latest source.
    pub fn tick(&self, s: &mut Sim, ci: usize, src: i32, current: i32) {
        if !s.mod_active(ci, self.status) || current != src || s.current_state() != AnimState::Normal {
            return;
        }
        if s.chars[s.player.active as usize].normal_counter == 1 && s.f - s.player.state_start < s.start_delay(self.delay_key) {
            return;
        }
        (self.fire)(s, ci);
        s.queue_char_task(ci, Task::Char { c: ci as u8, id: self.tick_task, arg: src }, 60);
    }
}
