//! How characters, weapons and artifact sets attach to the fight.
//!
//! Each one is a module implementing [`Character`], [`WeaponImpl`] or
//! [`SetImpl`]: hooks with default no-ops for its actions, its tasks, the
//! callbacks of its attacks, the amounts of its mods, the attack mods it
//! adds, and the events it subscribes to. A line in `chars!`, `weapons!`
//! or `sets!` (in `chars/mod.rs`, `weapons/mod.rs`, `sets/mod.rs`)
//! registers it: the macros generate the kind enum, the enum holding each
//! one's state, and the `match` that dispatches to it, so every call is
//! static and the fight's hot path has no dynamic dispatch.

use crate::gcsl::Num;
use crate::sim::*;
use crate::types::*;

/// Who a mod's amount, a task or a subscription belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Owner {
    /// The character in this slot.
    Char(u8),
    /// The weapon of the character in this slot.
    Weapon(u8),
    /// The set at this index in the character's sets.
    Set(u8, u8),
    /// The fight itself (energy from normal attacks).
    Core,
    /// gcsim's setupNightsoulBurst: its OnEnemyDamage subscription, made
    /// after every Init when a Natlan character is on the team.
    NightsoulBurst,
    /// gcsim's "ns-extend-state-<character>" OnStateChange subscription of
    /// the character in this slot: its Nightsoul's Blessing waits for the
    /// end of an action before it expires.
    NightsoulExtend(u8),
}

/// The events of gcsim's event handler that the engine emits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum EventKind {
    EnemyDamage,
    Skill,
    Burst,
    Swap,
    StateChange,
    Reaction,
    Heal,
    Drain,
    ParticleReceived,
    EnergyBurst,
    EnemyHit,
    PlayerHit,
    Shielded,
    ShieldBreak,
    ConstructSpawned,
    HpDebt,
    Attack,
    Dash,
    Plunge,
    NightsoulBurst,
    NightsoulGenerate,
    NightsoulConsume,
    ApplyAttack,
    SpecialReactionAttack,
    DendroCore,
    TargetDied,
}
pub const N_EVENTS: usize = 26;

#[derive(Clone, Copy)]
pub enum Event<'a> {
    /// OnEnemyDamage, after the hit applied its damage.
    EnemyDamage(&'a AttackEvent),
    /// OnSkill, before the skill runs.
    Skill,
    /// OnBurst, before the burst runs.
    Burst,
    /// OnCharacterSwap.
    Swap { prev: usize, next: usize },
    /// OnStateChange to this state.
    StateChange(AnimState),
    /// A reaction event (gcsim's OnVaporize, OnCrystallizeHydro and the
    /// others) on the enemy, with the attack that caused it.
    Reaction(Reaction, &'a AttackEvent),
    /// OnHeal, once for each character healed: the healer, the heal's
    /// target (None for the whole party), the character healed, the HP it
    /// gained (after paying its Bond of Life), the overheal, and the heal
    /// before the Bond of Life took its part (gcsim's amount_before_debt).
    Heal { caller: usize, target: Option<usize>, on: usize, amount: f64, overheal: f64, before_debt: f64 },
    /// OnHPDebt: a character's Bond of Life changed; `change` is how much
    /// it went down (negative when it grew).
    HpDebt { on: usize, change: f64 },
    /// OnPlayerHPDrain: the character drained, the HP it lost, and whether
    /// it came from outside the party (gcsim's DrainInfo.External).
    Drain { on: usize, amount: f64, external: bool },
    /// OnParticleReceived, after the party took a particle.
    ParticleReceived,
    /// OnEnergyBurst: a character's burst drained amount of its energy.
    EnergyBurst { on: usize, amount: f64 },
    /// OnEnemyHit, before attack mods: a subscriber adds flat damage to
    /// the hit through `Sim::hit_flat`, and changes its crit through
    /// `Sim::hit_crit`.
    EnemyHit(&'a AttackEvent),
    /// OnPlayerHit: the active character was hit.
    PlayerHit { on: usize },
    /// OnShielded: a shield was added (or replaced one of its type).
    Shielded(&'a crate::sim::Shield),
    /// OnShieldBreak: a shield broke or expired.
    ShieldBreak(&'a crate::sim::Shield),
    /// OnConstructSpawned.
    ConstructSpawned,
    /// OnAttack, before the normal attack runs.
    Attack,
    /// OnDash, before the dash runs.
    Dash,
    /// OnPlunge, before a low or high plunge runs.
    Plunge,
    /// OnNightsoulBurst, with the attack that triggered it.
    NightsoulBurst(&'a AttackEvent),
    /// OnNightsoulGenerate: the character and the points it gained.
    NightsoulGenerate { on: usize, amount: f64 },
    /// OnNightsoulConsume: the character and the points it used.
    NightsoulConsume { on: usize, amount: f64 },
    /// OnApplyAttack: an attack is applied, before it reaches any target.
    ApplyAttack(&'a AttackEvent),
    /// OnSpecialReactionAttack: a character's contribution to a lunar
    /// reaction, snapshot and not yet computed. Subscribers change it in
    /// `Sim::special_atk` (gcsim's subscribers change the AttackEvent they
    /// are handed); the event holds it as it was before the first of them.
    SpecialReactionAttack(&'a AttackEvent),
    /// OnDendroCore: a dendro core appeared, with its gadget key.
    DendroCore { key: u32 },
    /// OnTargetDied: the enemy died (damage mode), with the attack that
    /// killed it.
    TargetDied(&'a AttackEvent),
}

impl Event<'_> {
    pub fn kind(&self) -> EventKind {
        match self {
            Event::EnemyDamage(_) => EventKind::EnemyDamage,
            Event::Skill => EventKind::Skill,
            Event::Burst => EventKind::Burst,
            Event::Swap { .. } => EventKind::Swap,
            Event::StateChange(_) => EventKind::StateChange,
            Event::Reaction(..) => EventKind::Reaction,
            Event::Heal { .. } => EventKind::Heal,
            Event::Drain { .. } => EventKind::Drain,
            Event::PlayerHit { .. } => EventKind::PlayerHit,
            Event::Shielded(_) => EventKind::Shielded,
            Event::ShieldBreak(_) => EventKind::ShieldBreak,
            Event::ConstructSpawned => EventKind::ConstructSpawned,
            Event::HpDebt { .. } => EventKind::HpDebt,
            Event::ParticleReceived => EventKind::ParticleReceived,
            Event::EnergyBurst { .. } => EventKind::EnergyBurst,
            Event::EnemyHit(_) => EventKind::EnemyHit,
            Event::Attack => EventKind::Attack,
            Event::Dash => EventKind::Dash,
            Event::Plunge => EventKind::Plunge,
            Event::NightsoulBurst(_) => EventKind::NightsoulBurst,
            Event::NightsoulGenerate { .. } => EventKind::NightsoulGenerate,
            Event::NightsoulConsume { .. } => EventKind::NightsoulConsume,
            Event::ApplyAttack(_) => EventKind::ApplyAttack,
            Event::SpecialReactionAttack(_) => EventKind::SpecialReactionAttack,
            Event::DendroCore { .. } => EventKind::DendroCore,
            Event::TargetDied(_) => EventKind::TargetDied,
        }
    }
}

/// A character's logic. `ci` is its slot in the team.
pub trait Character {
    /// gcsim's key name.
    const KEY: &'static str;
    /// The highest constellation it implements; a higher one is an error.
    const MAX_CONS: i32 = 6;
    /// gcsim makes it Hexerei (IsHexerei, in `profile::HEXEREI`) and its
    /// Hexerei bonuses, which need two Hexerei characters in the team
    /// (`Sim::hexerei_count`), are implemented. A Hexerei character
    /// without them is an error in a team of two or more.
    const HEXEREI: bool = false;
    /// gcsim's Moonsign, which its NewChar sets (1 for a Moonsign
    /// character): two or more in the team give Ascendant Gleam.
    const MOONSIGN: i32 = 0;
    type State;
    fn new_state(cs: &CharSetup) -> Self::State;

    /// gcsim's Init, after every character, weapon and set is added.
    fn init(_s: &mut Sim, _ci: usize) {}
    /// The action, as gcsl names it, if it is not implemented with these
    /// params. Swap, dash, jump and walk are the template's.
    fn unsupported(a: Action, p: &Params) -> Option<String>;
    fn attack(_s: &mut Sim, _ci: usize, _p: &Params) -> ActInfo {
        unreachable!()
    }
    fn charge(_s: &mut Sim, _ci: usize, _p: &Params) -> ActInfo {
        unreachable!()
    }
    fn skill(_s: &mut Sim, _ci: usize, _p: &Params) -> ActInfo {
        unreachable!()
    }
    fn burst(_s: &mut Sim, _ci: usize, _p: &Params) -> ActInfo {
        unreachable!()
    }
    fn aim(_s: &mut Sim, _ci: usize, _p: &Params) -> ActInfo {
        unreachable!()
    }
    fn high_plunge(_s: &mut Sim, _ci: usize, _p: &Params) -> ActInfo {
        unreachable!()
    }
    fn low_plunge(_s: &mut Sim, _ci: usize, _p: &Params) -> ActInfo {
        unreachable!()
    }
    /// gcsim's errors from an action that cannot be used now, such as a
    /// plunge on the ground: the fight stops with it.
    fn invalid(s: &Sim, _ci: usize, a: Action, _p: &Params) -> Option<String> {
        template_invalid(s, a)
    }
    /// A hook on each of its own attacks as it hits, before attack mods
    /// (as gcsim's OnEnemyHit subscribers that change the attack).
    fn own_hit(_s: &Sim, _ci: usize, _ev: &mut AttackEvent) {}
    /// A ReactBonusMod it owns: the bonus to a reaction of character `on`.
    fn react_bonus(_s: &Sim, _ci: usize, _on: usize, _m: &Mod, _info: &AttackInfo) -> f64 {
        0.0
    }
    /// Its own ActionReady, when it has one: None to use the template's,
    /// Some(None) when ready, Some(Some(why)) when not.
    fn action_ready(_s: &Sim, _ci: usize, _a: Action, _p: &Params) -> Option<Option<Failure>> {
        None
    }
    /// A task it queued with `Task::Char`.
    fn task(_s: &mut Sim, _ci: usize, _id: u8, _arg: i32) {}
    /// The callbacks (its own bits) of an attack of its that hit.
    fn attack_cb(_s: &mut Sim, _ci: usize, _cbs: u16, _ev: &AttackEvent) {}
    /// Changes to an attack of its as it is snapshot.
    fn snapshot(_s: &Sim, _ci: usize, _info: &mut AttackInfo) {}
    /// The amount of a mod it owns on character `on`; false when the mod
    /// does not apply now.
    fn mod_amount(_s: &Sim, _ci: usize, _on: usize, _m: &Mod, _out: &mut Stats) -> bool {
        false
    }
    /// An attack mod it owns, on an attack by character `on`.
    fn attack_mod(_s: &Sim, _ci: usize, _on: usize, _m: &Mod, _ev: &mut AttackEvent) {}
    fn event(_s: &mut Sim, _ci: usize, _e: &Event) {}
    /// The frames of an action of its whose ActInfo has Frames::Char.
    fn frames(_id: u8, _arg: i16, _next: Action) -> i32 {
        unreachable!()
    }
    /// An action of its with OnRemoved::Char ends, for the next state.
    fn on_removed(_s: &mut Sim, _ci: usize, _id: u8, _next: AnimState) {}
    /// The stamina of a charged attack, when it is not the class's.
    fn charge_stam(_s: &Sim, _ci: usize) -> Option<f64> {
        None
    }
    /// Its own condition fields, such as .zhongli.shielded.
    fn condition(_s: &Sim, _ci: usize, _fields: &[String]) -> Option<Result<Num, String>> {
        None
    }
    /// A construct of its expired.
    fn construct_expired(_s: &mut Sim, _ci: usize, _key: ModKey) {}
    /// A gadget of its ticked, `timer` ticks after it was added (it may
    /// have just expired and be gone).
    fn gadget_tick(_s: &mut Sim, _ci: usize, _id: u8, _timer: i32) {}
    /// An attack reached one of its gadgets added with
    /// `add_hittable_gadget`: as gcsim hands each target its own copy, the
    /// attack as it was before the enemy took it. `lands` is whether its
    /// pattern reaches the gadget, None when the attack has no pattern.
    fn gadget_hit(_s: &mut Sim, _ci: usize, _id: u8, _ev: &AttackEvent, _lands: Option<bool>) {}
    /// Its dash's ActInfo, after the template's Dash ran (a character with
    /// its own dash frames).
    fn dash_info(_s: &mut Sim, _ci: usize, info: ActInfo) -> ActInfo {
        info
    }
    /// A shield of its took damage (gcsim's OnDamage of its own shield
    /// type): the damage it was handed, and whether that broke it.
    fn shield_damaged(_s: &mut Sim, _ci: usize, _key: ModKey, _dmg: f64, _broken: bool) {}
    /// A shield of its expired (gcsim's OnExpire).
    fn shield_expired(_s: &mut Sim, _ci: usize, _key: ModKey) {}
    /// A shield bonus mod it owns: the bonus, and whether gcsim's
    /// ShieldBonus then drops the mod (its `done`).
    fn shield_bonus(_s: &Sim, _ci: usize, _key: ModKey) -> (f64, bool) {
        (0.0, false)
    }
    /// Its ActionStam, when it is not the template's (gcsim's override of
    /// it): the stamina an action costs before percentage mods.
    fn action_stam(_s: &Sim, _ci: usize, _a: Action) -> Option<f64> {
        None
    }
    /// Its AnimationStartDelay for gcsim's AnimationDelayKey `key`, when
    /// it is not the fixed one in its data.
    fn start_delay(_s: &Sim, _ci: usize, _key: usize) -> Option<i32> {
        None
    }
    /// Its own Dash, when it replaces the template's entirely (gcsim's
    /// override of Dash that does not call the template's): None for the
    /// template's.
    fn own_dash(_s: &mut Sim, _ci: usize, _p: &Params) -> Option<ActInfo> {
        None
    }
    /// Its own Jump, when it replaces the template's entirely.
    fn own_jump(_s: &mut Sim, _ci: usize, _p: &Params) -> Option<ActInfo> {
        None
    }
    /// Its own part of a dash or jump (gcsim's override of Dash or Jump),
    /// run before the template's.
    fn template_action(_s: &mut Sim, _ci: usize, _a: Action) {}
    /// Its OnPlayerPreHPDrain subscription (subscribed with
    /// `Sim::pre_drain_subs`): the amount character `on` is about to lose
    /// to a drain, which it may change. `external` and `abil` are gcsim's
    /// DrainInfo's.
    fn pre_drain(_s: &mut Sim, _ci: usize, _on: usize, amount: f64, _external: bool, _abil: &'static str) -> f64 {
        amount
    }
    /// The frames of an action of its whose ActInfo has Frames::CharLive:
    /// a gcsim frames function that reads the character's state when the
    /// frames are asked for.
    fn frames_live(_s: &Sim, _ci: usize, _id: u8, _arg: i16, _next: Action) -> i32 {
        unreachable!()
    }
    /// Its ReceiveHeal, when it refuses heals: whether a heal by `caller`
    /// heals it (and pays its Bond of Life).
    fn receives_heal(_s: &Sim, _ci: usize, _caller: usize) -> bool {
        true
    }
    /// Its ReceiveHeal, when it changes what a heal does: Some(the HP it
    /// received) when it handled the heal of `amount` by `caller` itself
    /// (Clorinde's Night Watch turns it into Bond of Life), None for the
    /// template's.
    fn receive_heal(_s: &mut Sim, _ci: usize, _caller: usize, _amount: f64) -> Option<f64> {
        None
    }
}

/// gcsim's template NextQueueItemIsValid: swords and polearms charge
/// only after a normal attack.
pub fn template_invalid(s: &Sim, a: Action) -> Option<String> {
    let active = &s.chars[s.player.active as usize];
    if a == Action::Charge && matches!(active.class, WeaponClass::Sword | WeaponClass::Spear) && active.normal_counter == 0 {
        return Some("need to use attack right before charge".into());
    }
    None
}

/// A weapon's logic, on the character in slot `ci`.
pub trait WeaponImpl {
    const KEY: &'static str;
    type State;
    fn new_state() -> Self::State;
    /// gcsim's NewWeapon.
    fn init(s: &mut Sim, ci: usize);
    /// gcsim's Weapon.Init, after the character's Init.
    fn post_init(_s: &mut Sim, _ci: usize) {}
    fn task(_s: &mut Sim, _ci: usize, _id: u8, _arg: i32) {}
    fn mod_amount(_s: &Sim, _ci: usize, _on: usize, _m: &Mod, _out: &mut Stats) -> bool {
        false
    }
    fn attack_mod(_s: &Sim, _ci: usize, _on: usize, _m: &Mod, _ev: &mut AttackEvent) {}
    fn event(_s: &mut Sim, _ci: usize, _e: &Event) {}
}

/// An artifact set's logic, at index `si` of the sets of the character in
/// slot `ci`.
pub trait SetImpl {
    const KEY: &'static str;
    type State;
    fn new_state() -> Self::State;
    /// gcsim's NewSet, with the pieces worn.
    fn init(s: &mut Sim, ci: usize, si: usize, count: i32);
    /// gcsim's Set.Init, after the character's Init.
    fn post_init(_s: &mut Sim, _ci: usize, _si: usize) {}
    fn task(_s: &mut Sim, _ci: usize, _si: usize, _id: u8, _arg: i32) {}
    fn mod_amount(_s: &Sim, _ci: usize, _si: usize, _on: usize, _m: &Mod, _out: &mut Stats) -> bool {
        false
    }
    fn attack_mod(_s: &Sim, _ci: usize, _si: usize, _on: usize, _m: &Mod, _ev: &mut AttackEvent) {}
    fn react_bonus(_s: &Sim, _ci: usize, _si: usize, _on: usize, _m: &Mod, _info: &AttackInfo) -> f64 {
        0.0
    }
    fn event(_s: &mut Sim, _ci: usize, _si: usize, _e: &Event) {}
}

/// Access to one implementation's state inside the generated state enum.
pub trait StateOf<E> {
    fn of(e: &E) -> &Self;
    fn of_mut(e: &mut E) -> &mut Self;
}

/// Registers characters: `chars! { Hutao => hutao, ... }` where each
/// module has a `Char` type implementing [`Character`].
#[macro_export]
macro_rules! chars {
    ($($kind:ident => $m:ident),* $(,)?) => {
        $(pub mod $m;)*

        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum CharKind { $($kind),* }

        pub enum CharState { $($kind(<$m::Char as $crate::content::Character>::State)),* }

        $(impl $crate::content::StateOf<CharState> for <$m::Char as $crate::content::Character>::State {
            #[inline]
            fn of(e: &CharState) -> &Self {
                #[allow(irrefutable_let_patterns)]
                if let CharState::$kind(x) = e { x } else { unreachable!() }
            }
            #[inline]
            fn of_mut(e: &mut CharState) -> &mut Self {
                #[allow(irrefutable_let_patterns)]
                if let CharState::$kind(x) = e { x } else { unreachable!() }
            }
        })*

        impl CharKind {
            pub fn from_name(name: &str) -> Option<CharKind> {
                use $crate::content::Character;
                $(if name == $m::Char::KEY { return Some(CharKind::$kind); })*
                None
            }
            pub fn max_cons(self) -> i32 {
                use $crate::content::Character;
                match self { $(CharKind::$kind => $m::Char::MAX_CONS),* }
            }
            pub fn hexerei(self) -> bool {
                use $crate::content::Character;
                match self { $(CharKind::$kind => $m::Char::HEXEREI),* }
            }
            pub fn moonsign(self) -> i32 {
                use $crate::content::Character;
                match self { $(CharKind::$kind => $m::Char::MOONSIGN),* }
            }
            pub fn new_state(self, cs: &$crate::sim::CharSetup) -> CharState {
                use $crate::content::Character;
                match self { $(CharKind::$kind => CharState::$kind($m::Char::new_state(cs))),* }
            }
        }

        /// Dispatch to the character's implementation.
        pub mod dispatch {
            use super::*;
            use $crate::content::{Character, Event};
            use $crate::gcsl::Num;
            use $crate::sim::*;
            use $crate::types::*;

            #[inline]
            pub fn init(k: CharKind, s: &mut Sim, ci: usize) { match k { $(CharKind::$kind => $m::Char::init(s, ci)),* } }
            pub fn unsupported(k: CharKind, a: Action, p: &Params) -> Option<String> { match k { $(CharKind::$kind => $m::Char::unsupported(a, p)),* } }
            #[inline]
            pub fn attack(k: CharKind, s: &mut Sim, ci: usize, p: &Params) -> ActInfo { match k { $(CharKind::$kind => $m::Char::attack(s, ci, p)),* } }
            #[inline]
            pub fn charge(k: CharKind, s: &mut Sim, ci: usize, p: &Params) -> ActInfo { match k { $(CharKind::$kind => $m::Char::charge(s, ci, p)),* } }
            #[inline]
            pub fn skill(k: CharKind, s: &mut Sim, ci: usize, p: &Params) -> ActInfo { match k { $(CharKind::$kind => $m::Char::skill(s, ci, p)),* } }
            #[inline]
            pub fn burst(k: CharKind, s: &mut Sim, ci: usize, p: &Params) -> ActInfo { match k { $(CharKind::$kind => $m::Char::burst(s, ci, p)),* } }
            #[inline]
            pub fn high_plunge(k: CharKind, s: &mut Sim, ci: usize, p: &Params) -> ActInfo { match k { $(CharKind::$kind => $m::Char::high_plunge(s, ci, p)),* } }
            #[inline]
            pub fn low_plunge(k: CharKind, s: &mut Sim, ci: usize, p: &Params) -> ActInfo { match k { $(CharKind::$kind => $m::Char::low_plunge(s, ci, p)),* } }
            #[inline]
            pub fn own_hit(k: CharKind, s: &Sim, ci: usize, ev: &mut AttackEvent) { match k { $(CharKind::$kind => $m::Char::own_hit(s, ci, ev)),* } }
            pub fn invalid(k: CharKind, s: &Sim, ci: usize, a: Action, p: &Params) -> Option<String> { match k { $(CharKind::$kind => $m::Char::invalid(s, ci, a, p)),* } }
            pub fn react_bonus(k: CharKind, s: &Sim, ci: usize, on: usize, m: &Mod, info: &AttackInfo) -> f64 { match k { $(CharKind::$kind => $m::Char::react_bonus(s, ci, on, m, info)),* } }
            #[inline]
            pub fn aim(k: CharKind, s: &mut Sim, ci: usize, p: &Params) -> ActInfo { match k { $(CharKind::$kind => $m::Char::aim(s, ci, p)),* } }
            #[inline]
            pub fn action_ready(k: CharKind, s: &Sim, ci: usize, a: Action, p: &Params) -> Option<Option<Failure>> { match k { $(CharKind::$kind => $m::Char::action_ready(s, ci, a, p)),* } }
            #[inline]
            pub fn task(k: CharKind, s: &mut Sim, ci: usize, id: u8, arg: i32) { match k { $(CharKind::$kind => $m::Char::task(s, ci, id, arg)),* } }
            #[inline]
            pub fn attack_cb(k: CharKind, s: &mut Sim, ci: usize, cbs: u16, ev: &AttackEvent) { match k { $(CharKind::$kind => $m::Char::attack_cb(s, ci, cbs, ev)),* } }
            #[inline]
            pub fn snapshot(k: CharKind, s: &Sim, ci: usize, info: &mut AttackInfo) { match k { $(CharKind::$kind => $m::Char::snapshot(s, ci, info)),* } }
            #[inline]
            pub fn mod_amount(k: CharKind, s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool { match k { $(CharKind::$kind => $m::Char::mod_amount(s, ci, on, m, out)),* } }
            #[inline]
            pub fn attack_mod(k: CharKind, s: &Sim, ci: usize, on: usize, m: &Mod, ev: &mut AttackEvent) { match k { $(CharKind::$kind => $m::Char::attack_mod(s, ci, on, m, ev)),* } }
            #[inline]
            pub fn event(k: CharKind, s: &mut Sim, ci: usize, e: &Event) { match k { $(CharKind::$kind => $m::Char::event(s, ci, e)),* } }
            #[inline]
            pub fn frames(k: CharKind, id: u8, arg: i16, next: Action) -> i32 { match k { $(CharKind::$kind => $m::Char::frames(id, arg, next)),* } }
            #[inline]
            pub fn on_removed(k: CharKind, s: &mut Sim, ci: usize, id: u8, next: AnimState) { match k { $(CharKind::$kind => $m::Char::on_removed(s, ci, id, next)),* } }
            #[inline]
            pub fn charge_stam(k: CharKind, s: &Sim, ci: usize) -> Option<f64> { match k { $(CharKind::$kind => $m::Char::charge_stam(s, ci)),* } }
            pub fn condition(k: CharKind, s: &Sim, ci: usize, f: &[String]) -> Option<Result<Num, String>> { match k { $(CharKind::$kind => $m::Char::condition(s, ci, f)),* } }
            pub fn construct_expired(k: CharKind, s: &mut Sim, ci: usize, key: ModKey) { match k { $(CharKind::$kind => $m::Char::construct_expired(s, ci, key)),* } }
            pub fn gadget_tick(k: CharKind, s: &mut Sim, ci: usize, id: u8, timer: i32) { match k { $(CharKind::$kind => $m::Char::gadget_tick(s, ci, id, timer)),* } }
            pub fn gadget_hit(k: CharKind, s: &mut Sim, ci: usize, id: u8, ev: &AttackEvent, lands: Option<bool>) { match k { $(CharKind::$kind => $m::Char::gadget_hit(s, ci, id, ev, lands)),* } }
            pub fn shield_damaged(k: CharKind, s: &mut Sim, ci: usize, key: ModKey, dmg: f64, broken: bool) { match k { $(CharKind::$kind => $m::Char::shield_damaged(s, ci, key, dmg, broken)),* } }
            pub fn dash_info(k: CharKind, s: &mut Sim, ci: usize, info: ActInfo) -> ActInfo { match k { $(CharKind::$kind => $m::Char::dash_info(s, ci, info)),* } }
            pub fn shield_expired(k: CharKind, s: &mut Sim, ci: usize, key: ModKey) { match k { $(CharKind::$kind => $m::Char::shield_expired(s, ci, key)),* } }
            pub fn shield_bonus(k: CharKind, s: &Sim, ci: usize, key: ModKey) -> (f64, bool) { match k { $(CharKind::$kind => $m::Char::shield_bonus(s, ci, key)),* } }
            #[inline]
            pub fn action_stam(k: CharKind, s: &Sim, ci: usize, a: Action) -> Option<f64> { match k { $(CharKind::$kind => $m::Char::action_stam(s, ci, a)),* } }
            pub fn start_delay(k: CharKind, s: &Sim, ci: usize, key: usize) -> Option<i32> { match k { $(CharKind::$kind => $m::Char::start_delay(s, ci, key)),* } }
            pub fn own_dash(k: CharKind, s: &mut Sim, ci: usize, p: &Params) -> Option<ActInfo> { match k { $(CharKind::$kind => $m::Char::own_dash(s, ci, p)),* } }
            pub fn own_jump(k: CharKind, s: &mut Sim, ci: usize, p: &Params) -> Option<ActInfo> { match k { $(CharKind::$kind => $m::Char::own_jump(s, ci, p)),* } }
            pub fn template_action(k: CharKind, s: &mut Sim, ci: usize, a: Action) { match k { $(CharKind::$kind => $m::Char::template_action(s, ci, a)),* } }
            pub fn pre_drain(k: CharKind, s: &mut Sim, ci: usize, on: usize, amount: f64, external: bool, abil: &'static str) -> f64 { match k { $(CharKind::$kind => $m::Char::pre_drain(s, ci, on, amount, external, abil)),* } }
            pub fn frames_live(k: CharKind, s: &Sim, ci: usize, id: u8, arg: i16, next: Action) -> i32 { match k { $(CharKind::$kind => $m::Char::frames_live(s, ci, id, arg, next)),* } }
            pub fn receives_heal(k: CharKind, s: &Sim, ci: usize, caller: usize) -> bool { match k { $(CharKind::$kind => $m::Char::receives_heal(s, ci, caller)),* } }
            pub fn receive_heal(k: CharKind, s: &mut Sim, ci: usize, caller: usize, amount: f64) -> Option<f64> { match k { $(CharKind::$kind => $m::Char::receive_heal(s, ci, caller, amount)),* } }
        }
    };
}

/// Registers weapons: `weapons! { Homa => homa, ... }` where each module
/// has a `Weapon` type implementing [`WeaponImpl`].
#[macro_export]
macro_rules! weapons {
    ($($kind:ident => $m:ident),* $(,)?) => {
        $(pub mod $m;)*

        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum WeaponKind { $($kind),* }

        pub enum WeaponState { $($kind(<$m::Weapon as $crate::content::WeaponImpl>::State)),* }

        $(impl $crate::content::StateOf<WeaponState> for <$m::Weapon as $crate::content::WeaponImpl>::State {
            #[inline]
            fn of(e: &WeaponState) -> &Self {
                #[allow(irrefutable_let_patterns)]
                if let WeaponState::$kind(x) = e { x } else { unreachable!() }
            }
            #[inline]
            fn of_mut(e: &mut WeaponState) -> &mut Self {
                #[allow(irrefutable_let_patterns)]
                if let WeaponState::$kind(x) = e { x } else { unreachable!() }
            }
        })*

        impl WeaponKind {
            pub fn from_name(name: &str) -> Option<WeaponKind> {
                use $crate::content::WeaponImpl;
                $(if name == $m::Weapon::KEY { return Some(WeaponKind::$kind); })*
                None
            }
            pub fn new_state(self) -> WeaponState {
                use $crate::content::WeaponImpl;
                match self { $(WeaponKind::$kind => WeaponState::$kind($m::Weapon::new_state())),* }
            }
        }

        pub mod dispatch {
            use super::*;
            use $crate::content::{Event, WeaponImpl};
            use $crate::sim::*;
            use $crate::types::*;

            pub fn init(k: WeaponKind, s: &mut Sim, ci: usize) { match k { $(WeaponKind::$kind => $m::Weapon::init(s, ci)),* } }
            pub fn post_init(k: WeaponKind, s: &mut Sim, ci: usize) { match k { $(WeaponKind::$kind => $m::Weapon::post_init(s, ci)),* } }
            #[inline]
            pub fn task(k: WeaponKind, s: &mut Sim, ci: usize, id: u8, arg: i32) { match k { $(WeaponKind::$kind => $m::Weapon::task(s, ci, id, arg)),* } }
            #[inline]
            pub fn mod_amount(k: WeaponKind, s: &Sim, ci: usize, on: usize, m: &Mod, out: &mut Stats) -> bool { match k { $(WeaponKind::$kind => $m::Weapon::mod_amount(s, ci, on, m, out)),* } }
            #[inline]
            pub fn attack_mod(k: WeaponKind, s: &Sim, ci: usize, on: usize, m: &Mod, ev: &mut AttackEvent) { match k { $(WeaponKind::$kind => $m::Weapon::attack_mod(s, ci, on, m, ev)),* } }
            #[inline]
            pub fn event(k: WeaponKind, s: &mut Sim, ci: usize, e: &Event) { match k { $(WeaponKind::$kind => $m::Weapon::event(s, ci, e)),* } }
        }
    };
}

/// Registers artifact sets: `sets! { Shimenawa => shimenawa, ... }` where
/// each module has a `Set` type implementing [`SetImpl`].
#[macro_export]
macro_rules! sets {
    ($($kind:ident => $m:ident),* $(,)?) => {
        $(pub mod $m;)*

        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum SetKind { $($kind),* }

        pub enum SetState { $($kind(<$m::Set as $crate::content::SetImpl>::State)),* }

        $(impl $crate::content::StateOf<SetState> for <$m::Set as $crate::content::SetImpl>::State {
            #[inline]
            fn of(e: &SetState) -> &Self {
                #[allow(irrefutable_let_patterns)]
                if let SetState::$kind(x) = e { x } else { unreachable!() }
            }
            #[inline]
            fn of_mut(e: &mut SetState) -> &mut Self {
                #[allow(irrefutable_let_patterns)]
                if let SetState::$kind(x) = e { x } else { unreachable!() }
            }
        })*

        impl SetKind {
            pub fn from_name(name: &str) -> Option<SetKind> {
                use $crate::content::SetImpl;
                $(if name == $m::Set::KEY { return Some(SetKind::$kind); })*
                None
            }
            pub fn new_state(self) -> SetState {
                use $crate::content::SetImpl;
                match self { $(SetKind::$kind => SetState::$kind($m::Set::new_state())),* }
            }
        }

        pub mod dispatch {
            use super::*;
            use $crate::content::{Event, SetImpl};
            use $crate::sim::*;
            use $crate::types::*;

            pub fn init(k: SetKind, s: &mut Sim, ci: usize, si: usize, count: i32) { match k { $(SetKind::$kind => $m::Set::init(s, ci, si, count)),* } }
            pub fn post_init(k: SetKind, s: &mut Sim, ci: usize, si: usize) { match k { $(SetKind::$kind => $m::Set::post_init(s, ci, si)),* } }
            #[inline]
            pub fn task(k: SetKind, s: &mut Sim, ci: usize, si: usize, id: u8, arg: i32) { match k { $(SetKind::$kind => $m::Set::task(s, ci, si, id, arg)),* } }
            #[inline]
            pub fn mod_amount(k: SetKind, s: &Sim, ci: usize, si: usize, on: usize, m: &Mod, out: &mut Stats) -> bool { match k { $(SetKind::$kind => $m::Set::mod_amount(s, ci, si, on, m, out)),* } }
            #[inline]
            pub fn attack_mod(k: SetKind, s: &Sim, ci: usize, si: usize, on: usize, m: &Mod, ev: &mut AttackEvent) { match k { $(SetKind::$kind => $m::Set::attack_mod(s, ci, si, on, m, ev)),* } }
            pub fn react_bonus(k: SetKind, s: &Sim, ci: usize, si: usize, on: usize, m: &Mod, info: &AttackInfo) -> f64 { match k { $(SetKind::$kind => $m::Set::react_bonus(s, ci, si, on, m, info)),* } }
            #[inline]
            pub fn event(k: SetKind, s: &mut Sim, ci: usize, si: usize, e: &Event) { match k { $(SetKind::$kind => $m::Set::event(s, ci, si, e)),* } }
        }
    };
}
