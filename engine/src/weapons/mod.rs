//! The weapons the engine implements. Each module has a `Weapon` type
//! implementing `content::WeaponImpl`; a line here registers it.

crate::weapons! {
    StaffOfHoma => staffofhoma,
    SacrificialSword => sacrificialsword,
    AquaSimulacra => aquasimulacra,
    Deathmatch => deathmatch,
    TheAlleyFlash => thealleyflash,
    HarbingerOfDawn => harbingerofdawn,
    FavoniusWarbow => favoniuswarbow,
}
