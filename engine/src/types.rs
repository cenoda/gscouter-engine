//! Enums and tables shared by the whole engine. Their numbering follows
//! gcsim's, so values read off gcsim's source and data line up.

/// gcsim's `Normal N` ability names.
pub const NORMAL_NAMES: [&str; 8] = ["Normal 0", "Normal 1", "Normal 2", "Normal 3", "Normal 4", "Normal 5", "Normal 6", "Normal 7"];

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

/// gcsim's attributes.Element, in its order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Element {
    NoElement,
    Pyro,
    Hydro,
    Dendro,
    Electro,
    Cryo,
    Anemo,
    Geo,
    Physical,
    Frozen,
    Quicken,
}

impl Element {
    /// From gcsim's attributes.Element.
    pub fn from_gcsim(v: u8) -> Self {
        use Element::*;
        [NoElement, Pyro, Hydro, Dendro, Electro, Cryo, Anemo, Geo, Physical, Frozen, Quicken][v as usize]
    }

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Failure {
    SwapCd = 1,
    SkillCd,
    BurstCd,
    InsufficientEnergy,
    InsufficientStamina,
    DashCd = 7,
}
pub const N_FAILURES: usize = 8;
pub const FAILURE_NAMES: [&str; N_FAILURES] =
    ["no_failure", "swap_cd", "skill_cd", "burst_cd", "insufficient_energy", "insufficient_stamina", "character_deceased", "dash_cd"];

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

pub use crate::gcsimdata::icd::{self, IcdGroup, IcdTag};

impl IcdGroup {
    pub fn reset_timer(self) -> i32 {
        icd::GROUPS[self.0 as usize].0
    }
    pub fn ele_seq(self) -> &'static [f64] {
        icd::GROUPS[self.0 as usize].1
    }
    pub fn dmg_seq(self) -> &'static [f64] {
        icd::GROUPS[self.0 as usize].2
    }
}
