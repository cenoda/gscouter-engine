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
//! Stellar Swirl, from gcsim's pkg/reactable/stellarswirl.go and
//! stellarswirl_gadget.go. While a character that enables it is in the
//! team (StellarSwirlEnableKey: Sandrone, Vesna, the cryo Traveler), an anemo hit on the
//! enemy's cryo or frozen aura reacts as Stellar Swirl instead of a cryo
//! swirl: it emits OnStellarSwirl, gives every character Radiance: Stellar
//! Swirl for 8 s (not hitlag), hits the enemy 3 frames later with the
//! contributors' damage (the attacker and every character with durability
//! in that aura; `Sim::special_reaction_attack`, times 0.75), records the
//! contributors and the attacker as the owner, adds a stack (at most 6)
//! and makes a Stellar Vortex at the player if there is none; at 6 stacks
//! the vortex is killed. Only then does the aura lose its durability, half
//! the attack's, with no swirl GCD and no aoe. The vortex lives 181 frames
//! and explodes when killed or when it expires: the owner's cryo
//! detonation, its contributors' damage times 2 or 3 by stacks, on the
//! enemy if a circle of 6 or 8 around the vortex reaches it; the stacks and
//! contributors are cleared, and the jump gets airborne for 5 s
//! (`player.StellarSwirlAirborneBuff`). A detonation that reaches a player
//! jumping from the ground (gcsim's boosted jump) is an error, and so is a
//! cryo swirl on the player's own aura. Like the fields, the vortex is not
//! among `Sim::gadgets` and ticks after the fields.
//!
//! With Vodyanitsa's A1 in the team (gcsim's WanderingVortexKey), every
//! Stellar Swirl hands her the vortex it made or stacked (OnStellarSwirlVortex,
//! before a sixth stack detonates it), and every detonation its area
//! (OnStellarVortexDetonate, after the template's boosted jump); each
//! vortex has an id for her to tell them apart.

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

/// Whether gcsim's Init of this character sets StellarConductEnableKey.
pub fn enables_stellar(k: CharKind) -> bool {
    matches!(k, CharKind::Sandrone | CharKind::Odette | CharKind::LumineCryo | CharKind::AetherCryo)
}

/// Whether gcsim's Init of this character sets StellarSwirlEnableKey.
pub fn enables_stellar_swirl(k: CharKind) -> bool {
    matches!(k, CharKind::Sandrone | CharKind::Vesna | CharKind::Odette | CharKind::LumineCryo | CharKind::AetherCryo)
}

/// gcsim's RadianceSwirlKey: the status of Radiance: Stellar Swirl.
pub const RADIANCE_SWIRL: ModKey = ModKey::new("radiance-stellar-swirl");
/// gcsim's player.StellarSwirlAirborneBuff, a core status.
pub const SSW_AIRBORNE: ModKey = ModKey::new("ssw-airborne-buff");
const SSW_MAX_STACKS: i32 = 6;
const SSW_DURATION: i32 = 181;
/// gcsim's sswStackMult and sswStackRadius: the detonation by stacks.
const SSW_STACK_MULT: [f64; 7] = [0.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0];
const SSW_STACK_RADIUS: [f64; 7] = [0.0, 6.0, 6.0, 8.0, 8.0, 8.0, 8.0];

/// gcsim's StellarVortex.
#[derive(Clone, Copy, Debug)]
pub struct Vortex {
    /// Its id, from 1.
    id: u32,
    /// Ticks left.
    duration: i32,
    /// Where it is.
    x: f64,
    y: f64,
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
    /// gcsim's stellar-swirl-contribution-N flags, one bit per character.
    swirl_contrib: u8,
    /// gcsim's stellar-swirl-owner flag.
    swirl_owner: u8,
    /// gcsim's stellar-swirl-stacks flag.
    swirl_stacks: i32,
    /// The Stellar Vortex, if there is one.
    pub vortex: Option<Vortex>,
    /// The last vortex id given.
    vortex_ids: u32,
    /// Vodyanitsa's slot, when her A1 follows the vortices
    /// (WanderingVortexKey and her subscriptions).
    pub wandering: Option<u8>,
    /// Odette's slot, when her C6 elevates direct Stellar hits (her
    /// OnApplyAttack subscription).
    pub odette_c6: Option<u8>,
}

impl Stellar {
    /// Where the Stellar Vortex is, if there is one.
    pub fn vortex_pos(&self) -> Option<(f64, f64)> {
        self.vortex.map(|v| (v.x, v.y))
    }
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

    /// gcsim's tryStellarSwirl on the enemy's cryo or frozen aura (aura
    /// `m`), once try_swirl's checks passed.
    pub(crate) fn try_stellar_swirl(&mut self, ev: &mut AttackEvent, m: usize) {
        ev.reacted = true;
        self.emit(Event::Reaction(Reaction::StellarSwirl, ev));
        self.queue_stellar_swirl(ev.info.actor as usize, m);
        let rd = self.enemy.aura.reduce(m, ev.info.durability, 0.5);
        ev.info.durability -= rd;
    }

    /// gcsim's queueStellarSwirl.
    fn queue_stellar_swirl(&mut self, actor: usize, m: usize) {
        let n = self.chars.len();
        for i in 0..n {
            self.add_status(i, RADIANCE_SWIRL, 8 * 60, false);
        }
        let mut contrib = 1u8 << actor;
        for i in 0..n {
            if self.enemy.aura.dur[m][i] > ZERO_DUR {
                contrib |= 1 << i;
            }
        }
        // a circle of radius 5 on the enemy, which reaches it
        if self.enemy_alive {
            if let Some(mut ev) = self.special_reaction_attack(contrib, AttackTag::ReactionStellarSwirl, Element::Anemo, "Stellar Swirl", 1.0, 0.75, actor) {
                ev.info.icd_tag = icd::TAG_SWIRL_CRYO;
                ev.info.icd_group = icd::GROUP_REACTION_A;
                self.queue_attack_with_snap(ev, 0, 3);
            }
        }
        let sw = &mut self.stellar;
        sw.swirl_contrib |= contrib;
        sw.swirl_owner = actor as u8;
        sw.swirl_stacks = (sw.swirl_stacks + 1).min(SSW_MAX_STACKS);
        if sw.vortex.is_none() {
            sw.vortex_ids += 1;
            let id = sw.vortex_ids;
            let (x, y) = self.player_pos;
            self.stellar.vortex = Some(Vortex { id, duration: SSW_DURATION, x, y });
        }
        if let Some(v) = self.stellar.wandering {
            let id = self.stellar.vortex.unwrap().id;
            crate::chars::vodyanitsa::on_vortex(self, v as usize, id);
        }
        if self.stellar.swirl_stacks >= SSW_MAX_STACKS {
            // Kill: OnKill, then the gadget is removed
            let v = self.stellar.vortex.take().unwrap();
            self.vortex_explode(v.x, v.y);
        }
    }

    /// The vortex's explode, on a kill or on expiry.
    fn vortex_explode(&mut self, x: f64, y: f64) {
        let owner = self.stellar.swirl_owner as usize;
        let contrib = self.stellar.swirl_contrib;
        self.stellar.swirl_contrib = 0;
        let stacks = self.stellar.swirl_stacks.min(SSW_MAX_STACKS) as usize;
        let ap = Pattern::Circle { x, y, r: SSW_STACK_RADIUS[stacks] };
        if self.enemy_alive && self.hits_enemy(ap) == Some(true) {
            if let Some(mut ev) = self.special_reaction_attack(contrib, AttackTag::ReactionStellarSwirl, Element::Cryo, "Stellar Swirl Detonation", 1.0, SSW_STACK_MULT[stacks], owner) {
                ev.info.durability = 25.0;
                self.queue_attack_with_snap(ev, 0, 0);
            }
        }
        self.stellar.swirl_stacks = 0;
        self.add_core_status(SSW_AIRBORNE, 5 * 60);
        // OnStellarVortexDetonate: the template's boosted jump
        if self.current_state() == AnimState::Jump && self.player.airborne == Airborne::Grounded && self.player_within(ap) {
            self.unsupported("a Stellar Swirl detonation during a jump");
        }
        if let Some(v) = self.stellar.wandering {
            crate::chars::vodyanitsa::on_detonate(self, v as usize, ap);
        }
    }

    /// The vortex's Gadget.Tick: its expiry.
    pub(crate) fn tick_vortex(&mut self) {
        let Some(v) = &mut self.stellar.vortex else { return };
        v.duration -= 1;
        if v.duration == 0 {
            let v = self.stellar.vortex.take().unwrap();
            self.vortex_explode(v.x, v.y);
        }
    }

    /// The multiplier of direct Stellar-Conduct DMG in gcsim's
    /// calcDirectReaction (`Sim::calc_direct`): by the attacker's Polestar
    /// Field stacks tag.
    pub(crate) fn stellar_conduct_mult(&self, actor: usize) -> f64 {
        MULT[self.stellar.tags[actor] as usize]
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
