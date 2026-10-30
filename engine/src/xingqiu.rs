//! Xingqiu, from gcsim's internal/characters/xingqiu.

use crate::gcsimdata::chars::xingqiu as g;
use crate::sim::*;
use crate::types::*;

impl Sim {
    pub fn xq_init(&mut self, ci: usize) {
        if self.chars[ci].ascension >= 4 {
            self.chars[ci].mods.push(Mod::new(ModKey::XingqiuA4, ModKind::Stat, st::HYDROP, false, false, -1));
        }
    }

    pub fn xq_attack(&mut self, ci: usize) -> ActInfo {
        let n = self.chars[ci].normal_counter as usize;
        let lvl = self.talent(ci, 0);
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAMES[n] * 60.0;
            ai.hitlag_factor = 0.01;
            ai.def_halt = true;
            self.queue_attack_at(ci, ai, 0, g::ATTACK_HITMARKS[n][i]);
        }
        let atkspd = self.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        self.advance_normal(ci);
        info
    }

    pub fn xq_skill(&mut self, ci: usize) -> ActInfo {
        let lvl = self.talent(ci, 1);
        let c4 = self.chars[ci].cons >= 4 && self.mod_active(ci, ModKey::XingqiuBurst);
        for (i, v) in g::RAINSCREEN.iter().enumerate() {
            let mut mult = v[lvl];
            if c4 {
                mult *= 1.5;
            }
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, mult);
            ai.abil = ["Guhua Sword: Fatal Rainscreen 0", "Guhua Sword: Fatal Rainscreen 1"][i];
            ai.hitlag_halt = 0.02 * 60.0;
            ai.hitlag_factor = 0.01;
            ai.def_halt = true;
            self.queue_attack_at(ci, ai, CB_XQ_PARTICLE, g::SKILL_HITMARKS[i]);
        }
        self.xq_apply_orbital(ci, 15 * 60, 43);
        self.set_cd_with_delay(ci, Action::Skill, 21 * 60, 10);
        ActInfo::new(Frames::Table(&g::SKILL_FRAMES), g::SKILL_FRAMES[0], g::SKILL_FRAMES[Action::Dash as usize], AnimState::Skill)
    }

    pub fn xq_particle_cb(&mut self, ci: usize) {
        if self.mod_active(ci, ModKey::XingqiuParticleIcd) {
            return;
        }
        self.add_status(ci, ModKey::XingqiuParticleIcd, 60, true);
        self.queue_particle(5.0, Element::Hydro, self.particle_delay[ci]);
    }

    fn xq_apply_orbital(&mut self, ci: usize, duration: i32, delay: i32) {
        if !self.chars[ci].xq.orbital_active {
            self.queue_char_task(ci, Task::XqOrbitalTick { c: ci as u8 }, delay);
            self.chars[ci].xq.orbital_active = true;
        }
        self.add_status(ci, ModKey::XingqiuOrbital, duration, true);
    }

    pub fn xq_orbital_tick(&mut self, ci: usize) {
        if !self.mod_active(ci, ModKey::XingqiuOrbital) {
            self.chars[ci].xq.orbital_active = false;
            return;
        }
        let mut ai = AttackInfo::new(ci, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
        ai.abil = "Xingqiu Orbital";
        self.queue_char_task(ci, Task::XqOrbitalTick { c: ci as u8 }, 135);
        self.queue_attack(ai, 0, -1, 1);
    }

    pub fn xq_burst(&mut self, ci: usize) -> ActInfo {
        let mut dur = 15;
        if self.chars[ci].cons >= 2 {
            dur += 3;
        }
        dur *= 60;
        self.add_status(ci, ModKey::XingqiuBurst, dur + 33, true);
        self.xq_apply_orbital(ci, dur, g::BURST_HITMARK);
        self.chars[ci].xq.num_swords = 2;
        self.chars[ci].xq.next_regen = false;
        self.set_cd(ci, Action::Burst, 20 * 60);
        self.consume_energy(ci, 3);
        ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_HITMARK, AnimState::Burst)
    }

    pub fn xq_sword_wave(&mut self, ci: usize) {
        let lvl = self.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Hydro, 25.0, g::BURST[lvl]);
        ai.abil = "Guhua Sword: Raincutter";
        let (stats, lvl) = self.make_snapshot(&mut ai);
        let mut ev = AttackEvent::new(ai, 0);
        ev.stats = stats;
        ev.char_lvl = lvl;
        let x = self.chars[ci].xq;
        for i in 0..x.num_swords {
            // the C2 shred has a 1 frame icd and the C6 energy comes once:
            // both land with the first sword of the wave
            let mut cb = 0;
            if i == 0 {
                if self.chars[ci].cons >= 2 {
                    cb |= CB_XQ_C2;
                }
                if x.next_regen {
                    cb |= CB_XQ_C6;
                }
            }
            self.queue_attack_with_snap(ev, cb, 20);
        }
        let c6 = self.chars[ci].cons >= 6;
        let xq = &mut self.chars[ci].xq;
        match xq.num_swords {
            2 => {
                xq.num_swords = 3;
                xq.next_regen = false;
            }
            3 => {
                if c6 {
                    xq.num_swords = 5;
                    xq.next_regen = true;
                } else {
                    xq.num_swords = 2;
                    xq.next_regen = false;
                }
            }
            _ => {
                xq.num_swords = 2;
                xq.next_regen = false;
            }
        }
        self.add_status(ci, ModKey::XingqiuBurstIcd, 60, true);
    }
}
