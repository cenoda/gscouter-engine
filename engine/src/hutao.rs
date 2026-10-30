//! Hu Tao, from gcsim's internal/characters/hutao.

use crate::sim::*;
use crate::types::*;

const ATTACK: [&[[f64; 15]]; 6] = [
    &[[0.468864, 0.500832, 0.5328, 0.575424, 0.607392, 0.644688, 0.69264, 0.740592, 0.788544, 0.836496, 0.884448, 0.9324, 0.980352, 1.028304, 1.076256]],
    &[[0.482539, 0.51544, 0.54834, 0.592207, 0.625108, 0.663491, 0.712842, 0.762193, 0.811543, 0.860894, 0.910244, 0.959595, 1.008946, 1.058296, 1.107647]],
    &[[0.6105, 0.652125, 0.69375, 0.74925, 0.790875, 0.839438, 0.901875, 0.964313, 1.02675, 1.089188, 1.151625, 1.214063, 1.2765, 1.338938, 1.401375]],
    &[[0.65641, 0.701165, 0.74592, 0.805594, 0.850349, 0.902563, 0.969696, 1.036829, 1.103962, 1.171094, 1.238227, 1.30536, 1.372493, 1.439626, 1.506758]],
    &[
        [0.332737, 0.355423, 0.37811, 0.408359, 0.431045, 0.457513, 0.491543, 0.525573, 0.559603, 0.593633, 0.627663, 0.661693, 0.695722, 0.729752, 0.763782],
        [0.352, 0.376, 0.4, 0.432, 0.456, 0.484, 0.52, 0.556, 0.592, 0.628, 0.664, 0.7, 0.736, 0.772, 0.808],
    ],
    &[[0.859584, 0.918192, 0.9768, 1.054944, 1.113552, 1.181928, 1.26984, 1.357752, 1.445664, 1.533576, 1.621488, 1.7094, 1.797312, 1.885224, 1.973136]],
];
const CHARGE: [f64; 15] = [1.3596, 1.4523, 1.545, 1.6686, 1.7613, 1.86945, 2.0085, 2.14755, 2.2866, 2.42565, 2.5647, 2.70375, 2.8428, 2.98185, 3.1209];
const PPATK: [f64; 15] = [0.03841, 0.04071, 0.04301, 0.046, 0.0483, 0.0506, 0.05359, 0.05658, 0.05957, 0.06256, 0.06555, 0.06854, 0.07153, 0.07452, 0.07751];
const BB: [f64; 15] = [0.64, 0.688, 0.736, 0.8, 0.848, 0.896, 0.96, 1.024, 1.088, 1.152, 1.216, 1.28, 1.36, 1.44, 1.52];
const BURST: [f64; 15] = [3.03272, 3.21432, 3.39592, 3.632, 3.8136, 3.9952, 4.23128, 4.46736, 4.70344, 4.93952, 5.1756, 5.41168, 5.64776, 5.88384, 6.11992];
const BURST_LOW: [f64; 15] = [3.7909, 4.0179, 4.2449, 4.54, 4.767, 4.994, 5.2891, 5.5842, 5.8793, 6.1744, 6.4695, 6.7646, 7.0597, 7.3548, 7.6499];
const REGEN: [f64; 15] = [0.062625, 0.066375, 0.070125, 0.075, 0.07875, 0.0825, 0.087375, 0.09225, 0.097125, 0.102, 0.106875, 0.11175, 0.116625, 0.1215, 0.126375];
const REGEN_LOW: [f64; 15] = [0.0835, 0.0885, 0.0935, 0.1, 0.105, 0.11, 0.1165, 0.123, 0.1295, 0.136, 0.1425, 0.149, 0.1555, 0.162, 0.1685];

const SKILL_START: i32 = 14;
const SKILL_FRAMES: FrameTable = set(set(set(set(abil_slice(52), Action::Attack, 29), Action::Burst, 28), Action::Dash, 37), Action::Jump, 37);

// normal attacks outside and inside Paramita Papilio
const HITMARKS: [&[i32]; 6] = [&[12], &[9], &[17], &[22], &[16, 26], &[23]];
const PP_HITMARKS: [&[i32]; 6] = [&[12], &[9], &[17], &[22], &[15, 26], &[27]];
const HALT: [&[f64]; 6] = [&[0.01], &[0.01], &[0.01], &[0.02], &[0.02, 0.02], &[0.04]];
const DEF_HALT: [&[bool]; 6] = [&[true], &[true], &[true], &[true], &[false, true], &[true]];

static ATTACK_FRAMES: [FrameTable; 6] = [
    set(normal_cancel_slice(12, 20), Action::Attack, 14),
    set(normal_cancel_slice(9, 16), Action::Attack, 12),
    set(normal_cancel_slice(17, 26), Action::Charge, 23),
    set(normal_cancel_slice(22, 31), Action::Attack, 29),
    set(normal_cancel_slice(26, 48), Action::Attack, 37),
    normal_cancel_slice(23, 72),
];
static PP_ATTACK_FRAMES: [FrameTable; 6] = [
    set(normal_cancel_slice(12, 20), Action::Attack, 14),
    set(normal_cancel_slice(9, 16), Action::Attack, 12),
    set(normal_cancel_slice(17, 26), Action::Charge, 23),
    set(normal_cancel_slice(22, 31), Action::Attack, 29),
    set(normal_cancel_slice(26, 48), Action::Attack, 36),
    normal_cancel_slice(27, 72),
];

const CHARGE_HITMARK: i32 = 19;
static CHARGE_FRAMES: FrameTable = set(set(set(set(abil_slice(62), Action::Attack, 57), Action::Skill, 60), Action::Dash, CHARGE_HITMARK), Action::Jump, CHARGE_HITMARK);
const PP_CHARGE_HITMARK: i32 = 3;
const PP_CHARGE_FRAMES: FrameTable = set(set(set(abil_slice(42), Action::Burst, 33), Action::Dash, PP_CHARGE_HITMARK), Action::Jump, PP_CHARGE_HITMARK);

const BURST_HITMARK: i32 = 66;
static BURST_FRAMES: FrameTable = set(set(set(abil_slice(98), Action::Attack, 97), Action::Skill, 97), Action::Swap, 95);

pub fn pp_charge_frames(prev: i8, next: Action) -> i32 {
    if prev == -1 || !matches!(next, Action::Dash | Action::Jump) {
        return PP_CHARGE_FRAMES[next as usize];
    }
    match prev {
        0 => {
            if next == Action::Dash {
                1
            } else {
                2
            }
        }
        1 => {
            if next == Action::Dash {
                4
            } else {
                5
            }
        }
        2 => 2,
        3 => 3,
        4 => 3,
        _ => 500,
    }
}

impl Sim {
    pub fn hutao_init(&mut self, ci: usize) {
        if self.chars[ci].ascension >= 4 {
            self.chars[ci].mods.push(Mod::new(ModKey::HutaoA4, ModKind::Stat, st::PYROP, false, false, -1));
        }
    }

    pub fn hutao_snapshot(&self, ci: usize, info: &mut AttackInfo) {
        if self.mod_active(ci, ModKey::Paramita) && matches!(info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
            info.element = Element::Pyro;
        }
    }

    pub fn hutao_attack(&mut self, ci: usize) -> ActInfo {
        let pp = self.mod_active(ci, ModKey::Paramita);
        let n = self.chars[ci].normal_counter as usize;
        let lvl = self.talent(ci, 0);
        let hitmarks = if pp { PP_HITMARKS[n] } else { HITMARKS[n] };
        for (i, mult) in ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, IcdTag::NormalAttack, IcdGroup::Default, Element::Physical, 25.0, mult[lvl]);
            ai.abil = n as u16;
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = HALT[n][i] * 60.0;
            ai.def_halt = DEF_HALT[n][i];
            let cb = if pp { CB_HUTAO_PARTICLE } else { 0 };
            self.queue_attack_at(ci, ai, cb, hitmarks[i]);
        }
        let atkspd = self.stat(ci, st::ATKSPD);
        let table = if pp { &PP_ATTACK_FRAMES[n] } else { &ATTACK_FRAMES[n] };
        let mut info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *hitmarks.last().unwrap(), AnimState::Normal);
        if pp && n == 1 {
            info.norm_for = Some(Action::Charge);
        }
        self.advance_normal(ci);
        info
    }

    pub fn hutao_charge(&mut self, ci: usize) -> ActInfo {
        let lvl = self.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, IcdTag::ExtraAttack, IcdGroup::PoleExtraAttack, Element::Physical, 25.0, CHARGE[lvl]);
        ai.abil = 10;
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        if !self.mod_active(ci, ModKey::Paramita) {
            self.queue_attack(ai, 0, 0, CHARGE_HITMARK);
            return ActInfo::new(Frames::Table(&CHARGE_FRAMES), CHARGE_FRAMES[0], CHARGE_HITMARK, AnimState::Charge);
        }
        self.extend_mod(ci, ModKey::Paramita, 108);
        self.queue_attack(ai, CB_HUTAO_PARTICLE | CB_HUTAO_BB, 0, PP_CHARGE_HITMARK);
        let mut prev: i8 = -1;
        let last = &self.player.last;
        if last.char as usize == ci && last.action == Action::Attack {
            let c = &self.chars[ci];
            prev = (c.normal_counter - 1) as i8;
            if prev < 0 {
                prev = (c.normal_hit_num - 1) as i8;
            }
        }
        let mut info = ActInfo::new(Frames::HutaoPPCharge { prev }, PP_CHARGE_FRAMES[0], 1, AnimState::Charge);
        info.on_removed = OnRemoved::HutaoPPCharge;
        info
    }

    pub fn hutao_skill(&mut self, ci: usize) -> ActInfo {
        let lvl = self.talent(ci, 1);
        let mut bonus = PPATK[lvl] * self.max_hp(ci);
        let max_bonus = self.stat(ci, st::BASEATK) * 4.0;
        if bonus > max_bonus {
            bonus = max_bonus;
        }
        self.chars[ci].hutao.ppbuff_atk = bonus;
        self.add_mod(ci, ModKey::Paramita, ModKind::Stat, st::ATK, true, 540 + SKILL_START, true);
        self.chars[ci].hutao.apply_a1 = true;
        self.queue_char_task(ci, Task::HutaoA1 { c: ci as u8 }, 540 + SKILL_START);
        let drain = 0.30 * self.current_hp(ci);
        self.modify_hp(ci, -drain);
        // the 0 dmg attack at skillStart changes nothing here
        self.set_cd_with_delay(ci, Action::Skill, 960, 14);
        ActInfo::new(Frames::Table(&SKILL_FRAMES), SKILL_FRAMES[0], SKILL_FRAMES[Action::Burst as usize], AnimState::Skill)
    }

    pub fn hutao_burst(&mut self, ci: usize) -> ActInfo {
        let low = self.chars[ci].hp_ratio <= 0.5;
        let lvl = self.talent(ci, 2);
        let (mult, regen) = if low { (BURST_LOW[lvl], REGEN_LOW[lvl]) } else { (BURST[lvl], REGEN[lvl]) };
        self.chars[ci].hutao.heal_count = 0;
        self.chars[ci].hutao.heal_src = self.max_hp(ci) * regen;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, IcdTag::None, IcdGroup::Default, Element::Pyro, 50.0, mult);
        ai.abil = if low { 13 } else { 12 };
        let cb = CB_HUTAO_HEAL | if self.chars[ci].cons >= 2 { CB_HUTAO_BB } else { 0 };
        self.queue_attack(ai, cb, 0, BURST_HITMARK);
        self.consume_energy(ci, 68);
        self.set_cd_with_delay(ci, Action::Burst, 900, 62);
        ActInfo::new(Frames::Table(&BURST_FRAMES), BURST_FRAMES[0], BURST_FRAMES[Action::Swap as usize], AnimState::Burst)
    }

    pub fn hutao_a1(&mut self, ci: usize) {
        if self.chars[ci].ascension < 1 || !self.chars[ci].hutao.apply_a1 {
            return;
        }
        self.chars[ci].hutao.apply_a1 = false;
        for i in 0..self.chars.len() {
            if i != ci {
                self.add_mod(i, ModKey::HutaoA1, ModKind::Stat, st::CR, false, 480, true);
            }
        }
    }

    pub fn hutao_particle_cb(&mut self, ci: usize) {
        if !self.mod_active(ci, ModKey::Paramita) || self.mod_active(ci, ModKey::ParamitaIcd) {
            return;
        }
        self.add_status(ci, ModKey::ParamitaIcd, 300, true);
        let count = if self.rng.float64() < 0.5 { 3.0 } else { 2.0 };
        self.queue_particle(count, Element::Pyro, self.particle_delay[ci]);
    }

    pub fn hutao_apply_bb(&mut self, ci: usize) {
        if !self.enemy_status_active(ModKey::BloodBlossom) {
            let f = self.f;
            self.queue_enemy_task(Task::HutaoBBTick { c: ci as u8, src: f }, 240);
            self.enemy.bb_src = f;
        }
        self.enemy_add_status(ModKey::BloodBlossom, 570, true);
    }

    pub fn hutao_bb_tick(&mut self, ci: usize, src: i32) {
        if self.enemy.bb_src != src || !self.enemy_status_active(ModKey::BloodBlossom) {
            return;
        }
        let lvl = self.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, IcdTag::None, IcdGroup::Default, Element::Pyro, 25.0, BB[lvl]);
        ai.abil = 11;
        if self.chars[ci].cons >= 2 {
            ai.flat_dmg += self.max_hp(ci) * 0.1;
        }
        self.queue_attack(ai, 0, 0, 0);
        self.queue_enemy_task(Task::HutaoBBTick { c: ci as u8, src }, 240);
    }

    pub fn hutao_burst_heal_cb(&mut self, ci: usize) {
        if self.chars[ci].hutao.heal_count == 5 {
            return;
        }
        self.chars[ci].hutao.heal_count += 1;
        let src = self.chars[ci].hutao.heal_src;
        let bonus = self.stat(ci, st::HEAL);
        self.heal(ci, src, bonus);
    }
}
