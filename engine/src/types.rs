//! Enums and tables shared by the whole engine. Their numbering follows
//! gcsim's, so values read off gcsim's source and data line up.

pub const N_STATS: usize = 26;
pub type Stats = [f64; N_STATS];

/// Stat indices, in gcsim's `attributes.Stat` order.
pub mod st {
    pub const NONE: usize = 0;
    pub const DEFP: usize = 1;
    pub const DEF: usize = 2;
    pub const HP: usize = 3;
    pub const HPP: usize = 4;
    pub const ATK: usize = 5;
    pub const ATKP: usize = 6;
    pub const ER: usize = 7;
    pub const EM: usize = 8;
    pub const CR: usize = 9;
    pub const CD: usize = 10;
    pub const HEAL: usize = 11;
    pub const PYROP: usize = 12;
    pub const HYDROP: usize = 13;
    pub const CRYOP: usize = 14;
    pub const ELECTROP: usize = 15;
    pub const ANEMOP: usize = 16;
    pub const GEOP: usize = 17;
    pub const DENDROP: usize = 18;
    pub const PHYP: usize = 19;
    pub const ATKSPD: usize = 20;
    pub const DMGP: usize = 21;
    pub const BASEHP: usize = 23;
    pub const BASEATK: usize = 24;
    pub const BASEDEF: usize = 25;

    pub fn by_name(s: &str) -> Option<usize> {
        Some(match s {
            "def%" => DEFP,
            "def" => DEF,
            "hp" => HP,
            "hp%" => HPP,
            "atk" => ATK,
            "atk%" => ATKP,
            "er" => ER,
            "em" => EM,
            "cr" => CR,
            "cd" => CD,
            "heal" => HEAL,
            "pyro%" => PYROP,
            "hydro%" => HYDROP,
            "cryo%" => CRYOP,
            "electro%" => ELECTROP,
            "anemo%" => ANEMOP,
            "geo%" => GEOP,
            "dendro%" => DENDROP,
            "phys%" => PHYP,
            "atkspd%" => ATKSPD,
            "dmg%" => DMGP,
            "base_hp" => BASEHP,
            "base_atk" => BASEATK,
            "base_def" => BASEDEF,
            _ => return None,
        })
    }
}

pub fn max_hp(s: &Stats) -> f64 {
    s[st::BASEHP] * (1.0 + s[st::HPP]) + s[st::HP]
}
pub fn total_atk(s: &Stats) -> f64 {
    s[st::BASEATK] * (1.0 + s[st::ATKP]) + s[st::ATK]
}
pub fn total_def(s: &Stats) -> f64 {
    s[st::BASEDEF] * (1.0 + s[st::DEFP]) + s[st::DEF]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Element {
    Electro,
    Pyro,
    Cryo,
    Hydro,
    Dendro,
    Quicken,
    Frozen,
    Anemo,
    Geo,
    NoElement,
    Physical,
}

impl Element {
    pub fn dmg_stat(self) -> usize {
        match self {
            Element::Pyro => st::PYROP,
            Element::Hydro => st::HYDROP,
            Element::Cryo => st::CRYOP,
            Element::Electro => st::ELECTROP,
            Element::Anemo => st::ANEMOP,
            Element::Geo => st::GEOP,
            Element::Dendro => st::DENDROP,
            Element::Physical => st::PHYP,
            _ => st::NONE,
        }
    }
}

/// gcsim's `action.Action`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Action {
    Invalid = 0,
    Skill = 1,
    Burst = 2,
    Attack = 3,
    Charge = 4,
    HighPlunge = 5,
    LowPlunge = 6,
    Aim = 7,
    Dash = 8,
    Jump = 9,
    Swap = 10,
    Walk = 11,
}
pub const N_ACTIONS: usize = 15;
pub type FrameTable = [i32; N_ACTIONS];

/// `frames.InitAbilSlice`
pub const fn abil_slice(anim: i32) -> FrameTable {
    [anim; N_ACTIONS]
}
/// `frames.InitNormalCancelSlice`
pub const fn normal_cancel_slice(hitmark: i32, anim: i32) -> FrameTable {
    let mut t = [anim; N_ACTIONS];
    t[Action::Aim as usize] = hitmark;
    t[Action::Skill as usize] = hitmark;
    t[Action::Burst as usize] = hitmark;
    t[Action::Dash as usize] = hitmark;
    t[Action::Jump as usize] = hitmark;
    t[Action::Swap as usize] = hitmark;
    t
}
pub const fn set(mut t: FrameTable, a: Action, v: i32) -> FrameTable {
    t[a as usize] = v;
    t
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnimState {
    Idle,
    Normal,
    Charge,
    Plunge,
    Skill,
    Burst,
    Aim,
    Dash,
    Jump,
    Walk,
    Swap,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttackTag {
    None,
    Normal,
    Extra,
    Plunge,
    ElementalArt,
    ElementalArtHold,
    ElementalBurst,
    Weapon,
}

/// gcsim's ICD tags that the engine's characters use.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IcdTag {
    None,
    NormalAttack,
    ExtraAttack,
    ElementalArt,
    ElementalArtHold,
    ElementalBurst,
    YelanBreakthrough,
    YelanBurst,
    ElementalBurstHydro,
}

/// gcsim's ICD groups that the engine's characters use, with their
/// tables from `attacks/icd_groups.dm.go` (Yelan's burst group has the
/// default group's sequences and a 2 s timer).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IcdGroup {
    Default,
    PoleExtraAttack,
    XingqiuBurst,
    YelanBurst,
}

impl IcdGroup {
    pub fn reset_timer(self) -> i32 {
        match self {
            IcdGroup::Default => 150,
            IcdGroup::PoleExtraAttack => 30,
            IcdGroup::XingqiuBurst => 60,
            IcdGroup::YelanBurst => 120,
        }
    }
    pub fn ele_seq(self) -> &'static [f64] {
        match self {
            IcdGroup::Default => &DEFAULT_ELE,
            IcdGroup::PoleExtraAttack => &POLE_ELE,
            IcdGroup::XingqiuBurst => &XQ_BURST_ELE,
            IcdGroup::YelanBurst => &DEFAULT_ELE,
        }
    }
    pub fn dmg_seq(self) -> &'static [f64] {
        match self {
            IcdGroup::Default => &DEFAULT_DMG,
            IcdGroup::PoleExtraAttack => &POLE_DMG,
            IcdGroup::XingqiuBurst => &XQ_BURST_DMG,
            IcdGroup::YelanBurst => &DEFAULT_DMG,
        }
    }
}

static DEFAULT_ELE: [f64; 24] = [
    1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0,
    0.0, 0.0, 1.0, 0.0, 0.0,
];
static DEFAULT_DMG: [f64; 15] = [1.0; 15];
static POLE_ELE: [f64; 24] = {
    let mut a = [0.0; 24];
    a[0] = 1.0;
    a
};
static POLE_DMG: [f64; 15] = {
    let mut a = [0.0; 15];
    a[0] = 1.0;
    a
};
// Unused: Xingqiu's swords are in the default group.
static XQ_BURST_ELE: [f64; 1] = [1.0];
static XQ_BURST_DMG: [f64; 1] = [1.0];
