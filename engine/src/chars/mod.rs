//! The characters the engine implements. Each module has a `Char` type
//! implementing `content::Character`; a line here registers it.

pub mod plunge;
pub mod traveler;
pub mod traveleranemo;
pub mod travelerdendro;
pub mod travelergeo;
pub mod traveler_common;
pub mod traveler_pyro;
pub mod traveler_cryo;
pub mod traveler_hydro;
pub mod traveler_electro;
pub mod watch;

crate::chars! {
    LuminePyro => luminepyro,
    AetherPyro => aetherpyro,
    LumineCryo => luminecryo,
    AetherCryo => aethercryo,
    LumineHydro => luminehydro,
    AetherHydro => aetherhydro,
    LumineElectro => lumineelectro,
    AetherElectro => aetherelectro,
    Hutao => hutao,
    Xingqiu => xingqiu,
    Zhongli => zhongli,
    Yelan => yelan,
    Bennett => bennett,
    Fischl => fischl,
    Furina => furina,
    Kazuha => kazuha,
    Raiden => raiden,
    Xiangling => xiangling,
    Yoimiya => yoimiya,
    Chevreuse => chevreuse,
    Rosaria => rosaria,
    Escoffier => escoffier,
    Diluc => diluc,
    Gaming => gaming,
    Candace => candace,
    Nahida => nahida,
    Chiori => chiori,
    Jean => jean,
    Sucrose => sucrose,
    Wriothesley => wriothesley,
    Yaemiko => yaemiko,
    Diona => diona,
    Klee => klee,
    Lynette => lynette,
    Shenhe => shenhe,
    Lisa => lisa,
    Kuki => kuki,
    Alhaitham => alhaitham,
    Baizhu => baizhu,
    Neuvillette => neuvillette,
    Kirara => kirara,
    Xilonen => xilonen,
    Xianyun => xianyun,
    Sandrone => sandrone,
    Thoma => thoma,
    Ineffa => ineffa,
    Ororon => ororon,
    Mona => mona,
    Emilie => emilie,
    Cyno => cyno,
    Citlali => citlali,
    Mavuika => mavuika,
    Arlecchino => arlecchino,
    Skirk => skirk,
    Faruzan => faruzan,
    Kokomi => kokomi,
    KujouSara => sara,
    Navia => navia,
    Clorinde => clorinde,
    Razor => razor,
    Dehya => dehya,
    Chasca => chasca,
    Ganyu => ganyu,
    Sethos => sethos,
    Yunjin => yunjin,
    Noelle => noelle,
    Ayaka => ayaka,
    Albedo => albedo,
    Layla => layla,
    Durin => durin,
    Kaeya => kaeya,
    Vesna => vesna,
    Nicole => nicole,
    Mika => mika,
    Gorou => gorou,
    Chongyun => chongyun,
    Beidou => beidou,
    Ayato => ayato,
    Lanyan => lanyan,
    Columbina => columbina,
    Charlotte => charlotte,
    Yaoyao => yaoyao,
    Xiao => xiao,
    Eula => eula,
    Freminet => freminet,
    Kinich => kinich,
    Lyney => lyney,
    Keqing => keqing,
    Mualani => mualani,
    Lauma => lauma,
    Nilou => nilou,
    Collei => collei,
    Tighnari => tighnari,
    Dori => dori,
    Varesa => varesa,
    Wanderer => wanderer,
    Ningguang => ningguang,
    Itto => itto,
    Venti => venti,
    Xinyan => xinyan,
    Aino => aino,
    Flins => flins,
    Lohen => lohen,
    Jahoda => jahoda,
    Tartaglia => tartaglia,
    Kaveh => kaveh,
    Prune => prune,
    YumemizukiMizuki => mizuki,
    Linnea => linnea,
    Dahlia => dahlia,
    Sigewinne => sigewinne,
    Vodyanitsa => vodyanitsa,
    Odette => odette,
    Qiqi => qiqi,
    Barbara => barbara,
    Amber => amber,
    Iansan => iansan,
    Ifa => ifa,
    Aloy => aloy,
    Alyosha => alyosha,
    Zibai => zibai,
    Illuga => illuga,
    Varka => varka,
    Heizou => heizou,
    Sayu => sayu,
    Yanfei => yanfei,
    Aethergeo => aethergeo,
    Luminegeo => luminegeo,
    Aetheranemo => aetheranemo,
    Lumineanemo => lumineanemo,
    Aetherdendro => aetherdendro,
    Luminedendro => luminedendro,
}

/// The characters whose modules have been checked against gcsim for a
/// player that set_player_pos moved away from the origin: every pattern,
/// area and point taken from the player's position or direction, and at
/// the time gcsim takes it. Moving the player with anyone else on the team
/// is an error that names them.
pub const MOVES_WITH_PLAYER: &[CharKind] = &[
    CharKind::Barbara,
    CharKind::Bennett, CharKind::Diluc, CharKind::Gaming, CharKind::Sucrose, CharKind::Xingqiu, CharKind::Yoimiya, CharKind::Sandrone, CharKind::Arlecchino, CharKind::Ayaka, CharKind::Ayato, CharKind::Baizhu, CharKind::Beidou, CharKind::Charlotte, CharKind::Chasca, CharKind::Chevreuse, CharKind::Chongyun, CharKind::Citlali, CharKind::Clorinde, CharKind::Columbina, CharKind::Durin, CharKind::Fischl, CharKind::Furina, CharKind::Hutao, CharKind::Ineffa, CharKind::Kazuha, CharKind::Kinich, CharKind::KujouSara, CharKind::Kuki, CharKind::Lanyan, CharKind::Layla, CharKind::Mavuika, CharKind::Nahida, CharKind::Navia, CharKind::Nilou, CharKind::Noelle, CharKind::Ororon, CharKind::Raiden, CharKind::Rosaria, CharKind::Thoma, CharKind::Xiangling, CharKind::Xianyun, CharKind::Yaemiko, CharKind::Yaoyao, CharKind::Yelan, CharKind::Yunjin, CharKind::LuminePyro, CharKind::AetherPyro, CharKind::Aetherdendro, CharKind::Luminedendro, CharKind::Aethergeo, CharKind::Luminegeo, CharKind::Aetheranemo, CharKind::Lumineanemo, CharKind::Aino, CharKind::Collei, CharKind::Zhongli,
];

impl CharKind {
    /// One of gcsim's Travelers, whose NewTraveler reads the story quest
    /// params (`traveler_common::PARAMS`).
    pub fn traveler(self) -> bool {
        matches!(self, CharKind::LuminePyro | CharKind::AetherPyro | CharKind::LumineCryo | CharKind::AetherCryo | CharKind::LumineHydro | CharKind::AetherHydro | CharKind::LumineElectro | CharKind::AetherElectro | CharKind::Aetherdendro | CharKind::Luminedendro | CharKind::Aethergeo | CharKind::Luminegeo | CharKind::Aetheranemo | CharKind::Lumineanemo)
    }

    /// The hydro Traveler, whose Sourcewater Droplets are gadgets.
    pub fn traveler_hydro(self) -> bool {
        matches!(self, CharKind::LumineHydro | CharKind::AetherHydro)
    }
}
