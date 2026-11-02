//! The characters the engine implements. Each module has a `Char` type
//! implementing `content::Character`; a line here registers it.

pub mod plunge;
pub mod watch;

crate::chars! {
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
    Cyno => cyno,
    Citlali => citlali,
    Mavuika => mavuika,
}

/// The characters whose modules have been checked against gcsim for a
/// player that set_player_pos moved away from the origin: every pattern,
/// area and point taken from the player's position or direction, and at
/// the time gcsim takes it. Moving the player with anyone else on the team
/// is an error that names them.
pub const MOVES_WITH_PLAYER: &[CharKind] = &[CharKind::Bennett, CharKind::Diluc, CharKind::Gaming, CharKind::Sucrose, CharKind::Xingqiu, CharKind::Yoimiya, CharKind::Sandrone];
