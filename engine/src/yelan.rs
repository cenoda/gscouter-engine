//! Yelan, from gcsim's internal/characters/yelan, for C0 without
//! breakthrough shots.

use crate::sim::*;
use crate::types::*;

const ATTACK: [&[[f64; 15]]; 4] = [
    &[[0.40678, 0.43989, 0.473, 0.5203, 0.55341, 0.59125, 0.64328, 0.69531, 0.74734, 0.8041, 0.86086, 0.91762, 0.97438, 1.03114, 1.0879]],
    &[[0.39044, 0.42222, 0.454, 0.4994, 0.53118, 0.5675, 0.61744, 0.66738, 0.71732, 0.7718, 0.82628, 0.88076, 0.93524, 0.98972, 1.0442]],
    &[[0.516, 0.558, 0.6, 0.66, 0.702, 0.75, 0.816, 0.882, 0.948, 1.02, 1.092, 1.164, 1.236, 1.308, 1.38]],
    &[
        [0.32508, 0.35154, 0.378, 0.4158, 0.44226, 0.4725, 0.51408, 0.55566, 0.59724, 0.6426, 0.68796, 0.73332, 0.77868, 0.82404, 0.8694],
        [0.32508, 0.35154, 0.378, 0.4158, 0.44226, 0.4725, 0.51408, 0.55566, 0.59724, 0.6426, 0.68796, 0.73332, 0.77868, 0.82404, 0.8694],
    ],
];
const SKILL: [f64; 15] = [0.226136, 0.243096, 0.260056, 0.28267, 0.29963, 0.31659, 0.339204, 0.361818, 0.384431, 0.407045, 0.429658, 0.452272, 0.480539, 0.508806, 0.537073];
const BURST: [f64; 15] = [0.07308, 0.078561, 0.084042, 0.09135, 0.096831, 0.102312, 0.10962, 0.116928, 0.124236, 0.131544, 0.138852, 0.14616, 0.155295, 0.16443, 0.173565];
const BURST_DICE: [f64; 15] = [0.04872, 0.052374, 0.056028, 0.0609, 0.064554, 0.068208, 0.07308, 0.077952, 0.082824, 0.087696, 0.092568, 0.09744, 0.10353, 0.10962, 0.11571];

const HITMARKS: [&[i32]; 4] = [&[13], &[13], &[18], &[15, 29]];
static ATTACK_FRAMES: [FrameTable; 4] = [normal_cancel_slice(13, 15), normal_cancel_slice(13, 21), normal_cancel_slice(18, 38), normal_cancel_slice(29, 67)];
const TRAVEL: i32 = 10;

const SKILL_HITMARK: i32 = 35;
static SKILL_FRAMES: FrameTable = set(set(set(set(abil_slice(42), Action::Burst, 41), Action::Dash, 41), Action::Jump, 41), Action::Swap, 40);

const BURST_HITMARK: i32 = 76;
const BURST_TRAVEL: i32 = 20;
static BURST_FRAMES: FrameTable = set(set(set(abil_slice(93), Action::Skill, 92), Action::Jump, 91), Action::Swap, 90);

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
        for (i, mult) in ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, IcdTag::None, IcdGroup::Default, Element::Physical, 25.0, mult[lvl]);
            ai.abil = n as u16;
            self.queue_attack(ai, 0, HITMARKS[n][i], HITMARKS[n][i] + TRAVEL);
        }
        let atkspd = self.stat(ci, st::ATKSPD);
        let table = &ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *HITMARKS[n].last().unwrap(), AnimState::Normal);
        self.advance_normal(ci);
        info
    }

    pub fn yelan_skill(&mut self, ci: usize) -> ActInfo {
        self.chars[ci].yelan.marked = false;
        let lvl = self.talent(ci, 1);
        self.chars[ci].yelan.skill_flat = SKILL[lvl] * self.max_hp(ci);
        self.add_task(Task::YelanSkillHit { c: ci as u8 }, SKILL_HITMARK);
        self.set_cd_with_delay(ci, Action::Skill, 600, SKILL_HITMARK - 2);
        ActInfo::new(Frames::Table(&SKILL_FRAMES), SKILL_FRAMES[0], SKILL_FRAMES[Action::Swap as usize], AnimState::Skill)
    }

    /// The two tasks gcsim queues at the skill's hitmark: mark the enemy,
    /// then explode the marks. The damage uses Yelan's HP at the cast.
    pub fn yelan_skill_hit(&mut self, ci: usize) {
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, IcdTag::None, IcdGroup::Default, Element::Hydro, 25.0, 0.0);
        ai.abil = 10;
        ai.flat_dmg = self.chars[ci].yelan.skill_flat;
        self.queue_attack(ai, CB_YELAN_PARTICLE | CB_YELAN_SKILL, 1, 1);
    }

    pub fn yelan_particle_cb(&mut self, ci: usize) {
        if self.mod_active(ci, ModKey::YelanParticleIcd) {
            return;
        }
        self.add_status(ci, ModKey::YelanParticleIcd, 18, true);
        self.queue_particle(4.0, Element::Hydro, 100);
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
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, IcdTag::None, IcdGroup::Default, Element::Hydro, 50.0, 0.0);
        ai.abil = 11;
        ai.flat_dmg = BURST[lvl] * self.max_hp(ci);
        ai.hitlag_halt = 0.05 * 60.0;
        ai.hitlag_factor = 0.05;
        ai.deployable = true;
        self.queue_attack(ai, 0, BURST_HITMARK, BURST_HITMARK);
        self.add_task(Task::YelanBurstStatus { c: ci as u8 }, BURST_HITMARK);
        self.set_cd(ci, Action::Burst, 18 * 60);
        self.consume_energy(ci, 6);
        ActInfo::new(Frames::Table(&BURST_FRAMES), BURST_FRAMES[0], BURST_FRAMES[Action::Swap as usize], AnimState::Burst)
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
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, IcdTag::YelanBurst, IcdGroup::YelanBurst, Element::Hydro, 25.0, 0.0);
        ai.abil = 12;
        ai.flat_dmg = BURST_DICE[lvl] * hp;
        let (stats, clvl) = self.make_snapshot(&mut ai);
        let mut ev = AttackEvent::new(ai, 0);
        ev.stats = stats;
        ev.char_lvl = clvl;
        for i in 0..3 {
            self.queue_attack_with_snap(ev, 0, BURST_TRAVEL + i * 6);
        }
    }
}
