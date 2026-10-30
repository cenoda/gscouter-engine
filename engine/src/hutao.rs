//! Hu Tao, from gcsim's internal/characters/hutao.

use crate::gcsimdata::chars::hutao as g;
use crate::sim::*;
use crate::types::*;

pub fn pp_charge_frames(prev: i8, next: Action) -> i32 {
    if prev == -1 || !matches!(next, Action::Dash | Action::Jump) {
        return g::PP_CHARGE_FRAMES[next as usize];
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
        let hitmarks = if pp { g::PP_ATTACK_HITMARKS[n] } else { g::ATTACK_HITMARKS[n] };
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = (if pp { g::PP_ATTACK_HITLAG_HALT_FRAME[n][i] } else { g::ATTACK_HITLAG_HALT_FRAME[n][i] }) * 60.0;
            ai.def_halt = if pp { g::PP_ATTACK_DEF_HALT[n][i] } else { g::ATTACK_DEF_HALT[n][i] };
            let cb = if pp { CB_HUTAO_PARTICLE } else { 0 };
            self.queue_attack_at(ci, ai, cb, hitmarks[i]);
        }
        let atkspd = self.stat(ci, st::ATKSPD);
        let table = if pp { &g::PP_ATTACK_FRAMES[n] } else { &g::ATTACK_FRAMES[n] };
        let mut info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], *hitmarks.last().unwrap(), AnimState::Normal);
        if pp && n == 1 {
            info.norm_for = Some(Action::Charge);
        }
        self.advance_normal(ci);
        info
    }

    pub fn hutao_charge(&mut self, ci: usize) -> ActInfo {
        let lvl = self.talent(ci, 0);
        let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_EXTRA_ATTACK, icd::GROUP_POLE_EXTRA_ATTACK, Element::Physical, 25.0, g::CHARGE[lvl]);
        ai.abil = "Charge Attack";
        ai.hitlag_factor = 0.01;
        ai.def_halt = true;
        ai.deployable = true;
        if !self.mod_active(ci, ModKey::Paramita) {
            self.queue_attack(ai, 0, 0, g::CHARGE_HITMARK);
            return ActInfo::new(Frames::Table(&g::CHARGE_FRAMES), g::CHARGE_FRAMES[0], g::CHARGE_HITMARK, AnimState::Charge);
        }
        self.extend_mod(ci, ModKey::Paramita, 108);
        self.queue_attack(ai, CB_HUTAO_PARTICLE | CB_HUTAO_BB, 0, g::PP_CHARGE_HITMARK);
        let mut prev: i8 = -1;
        let last = &self.player.last;
        if last.char as usize == ci && last.action == Action::Attack {
            let c = &self.chars[ci];
            prev = (c.normal_counter - 1) as i8;
            if prev < 0 {
                prev = (c.normal_hit_num - 1) as i8;
            }
        }
        let mut info = ActInfo::new(Frames::HutaoPPCharge { prev }, g::PP_CHARGE_FRAMES[0], 1, AnimState::Charge);
        info.on_removed = OnRemoved::HutaoPPCharge;
        info
    }

    pub fn hutao_skill(&mut self, ci: usize) -> ActInfo {
        let lvl = self.talent(ci, 1);
        let mut bonus = g::PPATK[lvl] * self.max_hp(ci);
        let max_bonus = self.stat(ci, st::BASEATK) * 4.0;
        if bonus > max_bonus {
            bonus = max_bonus;
        }
        self.chars[ci].hutao.ppbuff_atk = bonus;
        self.add_mod(ci, ModKey::Paramita, ModKind::Stat, st::ATK, true, 540 + g::SKILL_START, true);
        self.chars[ci].hutao.apply_a1 = true;
        self.queue_char_task(ci, Task::HutaoA1 { c: ci as u8 }, 540 + g::SKILL_START);
        let drain = 0.30 * self.current_hp(ci);
        self.modify_hp(ci, -drain);
        // the 0 dmg attack at skillStart changes nothing here
        self.set_cd_with_delay(ci, Action::Skill, 960, 14);
        ActInfo::new(Frames::Table(&g::SKILL_FRAMES), g::SKILL_FRAMES[0], g::SKILL_FRAMES[Action::Burst as usize], AnimState::Skill)
    }

    pub fn hutao_burst(&mut self, ci: usize) -> ActInfo {
        let low = self.chars[ci].hp_ratio <= 0.5;
        let lvl = self.talent(ci, 2);
        let (mult, regen) = if low { (g::BURST_LOW[lvl], g::REGEN_LOW[lvl]) } else { (g::BURST[lvl], g::REGEN[lvl]) };
        self.chars[ci].hutao.heal_count = 0;
        self.chars[ci].hutao.heal_src = self.max_hp(ci) * regen;
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 50.0, mult);
        ai.abil = if low { "Spirit Soother (Low HP)" } else { "Spirit Soother" };
        let cb = CB_HUTAO_HEAL | if self.chars[ci].cons >= 2 { CB_HUTAO_BB } else { 0 };
        self.queue_attack(ai, cb, 0, g::BURST_HITMARK);
        self.consume_energy(ci, 68);
        self.set_cd_with_delay(ci, Action::Burst, 900, 62);
        ActInfo::new(Frames::Table(&g::BURST_FRAMES), g::BURST_FRAMES[0], g::BURST_FRAMES[Action::Swap as usize], AnimState::Burst)
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
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Pyro, 25.0, g::BB[lvl]);
        ai.abil = "Blood Blossom";
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
