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
    WolfFang => wolffang,
    AlleyHunter => alleyhunter,
    SapwoodBlade => sapwoodblade,
    FesteringDesire => festeringdesire,
    Slingshot => slingshot,
    DragonsBane => dragonsbane,
    XiphosMoonlight => xiphosmoonlight,
    TheCatch => thecatch,
    Rust => rust,
    SerpentSpine => serpentspine,
    LionsRoar => lionsroar,
    TheViridescentHunt => theviridescenthunt,
    WanderingEvenstar => wanderingevenstar,
    TheWidsith => thewidsith,
    ThrillingTales => thrillingtales,
}
