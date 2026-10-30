//! Xingqiu, from gcsim's internal/characters/xingqiu.

use crate::sim::*;
use crate::types::*;

const ATTACK: [&[[f64; 15]]; 5] = [
    &[[0.46612, 0.50406, 0.542, 0.5962, 0.63414, 0.6775, 0.73712, 0.79674, 0.85636, 0.9214, 0.995925, 1.083566, 1.171208, 1.258849, 1.354458]],
    &[[0.47644, 0.51522, 0.554, 0.6094, 0.64818, 0.6925, 0.75344, 0.81438, 0.87532, 0.9418, 1.017975, 1.107557, 1.197139, 1.28672, 1.384446]],
    &[
        [0.28552, 0.30876, 0.332, 0.3652, 0.38844, 0.415, 0.45152, 0.48804, 0.52456, 0.5644, 0.61005, 0.663734, 0.717419, 0.771103, 0.829668],
        [0.28552, 0.30876, 0.332, 0.3652, 0.38844, 0.415, 0.45152, 0.48804, 0.52456, 0.5644, 0.61005, 0.663734, 0.717419, 0.771103, 0.829668],
    ],
    &[[0.55986, 0.60543, 0.651, 0.7161, 0.76167, 0.81375, 0.88536, 0.95697, 1.02858, 1.1067, 1.196212, 1.301479, 1.406746, 1.512013, 1.626849]],
    &[
        [0.35862, 0.38781, 0.417, 0.4587, 0.48789, 0.52125, 0.56712, 0.61299, 0.65886, 0.7089, 0.766237, 0.833666, 0.901095, 0.968524, 1.042083],
        [0.35862, 0.38781, 0.417, 0.4587, 0.48789, 0.52125, 0.56712, 0.61299, 0.65886, 0.7089, 0.766237, 0.833666, 0.901095, 0.968524, 1.042083],
    ],
];
const RAINSCREEN: [[f64; 15]; 2] = [
    [1.68, 1.806, 1.932, 2.1, 2.226, 2.352, 2.52, 2.688, 2.856, 3.024, 3.192, 3.36, 3.57, 3.78, 3.99],
    [1.912, 2.0554, 2.1988, 2.39, 2.5334, 2.6768, 2.868, 3.0592, 3.2504, 3.4416, 3.6328, 3.824, 4.063, 4.302, 4.541],
];
const BURST: [f64; 15] = [0.54272, 0.583424, 0.624128, 0.6784, 0.719104, 0.759808, 0.81408, 0.868352, 0.922624, 0.976896, 1.031168, 1.08544, 1.15328, 1.22112, 1.28896];

const HITMARKS: [&[i32]; 5] = [&[10], &[13], &[9, 19], &[17], &[18, 39]];
const HALT: [f64; 5] = [0.03, 0.03, 0.06, 0.06, 0.1];
static ATTACK_FRAMES: [FrameTable; 5] = [
    set(normal_cancel_slice(10, 35), Action::Attack, 18),
    set(normal_cancel_slice(13, 29), Action::Attack, 24),
    set(normal_cancel_slice(19, 35), Action::Attack, 26),
    set(normal_cancel_slice(17, 33), Action::Attack, 28),
    normal_cancel_slice(39, 66),
];

const SKILL_HITMARKS: [i32; 2] = [12, 31];
static SKILL_FRAMES: FrameTable = set(set(set(abil_slice(67), Action::Skill, 65), Action::Dash, 31), Action::Jump, 34);

const BURST_HITMARK: i32 = 18;
static BURST_FRAMES: FrameTable = set(set(set(set(abil_slice(40), Action::Attack, 33), Action::Skill, 33), Action::Dash, 33), Action::Jump, 33);

impl Sim {
    pub fn xq_init(&mut self, ci: usize) {
        if self.chars[ci].ascension >= 4 {
            self.chars[ci].mods.push(Mod::new(ModKey::XingqiuA4, ModKind::Stat, st::HYDROP, false, false, -1));
        }
    }

    pub fn xq_attack(&mut self, ci: usize) -> ActInfo {
        let n = self.chars[ci].normal_counter as usize;
        let lvl = self.talent(ci, 0);
        for (i, mult) in ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, IcdTag::NormalAttack, IcdGroup::Default, Element::Physical, 25.0, mult[lvl]);
            ai.abil = n as u16;
            ai.hitlag_halt = HALT[n] * 60.0;
            ai.hitlag_factor = 0.01;
            ai.def_halt = true;
            self.queue_attack_at(ci, ai, 0, HITMARKS[n][i]);
        }
        let atkspd = self.stat(ci, st::ATKSPD);
        let table = &ATTACK_FRAMES[n];
        let info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *HITMARKS[n].last().unwrap(), AnimState::Normal);
        self.advance_normal(ci);
        info
    }

    pub fn xq_skill(&mut self, ci: usize) -> ActInfo {
        let lvl = self.talent(ci, 1);
        let c4 = self.chars[ci].cons >= 4 && self.mod_active(ci, ModKey::XingqiuBurst);
        for (i, v) in RAINSCREEN.iter().enumerate() {
            let mut mult = v[lvl];
            if c4 {
                mult *= 1.5;
            }
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, IcdTag::None, IcdGroup::Default, Element::Hydro, 25.0, mult);
            ai.abil = 10 + i as u16;
            ai.hitlag_halt = 0.02 * 60.0;
            ai.hitlag_factor = 0.01;
            ai.def_halt = true;
            self.queue_attack_at(ci, ai, CB_XQ_PARTICLE, SKILL_HITMARKS[i]);
        }
        self.xq_apply_orbital(ci, 15 * 60, 43);
        self.set_cd_with_delay(ci, Action::Skill, 21 * 60, 10);
        ActInfo::new(Frames::Table(&SKILL_FRAMES), SKILL_FRAMES[0], SKILL_FRAMES[Action::Dash as usize], AnimState::Skill)
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
        let mut ai = AttackInfo::new(ci, AttackTag::None, IcdTag::None, IcdGroup::Default, Element::Hydro, 25.0, 0.0);
        ai.abil = 13;
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
        self.xq_apply_orbital(ci, dur, BURST_HITMARK);
        self.chars[ci].xq.num_swords = 2;
        self.chars[ci].xq.next_regen = false;
        self.set_cd(ci, Action::Burst, 20 * 60);
        self.consume_energy(ci, 3);
        ActInfo::new(Frames::Table(&BURST_FRAMES), BURST_FRAMES[0], BURST_HITMARK, AnimState::Burst)
    }

    pub fn xq_sword_wave(&mut self, ci: usize) {
        let lvl = self.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, IcdTag::ElementalBurst, IcdGroup::Default, Element::Hydro, 25.0, BURST[lvl]);
        ai.abil = 12;
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
