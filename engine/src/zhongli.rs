//! Zhongli's hold skill, stele and Jade Shield, from gcsim's
//! internal/characters/zhongli. The rotation uses no other ability.

use crate::sim::*;
use crate::types::*;

const SKILL: [f64; 15] = [0.16, 0.172, 0.184, 0.2, 0.212, 0.224, 0.24, 0.256, 0.272, 0.288, 0.304, 0.32, 0.34, 0.36, 0.38];
const SKILL_TICK: [f64; 15] = [0.32, 0.344, 0.368, 0.4, 0.424, 0.448, 0.48, 0.512, 0.544, 0.576, 0.608, 0.64, 0.68, 0.72, 0.76];
const SKILL_HOLD: [f64; 15] = [0.8, 0.86, 0.92, 1.0, 1.06, 1.12, 1.2, 1.28, 1.36, 1.44, 1.52, 1.6, 1.7, 1.8, 1.9];

const SKILL_HOLD_HITMARK: i32 = 48;
static SKILL_HOLD_FRAMES: FrameTable = set(set(abil_slice(96), Action::Dash, 55), Action::Jump, 55);

/// The Jade Shield shreds every element; these are the ones this team's
/// attacks have.
const SHRED: [Element; 4] = [Element::Pyro, Element::Hydro, Element::Geo, Element::Physical];

impl Sim {
    fn zl_a4_skill(&self, ci: usize) -> f64 {
        if self.chars[ci].ascension < 4 {
            return 0.0;
        }
        0.019 * self.max_hp(ci)
    }

    pub fn zl_skill(&mut self, ci: usize, hold: i32) -> ActInfo {
        assert!(hold > 0, "only the hold skill is ported");
        let lvl = self.talent(ci, 1);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, IcdTag::ElementalArt, IcdGroup::Default, Element::Geo, 25.0, SKILL_HOLD[lvl]);
        ai.abil = "Stone Stele (Hold)";
        ai.flat_dmg = self.zl_a4_skill(ci);
        self.queue_attack(ai, 0, 0, SKILL_HOLD_HITMARK);
        let zl = &self.chars[ci].zl;
        if zl.stele_count < zl.max_stele {
            self.add_task(Task::ZlNewStele { c: ci as u8 }, SKILL_HOLD_HITMARK);
        }
        self.add_task(Task::ZlJadeShield { c: ci as u8 }, SKILL_HOLD_HITMARK);
        self.set_cd_with_delay(ci, Action::Skill, 720, 47);
        ActInfo::new(Frames::Table(&SKILL_HOLD_FRAMES), SKILL_HOLD_FRAMES[0], SKILL_HOLD_FRAMES[Action::Dash as usize], AnimState::Skill)
    }

    pub fn zl_new_stele(&mut self, ci: usize) {
        let lvl = self.talent(ci, 1);
        let flat = self.zl_a4_skill(ci);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, IcdTag::ElementalArt, IcdGroup::Default, Element::Geo, 50.0, SKILL[lvl]);
        ai.abil = "Stone Stele (Initial)";
        ai.flat_dmg = flat;
        self.queue_attack(ai, CB_ZL_PARTICLE, 0, 0);

        let f = self.f;
        let zl = &mut self.chars[ci].zl;
        if zl.stele.is_some() && zl.stele_count == zl.max_stele {
            // replaced: the old one is destroyed
            if zl.stele_count > 0 {
                zl.stele_count -= 1;
            }
        }
        zl.stele = Some((f, f + 1860));
        zl.stele_count = 1;

        let mut tick = AttackInfo::new(ci, AttackTag::ElementalArt, IcdTag::ElementalArt, IcdGroup::Default, Element::Geo, 25.0, SKILL_TICK[lvl]);
        tick.abil = "Stone Stele (Tick)";
        tick.flat_dmg = flat;
        let (stats, clvl) = self.make_snapshot(&mut tick);
        let mut ev = AttackEvent::new(tick, 0);
        ev.stats = stats;
        ev.char_lvl = clvl;
        self.chars[ci].zl.stele_snap = ev;
        self.add_task(Task::ZlResonance { c: ci as u8, src: f }, 120);
    }

    pub fn zl_resonance(&mut self, ci: usize, src: i32) {
        match self.chars[ci].zl.stele {
            Some((s, _)) if s == src => {}
            _ => return,
        }
        let ev = self.chars[ci].zl.stele_snap;
        self.queue_attack_with_snap(ev, CB_ZL_PARTICLE, 0);
        self.add_task(Task::ZlResonance { c: ci as u8, src }, 120);
    }

    pub fn zl_particle_cb(&mut self, ci: usize) {
        if self.mod_active(ci, ModKey::ZhongliParticleIcd) {
            return;
        }
        self.add_status(ci, ModKey::ZhongliParticleIcd, 90, true);
        if self.rng.float64() < 0.5 {
            self.queue_particle(1.0, Element::Geo, self.particle_delay[ci]);
        }
    }

    pub fn zl_jade_shield(&mut self, ci: usize) {
        let f = self.f;
        let zl = &mut self.chars[ci].zl;
        zl.shielded = true;
        zl.shield_expiry = f + 1200;
        // gcsim queues a shred every 18 frames up to 1200 at once; a chain
        // of tasks runs on the same frames
        self.zl_shred(ci, 0);
    }

    pub fn zl_shred(&mut self, ci: usize, i: i32) {
        if self.chars[ci].zl.shielded {
            for (j, e) in SHRED.iter().enumerate() {
                self.enemy_add_resist(ModKey::ZhongliRes(j as u8), *e, -0.2, 60);
            }
        }
        if i + 18 <= 1200 {
            self.add_task(Task::ZlShred { c: ci as u8, i: i + 18 }, 18);
        }
    }
}
