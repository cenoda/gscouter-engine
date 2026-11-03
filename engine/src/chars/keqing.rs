//! Keqing, from gcsim's internal/characters/keqing: normal attacks (N5 a
//! bullet on the enemy; N1's charged-attack cancel in normalized time) and
//! charged attacks (25 stamina, Thunderclap Slash on the Lightning
//! Stiletto), Stellar Restoration (the stiletto, the recast whatever the
//! cooldown while it lasts, C1's hits), Starward Sword, A1's electro
//! infusion, A4 and every constellation.

use crate::chars::plunge;
use crate::content::{Character, Event};
use crate::gcsimdata::chars::keqing as g;
use crate::sim::*;
use crate::types::*;

pub struct Char;

#[derive(Default)]
pub struct State {}

const STILETTO: ModKey = ModKey::new(g::STILETTO_KEY);
const PARTICLE_ICD: ModKey = ModKey::new(g::PARTICLE_ICD_KEY);
const INFUSE_STATUS: ModKey = ModKey::new("keqinginfuse");
const INFUSE: ModKey = ModKey::new("keqing-a1");
const A4: ModKey = ModKey::new("keqing-a4");
const C2_ICD: ModKey = ModKey::new(g::C2_ICD_KEY);
const C4: ModKey = ModKey::new("keqing-c4");
const C6_ATTACK: ModKey = ModKey::new("keqing-c6-attack");
const C6_SKILL: ModKey = ModKey::new("keqing-c6-skill");
const C6_CHARGE: ModKey = ModKey::new("keqing-c6-charge");
const C6_BURST: ModKey = ModKey::new("keqing-c6-burst");

const CB_PARTICLE: u16 = 1 << 0;
const CB_C2: u16 = 1 << 1;

const NORMAL_NAMES: [&str; 5] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4"];
const CHARGE_NAMES: [&str; 2] = ["Charge 0", "Charge 1"];

const PLUNGE: plunge::Spec = plunge::Spec::physical(g::LOW_PLUNGE_HITMARK, g::HIGH_PLUNGE_HITMARK, g::COLLISION_HITMARK, &g::LOW_PLUNGE_FRAMES, &g::HIGH_PLUNGE_FRAMES, g::LOW_PLUNGE_POISE_DMG, g::HIGH_PLUNGE_POISE_DMG, g::LOW_PLUNGE_RADIUS, g::HIGH_PLUNGE_RADIUS, g::LOW_PLUNGE, g::HIGH_PLUNGE, g::COLLISION);

impl Character for Char {
    const KEY: &'static str = "keqing";
    type State = State;

    fn new_state(_: &CharSetup) -> State {
        State::default()
    }

    fn init(s: &mut Sim, ci: usize) {
        if s.chars[ci].cons >= 4 {
            s.subscribe(EventKind::Reaction, Owner::Char(ci as u8));
        }
    }

    /// gcsim validates collision on the plunges and reads no params of the
    /// other actions.
    fn unsupported(a: Action, p: &Params) -> Option<String> {
        match a {
            Action::Attack | Action::Charge | Action::Skill | Action::Burst => None,
            Action::LowPlunge | Action::HighPlunge => plunge::unknown_param(a, p, &[]),
            _ => Some(crate::gcsl::action_name(a).into()),
        }
    }

    fn invalid(s: &Sim, _: usize, a: Action, _: &Params) -> Option<String> {
        plunge::grounded(s, a).or_else(|| crate::content::template_invalid(s, a))
    }

    /// The recast while the stiletto lasts, whatever the cooldown.
    fn action_ready(s: &Sim, _: usize, a: Action, _: &Params) -> Option<Option<Failure>> {
        (a == Action::Skill && s.core_status_key(STILETTO) > 0).then_some(None)
    }

    fn action_stam(_: &Sim, _: usize, a: Action) -> Option<f64> {
        (a == Action::Charge).then_some(25.0)
    }

    fn attack(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let n = s.chars[ci].normal_counter as usize;
        let lvl = s.talent(ci, 0);
        let c2 = if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
        for (i, mult) in g::ATTACK[n].iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Normal, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = NORMAL_NAMES[n];
            ai.hitlag_factor = 0.01;
            ai.hitlag_halt = g::ATTACK_HITLAG_HALT_FRAME[n][i] * 60.0;
            ai.def_halt = g::ATTACK_DEF_HALT[n][i];
            let hb = g::ATTACK_HITBOXES[n];
            let oy = g::ATTACK_OFFSETS[n];
            ai.pattern = if n == 0 {
                s.box_on_player(0.0, oy, hb[0], hb[1])
            } else if n == 4 {
                // N5 is a bullet, on the enemy
                s.circle_toward_target(0.0, oy, hb[0])
            } else {
                s.circle_on_player(0.0, oy, hb[0])
            };
            s.queue_attack_at(ci, ai, c2, g::ATTACK_HITMARKS[n][i]);
        }
        if s.chars[ci].cons >= 6 {
            c6(s, ci, C6_ATTACK);
        }
        let atkspd = s.stat(ci, st::ATKSPD);
        let table = &g::ATTACK_FRAMES[n];
        let marks = g::ATTACK_HITMARKS[n];
        let mut info = ActInfo::new(Frames::Attack { table, atkspd }, table[0], marks[marks.len() - 1], AnimState::Normal);
        if n == 0 {
            info.norm_for = Some(Action::Charge);
        }
        s.advance_normal(ci);
        info
    }

    fn charge(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 0);
        let c2 = if s.chars[ci].cons >= 2 { CB_C2 } else { 0 };
        for (i, mult) in g::CHARGE.iter().enumerate() {
            let mut ai = AttackInfo::new(ci, AttackTag::Extra, icd::TAG_NORMAL_ATTACK, icd::GROUP_DEFAULT, Element::Physical, 25.0, mult[lvl]);
            ai.abil = CHARGE_NAMES[i];
            ai.pattern = s.circle_on_player(0.0, g::CHARGE_OFFSETS[i], g::CHARGE_RADIUS[i]);
            let h = g::CHARGE_HITMARKS[i];
            s.queue_attack(ai, c2, h, h);
        }
        if s.core_status_key(STILETTO) > 0 {
            s.delete_core_status(STILETTO);
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Electro, 50.0, g::SKILL_CA[s.talent(ci, 1)]);
            ai.abil = "Thunderclap Slash";
            for &h in g::CHARGE_HITMARKS {
                ai.pattern = s.circle_on_player(0.0, 0.0, 2.5);
                s.queue_attack(ai, CB_PARTICLE, h, h);
            }
        }
        if s.chars[ci].cons >= 6 {
            c6(s, ci, C6_CHARGE);
        }
        let t = &g::CHARGE_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], g::CHARGE_HITMARKS[g::CHARGE_HITMARKS.len() - 1], AnimState::Charge)
    }

    fn skill(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        let lvl = s.talent(ci, 1);
        if s.core_status_key(STILETTO) > 0 {
            // skillRecast
            if s.chars[ci].cons >= 1 {
                let mut ai = AttackInfo::new(ci, AttackTag::ElementalArtHold, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Electro, 25.0, 0.5);
                ai.abil = "Stellar Restoration (C1)";
                ai.pattern = s.circle_on_player(0.0, 0.0, 2.0);
                s.queue_attack(ai, 0, 3, 3);
                ai.pattern = s.circle_toward_target(0.0, 1.5, 2.0);
                s.queue_attack(ai, 0, g::SKILL_RECAST_HITMARK, g::SKILL_RECAST_HITMARK);
            }
            let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_ELEMENTAL_ART, icd::GROUP_DEFAULT, Element::Electro, 50.0, g::SKILL_PRESS[lvl]);
            ai.abil = "Stellar Restoration (Slashing)";
            ai.hitlag_halt = 0.09 * 60.0;
            ai.hitlag_factor = 0.01;
            ai.pattern = s.circle_on_player(0.0, 1.0, 3.0);
            s.queue_attack(ai, CB_PARTICLE, g::SKILL_RECAST_HITMARK, g::SKILL_RECAST_HITMARK);
            // a1
            if s.chars[ci].ascension >= 1 {
                let dur = 5 * 60 + 20;
                s.add_status(ci, INFUSE_STATUS, dur, true);
                s.add_infusion(ci, INFUSE, Element::Electro, dur, true, &[AttackTag::Normal, AttackTag::Extra, AttackTag::Plunge]);
            }
            s.delete_core_status(STILETTO);
            let t = &g::SKILL_RECAST_FRAMES;
            return ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill);
        }
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalArt, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::SKILL[lvl]);
        ai.abil = "Stellar Restoration";
        ai.pattern = s.circle_toward_target(0.0, 0.0, 1.6);
        s.queue_attack(ai, 0, g::SKILL_HITMARK, g::SKILL_HITMARK);
        if s.chars[ci].cons >= 6 {
            c6(s, ci, C6_SKILL);
        }
        s.add_core_status(STILETTO, 5 * 60 + 20);
        s.set_cd_with_delay(ci, Action::Skill, 7 * 60 + 30, 20);
        let t = &g::SKILL_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Dash as usize], AnimState::Skill)
    }

    fn burst(s: &mut Sim, ci: usize, _: &Params) -> ActInfo {
        // a4
        if s.chars[ci].ascension >= 4 {
            s.add_mod(ci, Mod::owned(A4, ModKind::Stat, Owner::Char(ci as u8), st::NONE, false, true, -1), 480);
        }
        let lvl = s.talent(ci, 2);
        let mut ai = AttackInfo::new(ci, AttackTag::ElementalBurst, icd::TAG_ELEMENTAL_BURST, icd::GROUP_DEFAULT, Element::Electro, 25.0, g::BURST_INITIAL[lvl]);
        ai.abil = "Starward Sword (Initial)";
        ai.pattern = s.circle_on_player(0.0, 0.0, 8.0);
        s.queue_attack(ai, 0, g::BURST_HITMARK, g::BURST_HITMARK);
        ai.abil = "Starward Sword (Consecutive Slash)";
        ai.mult = g::BURST_DOT[lvl];
        let mut i = 82;
        while i < 162 {
            s.queue_attack(ai, 0, i, i);
            i += 11;
        }
        ai.abil = "Starward Sword (Last Attack)";
        ai.mult = g::BURST_FINAL[lvl];
        s.queue_attack(ai, 0, 197, 197);
        if s.chars[ci].cons >= 6 {
            c6(s, ci, C6_BURST);
        }
        s.consume_energy(ci, 55);
        s.set_cd_with_delay(ci, Action::Burst, 720, 52);
        let t = &g::BURST_FRAMES;
        ActInfo::new(Frames::Table(t), t[0], t[Action::Swap as usize], AnimState::Burst)
    }

    fn low_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, false, p, &PLUNGE, 0)
    }

    fn high_plunge(s: &mut Sim, ci: usize, p: &Params) -> ActInfo {
        plunge::standard(s, ci, true, p, &PLUNGE, 0)
    }

    fn attack_cb(s: &mut Sim, ci: usize, cbs: u16, _: &AttackEvent) {
        if cbs & CB_PARTICLE != 0 && !s.mod_active(ci, PARTICLE_ICD) {
            s.add_status(ci, PARTICLE_ICD, 36, true);
            let count = if s.rng.float64() < 0.5 { 3.0 } else { 2.0 };
            s.queue_particle(count, Element::Electro, s.particle_delay[ci]);
        }
        // c2: on an electro-affected enemy while she is on the field, half
        // the time, once every 5 s
        if cbs & CB_C2 != 0 && s.player.active as usize == ci && s.enemy_aura_contains(Element::Electro) && !s.mod_active(ci, C2_ICD) && s.rng.float64() < 0.5 {
            s.add_status(ci, C2_ICD, 5 * 60, true);
            s.queue_particle(1.0, Element::Electro, s.particle_delay[ci]);
        }
    }

    /// A4: CRIT Rate and ER +15%.
    fn mod_amount(_: &Sim, _: usize, _: usize, m: &Mod, out: &mut Stats) -> bool {
        if m.key != A4 {
            return false;
        }
        out[st::CR] += 0.15;
        out[st::ER] += 0.15;
        true
    }

    /// C4: ATK +25% for 10 s after her electro reactions.
    fn event(s: &mut Sim, ci: usize, e: &Event) {
        let Event::Reaction(r, ev) = *e else { return };
        let electro = matches!(
            r,
            Reaction::Overload
                | Reaction::ElectroCharged
                | Reaction::LunarCharged
                | Reaction::Superconduct
                | Reaction::StellarConduct
                | Reaction::SwirlElectro
                | Reaction::CrystallizeElectro
                | Reaction::Quicken
                | Reaction::Aggravate
                | Reaction::Hyperbloom
        );
        if !electro || ev.info.actor as usize != ci {
            return;
        }
        s.add_mod(ci, Mod::stat(C4, st::ATKP, 0.25, false, true, -1), 600);
    }
}

/// c6: Electro DMG +6% for 8 s, one mod for each kind of action.
fn c6(s: &mut Sim, ci: usize, key: ModKey) {
    s.add_mod(ci, Mod::stat(key, st::ELECTROP, 0.06, false, true, -1), 480);
}
