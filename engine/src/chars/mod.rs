//! The characters the engine implements. Each module has a `Char` type
//! implementing `content::Character`; a line here registers it.

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
}
