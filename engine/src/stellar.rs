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
//! Each field stays where the player was when it appeared (gcsim's
//! gadget.New at Player().Pos()); its area is a circle of radius 15 on it
//! (NewCircleHitOnTarget(field, nil, 15)), and the enemy and the player are
//! in it when their centers are (IsWithinArea, a point in the shape). A
//! Stellar-Conduct renews the first field the player stands in, else makes
//! a new one, so a player who moved can have several.
//!
//! gcsim subscribes a field's recording hook when it appears, keyed by its
//! position, and unsubscribes it only on a Kill, which nothing does: the
//! field expires, and its hook goes on recording stacks for the next
//! field. A field at a position that already has a hook replaces it in
//! place. The hooks share the record and the per-character ICD, so an
//! element counts once when the enemy is in any hook's area.
//!
//! The fields are not among `Sim::gadgets`: nothing reaches them but these
//! hooks (their HandleAttack does nothing, they have no aura and are not of
//! the enemy camp); they tick after the gadgets, in the order they came.
//! They take no gadget key, which only orders the others.
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

#[derive(Clone, Copy, Debug)]
pub struct Field {
    /// Ticks since it appeared.
    timer: i32,
    /// Ticks left.
    duration: i32,
    /// Where it is.
    x: f64,
    y: f64,
}

/// A field's area: the circle of radius 15 on its position.
fn area(x: f64, y: f64) -> Pattern {
    Pattern::Circle { x, y, r: FIELD_R }
}

/// The fight's Stellar state.
#[derive(Clone, Debug, Default)]
pub struct Stellar {
    /// Superconduct becomes Stellar-Conduct (StellarConductEnableKey).
    pub conduct: bool,
    /// Cryo swirls become Stellar Swirl (StellarSwirlEnableKey).
    pub swirl: bool,
    /// The fields, in the order they came.
    pub fields: Vec<Field>,
    /// The positions of the recording hooks, in the order subscribed.
    hooks: Vec<(f64, f64)>,
    /// gcsim's polestar-field-recorded flag.
    record: i32,
    /// gcsim's polestar-field-stacks flag.
    pub stacks: i32,
    /// Each character's polestar-field-stacks tag.
    pub tags: [i32; 4],
}

impl Sim {
    /// gcsim's OnElementApplied hooks of the fields, at the start of the
    /// enemy's React.
    #[inline]
    pub(crate) fn polestar_element_applied(&mut self, ev: &AttackEvent) {
        if self.stellar.hooks.is_empty() {
            return;
        }
        if !matches!(ev.info.element, Element::Electro | Element::Cryo) || ev.info.durability <= ZERO_DUR {
            return;
        }
        // the first hook whose area holds the enemy counts it; its ICD stops
        // the others
        if !self.stellar.hooks.iter().any(|&(x, y)| self.enemy_within(area(x, y))) {
            return;
        }
        let ci = ev.info.actor as usize;
        if self.mod_active(ci, FIELD_ICD) {
            return;
        }
        self.add_status(ci, FIELD_ICD, STACK_ICD, true);
        self.stellar.record = (self.stellar.record + 1).min(MAX_STACKS);
    }

    /// gcsim's queueStellarConduct: the event, then the duration of the
    /// first field the player is in again, or a new field at the player.
    pub(crate) fn queue_stellar_conduct(&mut self, ev: &AttackEvent) {
        self.emit(Event::Reaction(Reaction::StellarConduct, ev));
        if let Some(k) = self.stellar.fields.iter().position(|f| self.player_within(area(f.x, f.y))) {
            self.stellar.fields[k].duration = FIELD_DUR;
            return;
        }
        let (x, y) = self.player_pos;
        if !self.stellar.hooks.contains(&(x, y)) {
            self.stellar.hooks.push((x, y));
        }
        self.polestar_refresh(x, y);
        self.stellar.fields.push(Field { timer: 0, duration: FIELD_DUR, x, y });
    }

    /// gcsim's refreshBuffs of the field at (x, y).
    fn polestar_refresh(&mut self, x: f64, y: f64) {
        let stacks = self.stellar.record;
        self.stellar.stacks = stacks;
        self.stellar.record = 0;
        if self.enemy_alive && self.enemy_within(area(x, y)) {
            self.enemy_add_resist(SHRED, Element::Physical, -0.4, BUFF_DUR);
        }
        if !self.player_within(area(x, y)) {
            return;
        }
        let buff = BUFF[stacks as usize];
        for i in 0..self.chars.len() {
            self.add_stat_mod(i, FIELD_CRYO, st::CRYOP, buff, BUFF_DUR, true);
            self.add_stat_mod(i, FIELD_ELECTRO, st::ELECTROP, buff, BUFF_DUR, true);
            self.add_status(i, POLESTAR_FIELD, BUFF_DUR, true);
            self.stellar.tags[i] = stacks;
        }
    }

    /// Each field's Gadget.Tick: its think every 4 s, then its expiry.
    pub(crate) fn tick_polestar(&mut self) {
        let mut k = 0;
        while k < self.stellar.fields.len() {
            let f = &mut self.stellar.fields[k];
            f.timer += 1;
            let think = f.timer % THINK_INTERVAL == 0;
            f.duration -= 1;
            let expired = f.duration == 0;
            let (x, y) = (f.x, f.y);
            if think {
                self.polestar_refresh(x, y);
            }
            if expired {
                self.stellar.fields.remove(k);
            } else {
                k += 1;
            }
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
