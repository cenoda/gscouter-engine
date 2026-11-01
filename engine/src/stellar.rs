//! Stellar-Conduct and the Polestar Field, from gcsim's
//! pkg/reactable/stellarconduct.go and stellarconduct_gadget.go, and the
//! direct Stellar-Conduct DMG of pkg/enemy/damage.go.
//!
//! While a character that turns superconduct into Stellar-Conduct is in
//! the team (gcsim's Flags.Custom[StellarConductEnableKey], set in its
//! Init), superconduct (frozen or not) reacts the same way on the auras but
//! deals no damage and shreds nothing: it emits OnStellarConduct and makes
//! a Polestar Field at the player, or gives the one the player stands in
//! its 6 s again. The field records a stack (at most 12) for every electro
//! or cryo element applied to an enemy in it, once every 0.1 s per
//! character; on appearing and every 4 s after, the recorded stacks become
//! the field's, the enemies in it lose 40% physical RES for 4 s and, with
//! the player in it, every character gets the cryo and electro DMG bonus of
//! those stacks, the Polestar Field status (Radiance: Stellar-Conduct, read
//! by the characters that have it) and the stacks as a tag, which scales
//! that character's direct Stellar-Conduct DMG.
//!
//! gcsim subscribes the recording hook when the first field appears and
//! unsubscribes it only on a Kill, which nothing does: the field expires,
//! and the hook goes on recording stacks for the next field. One hook only
//! ever exists, as it is keyed by the field's position, the player's.
//!
//! The player stands at the origin and never moves, so the player is in
//! every field and there is only ever one. (With a player position, the
//! field keeps the position the player had when it appeared: gcsim's
//! gadget.New at Player().Pos(), its area NewCircleHitOnTarget(field, nil,
//! 15); the player's and the enemy's IsWithinArea are points in it.) The field is not among
//! `Sim::gadgets`: nothing reaches it but these hooks (its HandleAttack
//! does nothing, it has no aura and is not of the enemy camp); it ticks
//! after them. It takes no gadget key, which only orders the others.
//!
//! Stellar Swirl (gcsim's StellarSwirlEnableKey, which every character
//! that enables Stellar-Conduct enables too) is not ported: a cryo or
//! frozen swirl while it is on is an error, and so is an anemo character
//! in a team that enables it (`profile::config`).

use crate::chars::CharKind;
use crate::content::Event;
use crate::sim::*;
use crate::types::*;

/// gcsim's PolestarFieldKey: the status of Radiance: Stellar-Conduct.
pub const POLESTAR_FIELD: ModKey = ModKey::new("polestar-field");
const FIELD_CRYO: ModKey = ModKey::new("polestar-field-cryo");
const FIELD_ELECTRO: ModKey = ModKey::new("polestar-field-electro");
const FIELD_ICD: ModKey = ModKey::new("polestar-field-icd");
const SHRED: ModKey = ModKey::new("polestar-field-phys-shred");

const FIELD_DUR: i32 = 6 * 60;
const STACK_ICD: i32 = 6;
const MAX_STACKS: i32 = 12;
const THINK_INTERVAL: i32 = 4 * 60;
const BUFF_DUR: i32 = THINK_INTERVAL + 1;
/// The field's area: a circle of radius 15 on it.
const FIELD_R: f64 = 15.0;

/// gcsim's stellarConductBuff: the cryo and electro DMG bonus by stacks.
const BUFF: [f64; 13] = [0.2, 0.29, 0.3, 0.31, 0.32, 0.33, 0.34, 0.35, 0.36, 0.37, 0.38, 0.39, 0.4];
/// gcsim's StellarconductMult: direct Stellar-Conduct DMG by the
/// attacker's stacks tag.
pub const MULT: [f64; 13] = [1.0, 1.45, 1.5, 1.55, 1.6, 1.65, 1.7, 1.75, 1.8, 1.85, 1.9, 1.95, 2.0];

/// Whether gcsim's Init of this character sets StellarConductEnableKey
/// (and StellarSwirlEnableKey).
pub fn enables_stellar(k: CharKind) -> bool {
    matches!(k, CharKind::Sandrone)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Field {
    /// Ticks since it appeared.
    timer: i32,
    /// Ticks left.
    duration: i32,
}

/// The fight's Stellar state.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stellar {
    /// Superconduct becomes Stellar-Conduct (StellarConductEnableKey).
    pub conduct: bool,
    /// Cryo swirls become Stellar Swirl (StellarSwirlEnableKey).
    pub swirl: bool,
    pub field: Option<Field>,
    /// The recording hook exists.
    hooked: bool,
    /// gcsim's polestar-field-recorded flag.
    record: i32,
    /// gcsim's polestar-field-stacks flag.
    pub stacks: i32,
    /// Each character's polestar-field-stacks tag.
    pub tags: [i32; 4],
}

impl Sim {
    /// Whether the enemy's center is in the field's circle (gcsim's
    /// IsWithinArea, a point in the shape).
    fn enemy_in_field(&self) -> bool {
        self.enemy_alive && self.target_dist * self.target_dist <= FIELD_R * FIELD_R
    }

    /// gcsim's OnElementApplied hook of the field, at the start of the
    /// enemy's React.
    #[inline]
    pub(crate) fn polestar_element_applied(&mut self, ev: &AttackEvent) {
        if !self.stellar.hooked || !self.enemy_in_field() {
            return;
        }
        if !matches!(ev.info.element, Element::Electro | Element::Cryo) || ev.info.durability <= ZERO_DUR {
            return;
        }
        let ci = ev.info.actor as usize;
        if self.mod_active(ci, FIELD_ICD) {
            return;
        }
        self.add_status(ci, FIELD_ICD, STACK_ICD, true);
        self.stellar.record = (self.stellar.record + 1).min(MAX_STACKS);
    }

    /// gcsim's queueStellarConduct: the event, then a new field or the
    /// current one's duration again.
    pub(crate) fn queue_stellar_conduct(&mut self, ev: &AttackEvent) {
        self.emit(Event::Reaction(Reaction::StellarConduct, ev));
        match &mut self.stellar.field {
            Some(f) => f.duration = FIELD_DUR,
            None => {
                self.stellar.hooked = true;
                self.polestar_refresh();
                self.stellar.field = Some(Field { timer: 0, duration: FIELD_DUR });
            }
        }
    }

    /// gcsim's refreshBuffs.
    fn polestar_refresh(&mut self) {
        let stacks = self.stellar.record;
        self.stellar.stacks = stacks;
        self.stellar.record = 0;
        if self.enemy_in_field() {
            self.enemy_add_resist(SHRED, Element::Physical, -0.4, BUFF_DUR);
        }
        // the player is always in the field
        let buff = BUFF[stacks as usize];
        for i in 0..self.chars.len() {
            self.add_stat_mod(i, FIELD_CRYO, st::CRYOP, buff, BUFF_DUR, true);
            self.add_stat_mod(i, FIELD_ELECTRO, st::ELECTROP, buff, BUFF_DUR, true);
            self.add_status(i, POLESTAR_FIELD, BUFF_DUR, true);
            self.stellar.tags[i] = stacks;
        }
    }

    /// The field's Gadget.Tick: its think every 4 s, then its expiry.
    pub(crate) fn tick_polestar(&mut self) {
        let Some(f) = &mut self.stellar.field else { return };
        f.timer += 1;
        let think = f.timer % THINK_INTERVAL == 0;
        f.duration -= 1;
        let expired = f.duration == 0;
        if think {
            self.polestar_refresh();
        }
        if expired {
            self.stellar.field = None;
        }
    }

    /// gcsim's calcDirectReaction, for direct Stellar-Conduct DMG: no DMG
    /// bonus; the multiplier of the attacker's stacks tag, the EM bonus of
    /// special reactions and its reaction bonuses, then flat damage, DEF,
    /// RES, the damage group and elevation, and the crit last.
    pub(crate) fn calc_direct(&mut self, ev: &mut AttackEvent, grp: f64, base_bonus: f64) -> (f64, bool) {
        let actor = ev.info.actor as usize;
        let s = &ev.stats;
        let a = if ev.info.use_def { total_def(s) } else { total_atk(s) };
        let mut damage = ev.info.mult * a * (1.0 + base_bonus);
        damage *= MULT[self.stellar.tags[actor] as usize];
        let em = s[st::EM];
        let em_bonus = (6.0 * em) / (2000.0 + em);
        let react_bonus = self.react_bonus(actor, &ev.info);
        damage *= 1.0 + em_bonus + react_bonus;
        damage += ev.info.flat_dmg;
        let (res, defadj) = self.enemy_resist_def(ev.info.element);
        let defadj = defadj.min(0.9);
        let defmod = (ev.char_lvl + 100) as f64 / ((ev.char_lvl + 100) as f64 + (self.enemy.level + 100) as f64 * (1.0 + defadj) * (1.0 - ev.info.ignore_def.min(1.0)));
        damage *= defmod;
        damage *= resmod(res);
        damage *= grp;
        damage *= 1.0 + ev.elevation as f64 / 100.0;
        let s = &mut ev.stats;
        s[st::CR] = s[st::CR].clamp(0.0, 1.0);
        let mut crit = false;
        if self.rng.float64() <= s[st::CR] {
            damage *= 1.0 + s[st::CD];
            crit = true;
        }
        (damage, crit)
    }
}

/// gcsim's RES multiplier.
#[inline]
pub fn resmod(res: f64) -> f64 {
    if res < 0.0 {
        1.0 - res / 2.0
    } else if res < 0.75 {
        1.0 - res
    } else {
        1.0 / (4.0 * res + 1.0)
    }
}
