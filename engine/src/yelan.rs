//! Yelan, from gcsim's internal/characters/yelan, for C0 without
//! breakthrough shots.

use crate::gcsimdata::chars::yelan as g;
use crate::sim::*;
use crate::types::*;

const TRAVEL: i32 = 10;

impl Sim {
    pub fn yelan_init(&mut self, ci: usize) {
        if self.chars[ci].ascension >= 1 {
            let mut els: Vec<Element> = self.chars.iter().map(|c| c.element).collect();
            els.sort_by_key(|e| *e as u8);
            els.dedup();
            let count = els.len();
            let mut m = Mod::new(ModKey::YelanA1, ModKind::Stat, st::HPP, false, false, -1);
            m.value = if count >= 4 { 0.3 } else { count as f64 * 0.06 };
            self.chars[ci].mods.push(m);
        }
    }

    pub fn yelan_attack(&mut self, ci: usize) -> ActInfo {
        let n = self.chars[ci].normal_counter as usize;
        let lvl = self.talent(ci, 0);
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            self.queue_attack(ai, 0, g::ATTACK_HITMARKS[n][i], g::ATTACK_HITMARKS[n][i] + TRAVEL);
        }
        let atkspd = self.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *g::ATTACK_HITMARKS[n].last().unwrap(), AnimState::Normal);
        self.advance_normal(ci);
        info
    }

    pub fn yelan_skill(&mut self, ci: usize) -> ActInfo {
        self.chars[ci].yelan.marked = false;
        let lvl = self.talent(ci, 1);
        self.chars[ci].yelan.skill_flat = g::SKILL[lvl] * self.max_hp(ci);
        self.add_task(Task::YelanSkillHit { c: ci as u8 }, g::SKILL_HITMARK);
        self.set_cd_with_delay(ci, Action::Skill, 600, g::SKILL_HITMARK - 2);
        ActInfo::new(Frames::Table(&g::SKILL_FRAMES), g::SKILL_FRAMES[0], g::SKILL_FRAMES[Action::Swap as usize], AnimState::Skill)
    }

    /// The two tasks gcsim queues at the skill's hitmark: mark the enemy,
    /// then explode the marks. The damage uses Yelan's HP at the cast.
    pub fn yelan_skill_hit(&mut self, ci: usize) {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 25.0, 0.0);
        ai.abil = "Lingering Lifeline";
        ai.flat_dmg = self.chars[ci].yelan.skill_flat;
        self.queue_attack(ai, CB_YELAN_PARTICLE | CB_YELAN_SKILL, 1, 1);
    }

    pub fn yelan_particle_cb(&mut self, ci: usize) {
        if self.mod_active(ci, ModKey::YelanParticleIcd) {
            return;
        }
        self.add_status(ci, ModKey::YelanParticleIcd, 18, true);
        self.queue_particle(4.0, Element::Hydro, self.particle_delay[ci]);
    }

    pub fn yelan_skill_cb(&mut self, ci: usize) {
        // breakthrough: no aimed shots here, but the roll is kept
        let _ = self.rng.float64() < 0.34;
        if self.mod_active(ci, ModKey::YelanBurst) {
            self.yelan_throw(ci);
        }
    }

    pub fn yelan_burst(&mut self, ci: usize) -> ActInfo {
        let lvl = self.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Hydro, 50.0, 0.0);
        ai.abil = "Depth-Clarion Dice";
        ai.flat_dmg = g::BURST[lvl] * self.max_hp(ci);
        ai.hitlag_halt = 0.05 * 60.0;
        ai.hitlag_factor = 0.05;
        ai.deployable = true;
        self.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        self.add_task(Task::YelanBurstStatus { c: ci as u8 }, g::BURST_HITMARK);
        self.set_cd(ci, Action::Burst, 18 * 60);
        self.consume_energy(ci, 6);
        ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_FRAMES[Action::Swap as usize], AnimState::Burst)
    }

    pub fn yelan_burst_status(&mut self, ci: usize) {
        self.add_status(ci, ModKey::YelanBurst, 15 * 60, false);
        if self.chars[ci].ascension < 4 {
            return;
        }
        let f = self.f;
        for i in 0..self.chars.len() {
            let mut m = Mod::new(ModKey::YelanA4, ModKind::Attack, st::NONE, false, false, f + 15 * 60);
            m.started = f;
            let mods = &mut self.chars[i].mods;
            match mods.iter().position(|x| x.key == ModKey::YelanA4) {
                Some(k) => mods[k] = m,
                None => mods.push(m),
            }
        }
    }

    pub fn yelan_throw(&mut self, ci: usize) {
        let lvl = self.talent(ci, 2);
        let hp = self.max_hp(ci);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_YELAN_BURST, icd::GROUP_YELAN_BURST, Element::Hydro, 25.0, 0.0);
        ai.abil = "Exquisite Throw";
        ai.flat_dmg = g::BURST_DICE[lvl] * hp;
        let (stats, clvl) = self.make_snapshot(&mut ai);
        let mut ev = AttackEvent::new(ai, 0);
        ev.stats = stats;
        ev.char_lvl = clvl;
        for i in 0..3 {
            self.queue_attack_with_snap(ev, 0, g::BURST_TRAVEL + i * 6);
        }
    }
}
