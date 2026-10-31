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
}
pub const N_EVENTS: usize = 15;

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
    /// gained and the overheal.
    Heal { caller: usize, target: Option<usize>, on: usize, amount: f64, overheal: f64 },
    /// OnPlayerHPDrain: the character drained, the HP it lost, and whether
    /// it came from outside the party (gcsim's DrainInfo.External).
    Drain { on: usize, amount: f64, external: bool },
    /// OnParticleReceived, after the party took a particle.
    ParticleReceived,
    /// OnEnergyBurst: a character's burst drained amount of its energy.
    EnergyBurst { on: usize, amount: f64 },
    /// OnEnemyHit, before attack mods: a subscriber adds flat damage to
    /// the hit through `Sim::hit_flat`.
    EnemyHit(&'a AttackEvent),
    /// OnPlayerHit: the active character was hit.
    PlayerHit { on: usize },
    /// OnShielded: a shield was added (or replaced one of its type).
    Shielded(&'a crate::sim::Shield),
    /// OnShieldBreak: a shield broke or expired.
    ShieldBreak(&'a crate::sim::Shield),
    /// OnConstructSpawned.
    ConstructSpawned,
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
            Event::ParticleReceived => EventKind::ParticleReceived,
            Event::EnergyBurst { .. } => EventKind::EnergyBurst,
            Event::EnemyHit(_) => EventKind::EnemyHit,
        }
    }
}

/// A character's logic. `ci` is its slot in the team.
pub trait Character {
    /// gcsim's key name.
    const KEY: &'static str;
    /// The highest constellation it implements; a higher one is an error.
    const MAX_CONS: i32 = 6;
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
    /// A shield of its took damage (gcsim's OnDamage of its own shield
    /// type), and whether that broke it.
    fn shield_damaged(_s: &mut Sim, _ci: usize, _key: ModKey, _broken: bool) {}
    /// A shield of its expired (gcsim's OnExpire).
    fn shield_expired(_s: &mut Sim, _ci: usize, _key: ModKey) {}
    /// A shield bonus mod it owns: the bonus, and whether gcsim's
    /// ShieldBonus then drops the mod (its `done`).
    fn shield_bonus(_s: &Sim, _ci: usize, _key: ModKey) -> (f64, bool) {
        (0.0, false)
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
            pub fn shield_damaged(k: CharKind, s: &mut Sim, ci: usize, key: ModKey, broken: bool) { match k { $(CharKind::$kind => $m::Char::shield_damaged(s, ci, key, broken)),* } }
            pub fn shield_expired(k: CharKind, s: &mut Sim, ci: usize, key: ModKey) { match k { $(CharKind::$kind => $m::Char::shield_expired(s, ci, key)),* } }
            pub fn shield_bonus(k: CharKind, s: &Sim, ci: usize, key: ModKey) -> (f64, bool) { match k { $(CharKind::$kind => $m::Char::shield_bonus(s, ci, key)),* } }
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
