//! The weapons the engine implements. Each module has a `Weapon` type
//! implementing `content::WeaponImpl`; a line here registers it.

/// Shared by the Favonius weapons.
pub mod favonius;

crate::weapons! {
    StaffOfHoma => staffofhoma,
    SacrificialSword => sacrificialsword,
    AquaSimulacra => aquasimulacra,
    Deathmatch => deathmatch,
    TheAlleyFlash => thealleyflash,
    HarbingerOfDawn => harbingerofdawn,
    FavoniusWarbow => favoniuswarbow,
    FavoniusSword => favoniussword,
    FavoniusLance => favoniuslance,
    FavoniusGreatsword => favoniusgreatsword,
    FavoniusCodex => favoniuscodex,
    TheStringless => thestringless,
    BalladOfTheFjords => balladofthefjords,
}
