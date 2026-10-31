//! The fight: gcsim's frame loop, task queues, actions, hitlag, mods,
//! elemental gauges and damage, kept in the order gcsim runs them.

pub use crate::chars::{CharKind, CharState};
pub use crate::content::{Event, EventKind, Owner, StateOf, N_EVENTS};
use crate::gcsl::Num;
use crate::queue::Queue;
pub use crate::sets::{SetKind, SetState};
pub use crate::weapons::{WeaponKind, WeaponState};
use crate::rng::Rng;
use crate::types::*;

// ---------------------------------------------------------------- mods

/// A mod's key: gcsim's key name, which .char.mods.<key> reads, and its
/// hash, which lookups compare.
#[derive(Clone, Copy, Debug)]
pub struct ModKey {
    pub hash: u32,
    pub name: &'static str,
}

impl ModKey {
    pub const fn new(name: &'static str) -> ModKey {
        // FNV-1a
        let b = name.as_bytes();
        let mut h: u32 = 0x811c9dc5;
        let mut i = 0;
        while i < b.len() {
            h ^= b[i] as u32;
            h = h.wrapping_mul(0x01000193);
            i += 1;
        }
        ModKey { hash: h, name }
    }
}

impl ModKey {
    /// A key whose name is built at run time, such as gcsim's
    /// "xiphos-<character>". Each name is interned once for the whole
    /// process, so fights may make them.
    pub fn named(name: String) -> ModKey {
        use std::collections::HashMap;
        use std::sync::{Mutex, OnceLock};
        static NAMES: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
        let mut names = NAMES.get_or_init(Default::default).lock().unwrap();
        let s: &'static str = match names.get(&name) {
            Some(s) => s,
            None => {
                let s: &'static str = Box::leak(name.clone().into_boxed_str());
                names.insert(name, s);
                s
            }
        };
        ModKey::new(s)
    }
}

impl PartialEq for ModKey {
    #[inline]
    fn eq(&self, o: &ModKey) -> bool {
        self.hash == o.hash
    }
}
impl Eq for ModKey {}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModKind {
    Stat,
    Attack,
    Status,
    Resist,
    /// gcsim's DefMod on the enemy: its amount in `value`.
    Def,
    /// gcsim's HealBonusMod: incoming healing bonus, its amount in
    /// st::HEAL.
    HealBonus,
    /// gcsim's ReactBonusMod: its owner's react_bonus.
    ReactBonus,
}

/// What a stat or attack mod adds: a fixed amount of one stat, or what
/// its owner computes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Amount {
    Fixed,
    Owner(Owner),
}

#[derive(Clone, Copy, Debug)]
pub struct Mod {
    pub key: ModKey,
    pub kind: ModKind,
    pub amount: Amount,
    /// The stat it affects (gcsim's AffectedStat; NONE for any), which is
    /// also the stat a fixed amount adds to.
    pub affected: usize,
    pub extra: bool,
    pub hitlag: bool,
    pub expiry_base: i32,
    pub extension: i32,
    /// Resist mods: the element. Fixed and resist mods: the amount.
    pub ele: Element,
    pub value: f64,
    /// The frame it was added.
    pub started: i32,
}

impl Mod {
    pub fn new(key: ModKey, kind: ModKind, affected: usize, extra: bool, hitlag: bool, expiry: i32) -> Mod {
        Mod { key, kind, amount: Amount::Fixed, affected, extra, hitlag, expiry_base: expiry, extension: 0, ele: Element::NoElement, value: 0.0, started: 0 }
    }
    /// A stat mod adding value to stat.
    pub fn stat(key: ModKey, stat: usize, value: f64, extra: bool, hitlag: bool, expiry: i32) -> Mod {
        let mut m = Mod::new(key, ModKind::Stat, stat, extra, hitlag, expiry);
        m.value = value;
        m
    }
    /// A mod whose amount its owner computes.
    pub fn owned(key: ModKey, kind: ModKind, owner: Owner, affected: usize, extra: bool, hitlag: bool, expiry: i32) -> Mod {
        let mut m = Mod::new(key, kind, affected, extra, hitlag, expiry);
        m.amount = Amount::Owner(owner);
        m
    }
    #[inline]
    pub fn expiry(&self) -> i32 {
        self.expiry_base + self.extension
    }
    /// Counts in a stat sum (gcsim's Stats, Stat, SelectStat).
    #[inline]
    pub fn applies(&self, f: i32) -> bool {
        let e = self.expiry();
        e > f || e == -1
    }
    /// Counts as active (gcsim's FindCheckExpiry): active on its last frame.
    #[inline]
    pub fn active(&self, f: i32) -> bool {
        let e = self.expiry();
        !(e < f && e > -1)
    }
    pub fn extend(&mut self, amt: i32) {
        self.extension += amt;
        if self.extension < 0 {
            self.extension = 0;
        }
    }
}

fn mod_find(mods: &[Mod], key: ModKey) -> Option<usize> {
    mods.iter().position(|m| m.key == key)
}

pub fn mod_add(mods: &mut Vec<Mod>, m: Mod) {
    match mod_find(mods, m.key) {
        Some(i) => mods[i] = m,
        None => mods.push(m),
    }
}

pub const PYRO_RESONANCE: ModKey = ModKey::new("pyro-res");
pub const HYDRO_RESONANCE: ModKey = ModKey::new("hydro-res-hpp");
pub const CRYO_RESONANCE: ModKey = ModKey::new("cryo-res");
const LUNAR_BLOOM: ModKey = ModKey::new("lunarbloom-enabled");
pub const GEO_RESONANCE: ModKey = ModKey::new("geo-res");
pub const DENDRO_RESONANCE: ModKey = ModKey::new("dendro-res-50");
const DENDRO_RESONANCE_30: ModKey = ModKey::new("dendro-res-30");
const DENDRO_RESONANCE_20: ModKey = ModKey::new("dendro-res-20");
const SUPERCONDUCT_SHRED: ModKey = ModKey::new("superconduct-phys-shred");

// ---------------------------------------------------------------- attacks

/// Where an attack hits (gcsim's AttackPattern), for the gadgets it may
/// hit besides the enemy, in the player's frame: the player at the origin,
/// facing the enemy along +y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pattern {
    /// Not given. The enemy is taken to be hit; a dendro core that could
    /// react to it is an error.
    Unknown,
    /// One target, the enemy: never a gadget.
    Single,
    /// A full circle.
    Circle { x: f64, y: f64, r: f64 },
    /// A circle cut to a fan of `fan` degrees facing the enemy.
    Fan { x: f64, y: f64, r: f64, fan: f64 },
    /// gcsim's rectangle facing the enemy: w wide, from its spawn (x, y)
    /// to h ahead.
    Box { x: f64, y: f64, w: f64, h: f64 },
}

/// Pattern's shapes, for the tests.
#[derive(Clone, Copy)]
enum Wide {
    Unknown,
    Single,
    Circle { x: f64, y: f64, r: f64 },
    Fan { x: f64, y: f64, r: f64, fan: f64 },
    Box { x: f64, y: f64, w: f64, h: f64 },
}

impl Pattern {
    fn wide(self) -> Wide {
        match self {
            Pattern::Unknown => Wide::Unknown,
            Pattern::Single => Wide::Single,
            Pattern::Circle { x, y, r } => Wide::Circle { x, y, r },
            Pattern::Fan { x, y, r, fan } => Wide::Fan { x, y, r, fan },
            Pattern::Box { x, y, w, h } => Wide::Box { x, y, w, h },
        }
    }
    /// Whether it reaches a circle at (cx, cy) of radius r (gcsim's
    /// IntersectCircle and IntersectRectangle); None when unknown.
    pub fn hits_circle(self, cx: f64, cy: f64, r: f64) -> Option<bool> {
        match self.wide() {
            Wide::Unknown => None,
            Wide::Single => Some(false),
            Wide::Circle { x, y, r: pr } => Some((cx - x) * (cx - x) + (cy - y) * (cy - y) <= (pr + r) * (pr + r)),
            Wide::Fan { x, y, r: pr, fan } if fan <= 0.0 || fan >= 360.0 => Some((cx - x) * (cx - x) + (cy - y) * (cy - y) <= (pr + r) * (pr + r)),
            Wide::Fan { x, y, r: pr, fan } => {
                if (cx - x) * (cx - x) + (cy - y) * (cy - y) > (pr + r) * (pr + r) {
                    return Some(false);
                }
                // the target's circle against the fan's two edges
                let (opx, opy) = (x - cx, y - cy);
                let op_dist = opx.hypot(opy);
                for a in [-fan / 2.0, fan / 2.0] {
                    let a = a.to_radians();
                    let (qx, qy) = (x + pr * a.sin(), y + pr * a.cos());
                    let (qpx, qpy) = (x - qx, y - qy);
                    let (oqx, oqy) = (qx - cx, qy - cy);
                    let mut min_dist = op_dist.min(oqx.hypot(oqy));
                    if opx * qpx + opy * qpy > 0.0 && oqx * -qpx + oqy * -qpy > 0.0 {
                        min_dist = (opx * oqy - opy * oqx).abs() / pr;
                    }
                    if min_dist <= r {
                        return Some(true);
                    }
                }
                // or its center within the fan's angle
                let dy = if cx == x && cy == y { 1.0 } else { (cy - y) / (cx - x).hypot(cy - y) };
                Some(dy.clamp(-1.0, 1.0).acos().to_degrees() <= fan / 2.0)
            }
            Wide::Box { x, y, w, h } => {
                let (hw, hh) = (w / 2.0, h / 2.0);
                let (lx, ly) = ((cx - x).abs(), (cy - (y + hh)).abs());
                if lx > r + hw || ly > r + hh {
                    return Some(false);
                }
                if lx <= hw || ly <= hh {
                    return Some(true);
                }
                Some((lx - hw) * (lx - hw) + (ly - hh) * (ly - hh) <= r * r)
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AttackInfo {
    pub actor: u8,
    pub abil: &'static str,
    pub tag: AttackTag,
    pub icd_tag: IcdTag,
    pub icd_group: IcdGroup,
    pub element: Element,
    pub durability: f64,
    pub mult: f64,
    pub flat_dmg: f64,
    pub hitlag_factor: f64,
    pub hitlag_halt: f64,
    pub def_halt: bool,
    pub deployable: bool,
    pub amped: bool,
    pub amp_mult: f64,
    pub ignore_def: f64,
    pub ignore_infusion: bool,
    /// gcsim's StrikeTypeBlunt, and the attack's PoiseDMG: they break
    /// frozen.
    pub blunt: bool,
    pub poise: f64,
    /// gcsim's SourceIsSim: no attack mods and no ICD, and not recorded.
    pub sim: bool,
    /// gcsim's Catalyzed and CatalyzedType: aggravate or spread added
    /// flat damage to it.
    pub catalyzed: Option<Reaction>,
    pub pattern: Pattern,
}

impl AttackInfo {
    pub fn new(actor: usize, tag: AttackTag, icd_tag: IcdTag, icd_group: IcdGroup, element: Element, durability: f64, mult: f64) -> Self {
        AttackInfo {
            actor: actor as u8,
            abil: "",
            tag,
            icd_tag,
            icd_group,
            element,
            durability,
            mult,
            flat_dmg: 0.0,
            hitlag_factor: 0.0,
            hitlag_halt: 0.0,
            def_halt: false,
            deployable: false,
            amped: false,
            amp_mult: 1.0,
            ignore_def: 0.0,
            ignore_infusion: false,
            blunt: false,
            poise: 0.0,
            sim: false,
            catalyzed: None,
            pattern: Pattern::Unknown,
        }
    }
}

#[derive(Clone, Copy)]
pub struct AttackEvent {
    pub info: AttackInfo,
    pub stats: Stats,
    pub char_lvl: i32,
    /// The frame its snapshot was taken (gcsim's Snapshot.SourceFrame).
    pub snap_f: i32,
    /// The attacker's callbacks for when it hits: bits its character's
    /// module defines.
    pub callbacks: u16,
    pub reacted: bool,
    pub damage: f64,
    pub crit: bool,
    /// The reaction it causes is on the player (ReactWithSelf), not the
    /// enemy: subscribers that check for an enemy target skip it.
    pub on_player: bool,
}

impl AttackEvent {
    pub fn new(info: AttackInfo, callbacks: u16) -> Self {
        AttackEvent { info, stats: [0.0; N_STATS], char_lvl: 0, snap_f: 0, callbacks, reacted: false, damage: 0.0, crit: false, on_player: false }
    }
}

// ---------------------------------------------------------------- tasks

#[derive(Clone, Copy, Debug)]
pub enum Task {
    /// QueueAttack(slot) with these delays, run later (from a char queue).
    QueueAttack { slot: u16, snap: i32, dmg: i32 },
    Snapshot { slot: u16, dmg: i32 },
    ApplyAttack { slot: u16 },
    DamageApplied { slot: u16 },
    SetCd { c: u8, a: Action, dur: i32 },
    CdWorker { c: u8, a: Action, token: u32 },
    ConsumeEnergy { c: u8, delay_amount: bool },
    AddEnergy { c: u8, amt: f64 },
    Particle { num: f64, ele: Element },
    DashStam { c: u8 },
    IcdResetDmg { i: u16 },
    IcdResetTag { i: u16 },
    Swap { to: u8 },
    /// A task of the character in slot c, of its weapon, or of its set at
    /// index si: the module reads id and arg.
    Char { c: u8, id: u8, arg: i32 },
    Weapon { c: u8, id: u8, arg: i32 },
    Set { c: u8, si: u8, id: u8, arg: i32 },
    /// Electro-charged's ticks, of the electro-charged started at src.
    EcTick { src: i32 },
    EcWane,
    /// Burning's ticks, of the burning started at src, the counter-th.
    BurningTick { src: i32, counter: i32 },
    /// Quicken's bloom with hydro left, at the end of the frame (the
    /// attack in the slot).
    QuickenBloom { slot: u16 },
    /// A bloom's dendro core appears, from the attack in the slot.
    SpawnCore { slot: u16 },
    /// A dendro core's attack: bloom, burgeon or hyperbloom, by actor, and
    /// whether its self damage reaches the player.
    CoreAttack { kind: AttackTag, actor: u8, on_player: bool },
    PlayerIcdResetDmg { i: u16 },
    /// An attack that reaches only gadgets (swirl's aoe).
    GadgetAttack { slot: u16 },
    /// A crystallize shard appears, of an element by a character.
    SpawnShard { actor: u8, ele: Element },
    /// The shield stats collector's ShieldBonus at the end of the frame
    /// a player is hit.
    ShieldTrack,
    /// Geo resonance's check of the active character's shield.
    GeoRes { src: i32 },
}

// ---------------------------------------------------------------- actions

#[derive(Clone, Copy, Debug)]
pub enum Frames {
    Table(&'static FrameTable),
    Attack { table: &'static FrameTable, atkspd: f64 },
    /// The character's own frames function, with an argument.
    Char { id: u8, arg: i16 },
    Dash { len: i32, jump: i32 },
    Const(i32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OnRemoved {
    None,
    /// The character's on_removed, with an id.
    Char(u8),
}

#[derive(Clone, Copy, Debug)]
pub struct ActInfo {
    pub frames: Frames,
    pub anim_len: i32,
    pub can_queue_after: i32,
    pub state: AnimState,
    pub char: u8,
    pub time: f64,
    pub norm_time: f64,
    pub norm_for: Option<Action>,
    pub on_removed: OnRemoved,
    pub swap: Option<(f64, u8)>,
    /// gcsim's Info.QueueAction for the acting character: its task
    /// `Task::Char` (id, arg) runs once the action has lasted delay
    /// frames, and not at all if another action replaces it first.
    pub queued: Option<(i32, u8, i32)>,
    /// Pauses on the acting character's hitlag; a swap does not.
    pub pausable: bool,
}

impl ActInfo {
    pub fn new(frames: Frames, anim_len: i32, can_queue_after: i32, state: AnimState) -> Self {
        ActInfo { frames, anim_len, can_queue_after, state, char: 0, time: 0.0, norm_time: 0.0, norm_for: None, on_removed: OnRemoved::None, swap: None, queued: None, pausable: true }
    }
}

pub fn atk_spd_adjust(f: i32, atkspd: f64) -> i32 {
    let a = atkspd.min(0.6);
    f - (a.min(0.1 + (a - 0.1) / 2.0) * f as f64) as i32
}

// ---------------------------------------------------------------- characters

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WeaponClass {
    Sword,
    Claymore,
    Spear,
    Bow,
    Catalyst,
}

impl WeaponClass {
    /// From gcsim's info.WeaponClass.
    pub fn from_gcsim(v: u8) -> Self {
        [WeaponClass::Sword, WeaponClass::Claymore, WeaponClass::Spear, WeaponClass::Bow, WeaponClass::Catalyst][v as usize]
    }
}

/// gcsim's info.BodyType.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Body {
    Boy,
    Girl,
    Male,
    Lady,
    Loli,
}

impl Body {
    pub fn from_gcsim(v: u8) -> Self {
        [Body::Boy, Body::Girl, Body::Male, Body::Lady, Body::Loli][v as usize]
    }
}

/// An artifact set a character wears, with two pieces or more.
pub struct SetSlot {
    pub kind: SetKind,
    pub count: i32,
    pub state: SetState,
}

#[derive(Clone, Copy, Default)]
pub struct Cooldown {
    pub available: i32,
    pub max_extra: i32,
    pub queue: [i32; 4],
    pub len: usize,
    pub started: i32,
    pub token: u32,
}

pub struct Char {
    pub kind: CharKind,
    /// gcsim's character, weapon and set keys, which fields compare.
    pub key: u16,
    pub weapon_key: u16,
    pub set_counts: Vec<(u16, i32)>,
    pub base: Stats,
    pub level: i32,
    pub cons: i32,
    pub ascension: i32,
    pub talents: [i32; 3],
    pub element: Element,
    pub class: WeaponClass,
    pub body: Body,
    /// The constellations that raise normal attack, skill and burst by 3.
    pub talent_cons: [i32; 3],
    /// AnimationStartDelay by gcsim's AnimationDelayKey.
    pub anim_delay: [i32; 3],
    pub weapon: WeaponKind,
    pub weapon_state: WeaponState,
    pub refine: i32,
    pub weapon_params: Vec<(String, i64)>,
    pub sets: Vec<SetSlot>,
    pub energy: f64,
    pub energy_max: f64,
    pub hp_ratio: f64,
    pub normal_counter: i32,
    pub normal_hit_num: i32,
    pub mods: Vec<Mod>,
    pub frozen: i32,
    pub time_passed: i32,
    pub queue: Queue<Task>,
    pub cd: [Cooldown; 3],
    pub remaining_dash_cd: i32,
    pub dash_lockout: bool,
    pub dash_len: i32,
    pub dash_jump: i32,
    /// The character module's own state.
    pub state: CharState,
    /// gcsim's IsHexerei: a Hexerei character, unless its hexerei param
    /// is 0.
    pub hexerei: bool,
}

impl Char {
    /// A weapon param, 0 when unset as in gcsim.
    pub fn weapon_param(&self, k: &str) -> i64 {
        self.weapon_params.iter().find(|p| p.0 == k).map_or(0, |p| p.1)
    }

    pub fn cd_index(a: Action) -> usize {
        match a {
            Action::Skill => 0,
            Action::Burst => 1,
            _ => 2,
        }
    }
}

// ---------------------------------------------------------------- enemy

#[derive(Clone, Copy, Default)]
pub struct IcdState {
    pub char: u8,
    pub tag: u16,
    pub group: u16,
    pub tag_on: bool,
    pub tag_count: usize,
    pub dmg_on: bool,
    pub dmg_count: usize,
}

pub const ZERO_DUR: f64 = 0.00000000001;
/// Aura slots (gcsim's ReactionModKeys): electro, pyro, hydro, cryo,
/// frozen, dendro, quicken, burning fuel and burning.
pub const AURA_ELECTRO: usize = 0;
pub const AURA_PYRO: usize = 1;
pub const AURA_HYDRO: usize = 2;
pub const AURA_CRYO: usize = 3;
pub const AURA_FROZEN: usize = 4;
pub const AURA_DENDRO: usize = 5;
pub const AURA_QUICKEN: usize = 6;
pub const AURA_BURNING_FUEL: usize = 7;
pub const AURA_BURNING: usize = 8;
const N_AURAS: usize = 9;
/// Frozen's decay ramps up by this each frame frozen lasts, and down by
/// twice it to the cap after (gcsim's frzDelta and frzDecayCap).
const FRZ_DELTA: f64 = 2.5 / (60.0 * 60.0);
const FRZ_DECAY_CAP: f64 = 10.0 / 60.0;

#[derive(Clone, Copy, Default)]
pub struct Aura {
    /// Durability by aura and by the character that applied it.
    pub dur: [[f64; 4]; N_AURAS],
    pub decay: [f64; N_AURAS],
    /// Bit m set while slot m may hold durability (gcsim's live): tick
    /// skips the slots that have nothing to decay.
    pub live: u16,
    /// The frame burning's ticks started, -1 without burning (gcsim's
    /// burningTickSrc).
    pub burning_src: i32,
}

impl Aura {
    /// A reactable's aura as gcsim's Init sets it up.
    pub fn new() -> Aura {
        let mut a = Aura::default();
        a.decay[AURA_FROZEN] = FRZ_DECAY_CAP;
        a.burning_src = -1;
        a
    }
    #[inline]
    pub fn get(&self, m: usize) -> f64 {
        let d = &self.dur[m];
        d[0].max(d[1]).max(d[2]).max(d[3])
    }
    fn above(&self, m: usize, x: f64) -> bool {
        self.dur[m].iter().any(|&d| d > x)
    }
    fn below(&self, m: usize, x: f64) -> bool {
        self.dur[m].iter().all(|&d| d < x)
    }
    fn at_most(&self, m: usize, x: f64) -> bool {
        self.dur[m].iter().all(|&d| d <= x)
    }
    fn reduce_mod(&mut self, m: usize, amt: f64) {
        for d in &mut self.dur[m] {
            *d -= amt.min(*d);
        }
        self.live |= 1 << m;
    }
    /// gcsim's removeMod.
    fn remove(&mut self, m: usize) {
        self.dur[m] = [0.0; 4];
        self.decay[m] = 0.0;
    }
    /// gcsim's reduce of the element of slot m: takes up to dur*factor off
    /// every slot of that element (burning is pyro, burning fuel dendro)
    /// and returns the attack durability the largest consumed.
    fn reduce(&mut self, m: usize, dur: f64, factor: f64) -> f64 {
        let (a, b) = match m {
            AURA_PYRO => (AURA_PYRO, Some(AURA_BURNING)),
            AURA_DENDRO => (AURA_BURNING_FUEL, Some(AURA_DENDRO)),
            _ => (m, None),
        };
        let mut reduced = self.reduce_one(a, dur * factor);
        if let Some(b) = b {
            reduced = reduced.max(self.reduce_one(b, dur * factor));
        }
        reduced / factor
    }
    fn reduce_one(&mut self, m: usize, max: f64) -> f64 {
        if self.below(m, ZERO_DUR) {
            return 0.0;
        }
        let red = max.min(self.get(m));
        self.reduce_mod(m, red);
        red
    }
    /// gcsim's AuraContains for one element: every slot of the element,
    /// and frozen for cryo.
    pub fn contains(&self, e: Element) -> bool {
        let has = |m: usize| self.get(m) > ZERO_DUR;
        match e {
            Element::Electro => has(AURA_ELECTRO),
            Element::Pyro => has(AURA_PYRO) || has(AURA_BURNING),
            Element::Hydro => has(AURA_HYDRO),
            Element::Cryo => has(AURA_CRYO) || has(AURA_FROZEN),
            Element::Frozen => has(AURA_FROZEN),
            Element::Dendro => has(AURA_BURNING_FUEL) || has(AURA_DENDRO),
            Element::Quicken => has(AURA_QUICKEN),
            _ => false,
        }
    }
    /// The part of Tick for burning fuel (which decays at its own rate),
    /// burning, dendro and quicken, in gcsim's order.
    #[inline(never)]
    fn tick_dendro(&mut self) {
        self.decay_own(AURA_BURNING_FUEL);
        // burning ends with its fuel, and takes dendro and quicken with it
        if self.burning_src > -1 && self.below(AURA_BURNING_FUEL, ZERO_DUR) {
            self.burning_src = -1;
            self.remove(AURA_BURNING);
            self.remove(AURA_DENDRO);
            self.remove(AURA_QUICKEN);
        }
        // dendro and quicken decay at burning fuel's rate while it lasts
        for m in [AURA_DENDRO, AURA_QUICKEN] {
            if self.live & 1 << m == 0 {
                continue;
            }
            if self.below(m, ZERO_DUR) {
                self.live &= !(1 << m);
                continue;
            }
            let mut rate = self.decay[m];
            if self.live & 1 << AURA_BURNING_FUEL != 0 && self.above(AURA_BURNING_FUEL, ZERO_DUR) {
                rate = self.decay[AURA_BURNING_FUEL];
                if m == AURA_DENDRO {
                    rate = rate.max(self.decay[m] * 2.0);
                }
            }
            self.reduce_mod(m, rate);
            if self.at_most(m, ZERO_DUR) {
                self.decay[m] = 0.0;
            }
        }
    }
    #[inline(always)]
    fn decay_own(&mut self, m: usize) {
        if self.live & 1 << m == 0 {
            return;
        }
        if self.decay[m] != 0.0 && self.above(m, ZERO_DUR) {
            self.reduce_mod(m, self.decay[m]);
            if self.at_most(m, ZERO_DUR) {
                self.decay[m] = 0.0;
            }
        } else if self.below(m, ZERO_DUR) {
            self.live &= !(1 << m);
        }
    }
    /// Nothing for Tick to do: no aura, and frozen's decay rate at its cap.
    #[inline(always)]
    fn idle(&self) -> bool {
        self.live == 0 && self.burning_src < 0 && self.decay[AURA_FROZEN] <= FRZ_DECAY_CAP
    }
    /// gcsim's Reactable.Tick; true when frozen ran out this tick (its
    /// checkFreeze, which the caller runs).
    fn tick(&mut self) -> bool {
        // nothing on it: only frozen's decay rate winds down
        if self.live == 0 && self.burning_src < 0 {
            let f = AURA_FROZEN;
            if self.decay[f] > FRZ_DECAY_CAP {
                self.decay[f] = (self.decay[f] - FRZ_DELTA * 2.0).max(FRZ_DECAY_CAP);
            }
            return false;
        }
        // the slots that decay at their own rate
        self.decay_own(AURA_ELECTRO);
        self.decay_own(AURA_PYRO);
        self.decay_own(AURA_HYDRO);
        self.decay_own(AURA_CRYO);
        // burning fuel, burning, dendro and quicken
        if self.live & (1 << AURA_BURNING_FUEL | 1 << AURA_DENDRO | 1 << AURA_QUICKEN) != 0 || self.burning_src > -1 {
            self.tick_dendro();
        }
        // frozen: d(t) = -1.25 (t/60)^2 - k (t/60) + d(0), with no freeze
        // resistance
        let f = AURA_FROZEN;
        if self.live & 1 << f != 0 && self.above(f, ZERO_DUR) {
            self.decay[f] += FRZ_DELTA;
            self.reduce_mod(f, self.decay[f]);
            return self.at_most(f, ZERO_DUR);
        } else if self.decay[f] > FRZ_DECAY_CAP {
            if self.below(f, ZERO_DUR) {
                self.live &= !(1 << f);
            }
            self.decay[f] -= FRZ_DELTA * 2.0;
            if self.decay[f] < FRZ_DECAY_CAP {
                self.decay[f] = FRZ_DECAY_CAP;
            }
        }
        false
    }
    /// gcsim's attachOverlapSrcs, for the sources in the mask.
    fn attach_overlap_srcs(&mut self, m: usize, amt: f64, length: f64, srcs: u8) {
        if self.get(m) > ZERO_DUR {
            for src in 0..4 {
                if srcs & 1 << src != 0 {
                    let add = (amt - self.dur[m][src]).max(0.0);
                    if add > 0.0 {
                        self.dur[m][src] += add;
                        self.live |= 1 << m;
                    }
                }
            }
        } else {
            for src in 0..4 {
                if srcs & 1 << src != 0 {
                    self.dur[m][src] = amt;
                    self.live |= 1 << m;
                }
            }
            if length > ZERO_DUR {
                self.decay[m] = amt / length;
            }
        }
    }
    fn attach_overlap(&mut self, m: usize, amt: f64, length: f64, src: usize) {
        if self.get(m) > ZERO_DUR {
            let add = (amt - self.dur[m][src]).max(0.0);
            if add > 0.0 {
                self.dur[m][src] += add;
                self.live |= 1 << m;
            }
        } else {
            self.dur[m][src] = amt;
            self.live |= 1 << m;
            if length > ZERO_DUR {
                self.decay[m] = amt / length;
            }
        }
    }
    fn attach_refresh(&mut self, m: usize, amt: f64, length: f64, src: usize) {
        if amt < self.get(m) {
            return;
        }
        self.dur[m][src] = amt;
        self.live |= 1 << m;
        if amt < self.get(m) {
            return;
        }
        self.decay[m] = amt / length;
    }
    /// gcsim's attachBurningFuel: burning fuel always overwrites.
    fn attach_burning_fuel(&mut self, dur: f64, mult: f64, src: usize) {
        self.remove(AURA_BURNING_FUEL);
        self.dur[AURA_BURNING_FUEL][src] = mult * dur;
        self.live |= 1 << AURA_BURNING_FUEL;
        self.decay[AURA_BURNING_FUEL] = (mult * dur / (6.0 * dur + 420.0)).max(10.0 / 60.0);
    }
    /// gcsim's attachBurning: 50 of burning that does not decay.
    fn attach_burning(&mut self, src: usize) {
        self.remove(AURA_BURNING);
        self.dur[AURA_BURNING][src] = 50.0;
        self.live |= 1 << AURA_BURNING;
    }
}

pub struct Enemy {
    pub level: i32,
    pub resist: [f64; 11],
    pub frozen: i32,
    pub queue: Queue<Task>,
    pub mods: Vec<Mod>,
    pub icd: Vec<IcdState>,
    pub aura: Aura,
    pub crystallize_gcd: i32,
    pub overload_gcd: i32,
    pub superconduct_gcd: i32,
    pub shatter_gcd: i32,
    /// Swirl's GCD by aura slot.
    pub swirl_gcd: [i32; N_AURAS],
    /// Electro-charged: the attack of its next ticks, the frame its ticks
    /// started (-1 without electro-charged), and whether the wane hook on
    /// enemy damage is subscribed.
    pub ec_atk: AttackEvent,
    pub ec_tick_src: i32,
    pub ec_wane_sub: bool,
    /// Burning: the attack of its ticks, recalculated whenever it is fed.
    pub burning_atk: AttackEvent,
}

// ---------------------------------------------------------------- player

pub struct LastAction {
    pub char: u8,
    pub action: Action,
}

pub struct Player {
    pub active: u8,
    pub stam: f64,
    pub last_stam_use: i32,
    pub swap_cd: i32,
    pub dash_cd_exp: i32,
    pub dash_lockout: bool,
    pub anim: Option<ActInfo>,
    pub state: AnimState,
    pub state_start: i32,
    pub last: LastAction,
}

// ---------------------------------------------------------------- sim

pub struct Config {
    pub chars: Vec<CharSetup>,
    pub active: usize,
    pub duration: f64,
    pub enemy_level: i32,
    /// Resistance by element, in Element order.
    pub resist: [f64; 11],
    /// energy every (start, end, amount), or once at start when once.
    pub energy: Option<(i32, i32, i32)>,
    pub energy_once: bool,
    pub swap_delay: i32,
    pub hitlag: bool,
    pub defhalt: bool,
    pub ignore_burst_energy: bool,
    /// The enemy's HP in damage mode (a target with hp); 0 otherwise.
    pub enemy_hp: f64,
    /// The enemy's distance from the player and its radius.
    pub target_dist: f64,
    pub target_radius: f64,
}

#[derive(Clone)]
pub struct CharSetup {
    pub kind: CharKind,
    pub key: u16,
    pub weapon_key: u16,
    pub set_counts: Vec<(u16, i32)>,
    pub base: Stats,
    pub level: i32,
    pub cons: i32,
    pub ascension: i32,
    pub talents: [i32; 3],
    pub weapon: WeaponKind,
    pub refine: i32,
    /// The sets worn with two pieces or more.
    pub sets: Vec<(SetKind, i32)>,
    /// gcsim's start_hp, start_hp% and start_energy params; -1 when unset.
    pub start_hp: f64,
    pub start_hp_ratio: f64,
    pub start_energy: f64,
    /// The character's own params (char ... key=value).
    pub params: Vec<(String, i64)>,
    /// The weapon's params (add weapon=... +params=[...]); a weapon reads
    /// the ones its gcsim package reads.
    pub weapon_params: Vec<(String, i64)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Construct {
    pub owner: u8,
    pub key: ModKey,
    /// The frame it was made.
    pub src: i32,
    pub expiry: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct Infusion {
    pub key: ModKey,
    pub ele: Element,
    /// The attack tags it applies to, as bits.
    pub tags: u32,
    /// Hitlag extends it by fractions of a frame.
    pub expiry: f64,
    pub can_be_overridden: bool,
}

/// A gadget (gcsim's gadget.Gadget) that a character owns: it ticks in
/// Combat.Tick after the enemy, with an aura of its own that absorb checks
/// read. `id` says which of its owner's gadgets it is.
#[derive(Clone, Copy)]
pub struct Gadget {
    pub owner: u8,
    pub id: u8,
    /// Ticks since it was added (the first tick is the frame after).
    pub timer: i32,
    /// Ticks left, -1 for no end.
    pub duration: i32,
    pub aura: Aura,
    /// A dendro core (owner CORE): the frame it appeared, where it is, and
    /// the character whose bloom made it.
    pub src: i32,
    /// Where it is and its radius, from the player.
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub actor: u8,
    /// Its owner's gadget_hit sees the attacks that reach it (gcsim's
    /// HandleAttack of a gadget of its own).
    pub takes_hits: bool,
    /// Of the enemy camp (gcsim's GadgetTyp between StartGadgetTypEnemy
    /// and EndGadgetTypEnemy: dendro cores, the Bogglecat Box), which
    /// abilities that pick gadgets as targets choose from.
    pub enemy_camp: bool,
}

/// The owner of dendro cores, which belong to no character.
pub const CORE: u8 = u8::MAX;

/// gcsim's shield.Type, for the shields the engine makes: a new shield
/// replaces the one of its type with the same target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShieldType {
    Crystallize,
    ZhongliJadeShield,
    CandaceSkill,
    DionaSkill,
    BaizhuBurst,
}

/// A shield (gcsim's shield.Tmpl): HP absorbing damage, stronger against
/// its own element.
#[derive(Clone, Copy, Debug)]
pub struct Shield {
    pub owner: u8,
    pub key: ModKey,
    pub typ: ShieldType,
    /// The character it protects, or -1 for whoever is active.
    pub target: i8,
    pub ele: Element,
    pub hp: f64,
    pub expiry: i32,
    /// Crystallize's bonus from its maker's EM.
    pub em_bonus: f64,
}

impl Shield {
    /// gcsim's Tmpl.OnDamage: the damage left over and whether the shield
    /// holds. (Its geo factor tests the damage's element, as gcsim's does.)
    fn on_damage(&mut self, dmg: f64, ele: Element, bonus: f64) -> (f64, bool) {
        let mut same = if ele == self.ele { 2.5 } else { 1.0 };
        if ele == Element::Geo {
            same = 1.5;
        }
        let block = self.hp * same * (1.0 + bonus + self.em_bonus);
        self.hp = (self.hp - self.hp * (dmg / block)).max(0.0);
        ((dmg - block).max(0.0), self.hp != 0.0)
    }
}

/// A shield bonus mod (gcsim's AddShieldBonusMod): geo resonance's from
/// the fight, or one a character owns.
#[derive(Clone, Copy, Debug)]
pub struct ShieldBonusMod {
    pub key: ModKey,
    pub owner: Owner,
    pub expiry: i32,
}

/// A crystallize shard (gcsim's crystallize.Shard): picked up, it gives
/// the shield it carries for 15.1 s.
#[derive(Clone, Copy, Debug)]
pub struct Shard {
    pub src: i32,
    /// The frame it is gone (15 s after it appeared).
    pub expiry: i32,
    pub earliest_pickup: i32,
    pub shield: Shield,
}

pub struct Sim {
    pub f: i32,
    pub rng: Rng,
    pub core: Queue<Task>,
    pub chars: Vec<Char>,
    pub enemy: Enemy,
    /// The player's own aura (gcsim's avatar.Player is a Reactable), which
    /// self infusions apply and absorb checks read.
    pub player_aura: Aura,
    /// The player's own ICD for damage (gcsim's Target ICD of the player).
    pub player_icd: Vec<IcdState>,
    pub player: Player,
    pub slots: Vec<AttackEvent>,
    pub free: Vec<u16>,
    pub end: i32,
    pub swap_delay: i32,
    pub energy: Option<(i32, i32, i32)>,
    pub energy_once: bool,
    pub last_energy_drop: i32,
    pub hitlag: bool,
    pub defhalt: bool,
    pub ignore_burst_energy: bool,
    /// Frames to wait before the next action (gcsim's preActionDelay).
    pub pre_delay: i32,
    /// Particle delay by character (set_particle_delay).
    pub particle_delay: [i32; 4],
    /// The swap cooldown a swap starts (set_swap_icd).
    pub swap_icd: i32,
    starts: Vec<(f64, f64)>,
    /// The fight is over.
    pub done: bool,
    /// What the fight found it does not implement, which ends it.
    pub error: Option<String>,
    /// Damage mode: the enemy has HP, and the fight ends when it dies or
    /// the program has no more actions.
    pub damage_mode: bool,
    pub enemy_hp: f64,
    pub target_dist: f64,
    pub target_radius: f64,
    pub enemy_alive: bool,
    pub no_more_actions: bool,
    /// The holder of gcsim's "utility-dash" stamina mod (dash stamina
    /// -20% while it lives), if any.
    pub utility_dash: Option<u8>,
    /// Diona's A1: stamina -10% while her shield is up.
    pub diona_a1: bool,
    /// Electro resonance, with the frame of its last particle (0 before
    /// the first).
    pub electro_res: Option<i32>,
    /// Dendro resonance.
    pub dendro_res: bool,
    /// Anemo resonance: cooldowns -5%, stamina -15%.
    pub anemo_res: bool,
    /// Swirl's GCD on the player's own aura, by slot.
    pub player_swirl_gcd: [i32; N_AURAS],
    /// Subscribers to each event, in the order gcsim subscribes them.
    pub subs: [Vec<Owner>; N_EVENTS],
    /// Constructs (gcsim's Constructs) and shields, by owner.
    pub constructs: Vec<Construct>,
    /// Gadgets, in the order they were added.
    pub gadgets: Vec<Gadget>,
    /// Crystallize shards. gcsim keeps them among the gadgets; nothing
    /// reaches them but pick_up_crystallize, so their order among the
    /// others does not matter.
    pub shards: Vec<Shard>,
    pub shields: Vec<Shield>,
    pub shield_bonus_mods: Vec<ShieldBonusMod>,
    /// Geo resonance: the frame its ticker started, -1 without it.
    pub geo_res_src: i32,
    /// The first frame a shield or a shard expires (i32::MAX for none).
    next_expiry: i32,
    /// Weapon infusions by character (gcsim's infusion handler).
    pub infusion: [Option<Infusion>; 4],
    /// Statuses of the whole fight (gcsim's Status), with their expiry.
    pub statuses: Vec<(ModKey, i32)>,
    /// gcsim's Flags.Custom: values the whole fight shares.
    pub flags: Vec<(ModKey, f64)>,
    /// Flat damage EnemyHit subscribers add to the hit.
    pub hit_flat: f64,
    // normal attack energy
    pub na_icd: i32,
    pub na_chance: [f64; 4],
    // results
    pub window_start: i32,
    pub total: f64,
    pub window_char: [f64; 4],
    /// Frames in the window that actions waited, by character and reason.
    pub waits: [[u32; N_FAILURES]; 4],
    /// gcsim's active failures: by character and action, the frame an
    /// action first failed and why, until it is used. The whole wait counts
    /// under that first reason.
    wait_open: [[Option<(i32, Failure)>; N_ACTIONS]; 4],
    /// Hits and damage in the window by character and ability.
    pub abils: [Vec<(&'static str, u32, f64)>; 4],
}

impl Sim {
    pub fn new(cfg: &Config, seed: u64, warmup: f64) -> Sim {
        let chars: Vec<Char> = cfg.chars.iter().map(Sim::make_char).collect();
        let mut s = Sim {
            f: 0,
            rng: Rng::new(seed),
            core: Queue::new(),
            chars,
            player_aura: Aura::new(),
            player_icd: Vec::new(),
            enemy: Enemy {
                level: cfg.enemy_level,
                resist: cfg.resist,
                frozen: 0,
                queue: Queue::new(),
                mods: Vec::with_capacity(16),
                icd: Vec::with_capacity(16),
                aura: Aura::new(),
                crystallize_gcd: -1,
                overload_gcd: -1,
                superconduct_gcd: -1,
                shatter_gcd: -1,
                swirl_gcd: [-1; N_AURAS],
                ec_atk: AttackEvent::new(AttackInfo::new(0, AttackTag::ECDamage, icd::TAG_EC_DAMAGE, icd::GROUP_REACTION_B, Element::Electro, 0.0, 0.0), 0),
                ec_tick_src: -1,
                ec_wane_sub: false,
                burning_atk: AttackEvent::new(AttackInfo::new(0, AttackTag::BurningDamage, icd::TAG_BURNING_DAMAGE, icd::GROUP_BURNING, Element::Pyro, 25.0, 0.0), 0),
            },
            player: Player {
                active: cfg.active as u8,
                stam: 240.0,
                last_stam_use: 0,
                swap_cd: 0,
                dash_cd_exp: 0,
                dash_lockout: false,
                anim: None,
                state: AnimState::Idle,
                state_start: 0,
                last: LastAction { char: 0, action: Action::Invalid },
            },
            slots: Vec::with_capacity(64),
            free: Vec::with_capacity(64),
            end: (cfg.duration * 60.0) as i32,
            swap_delay: cfg.swap_delay,
            energy: cfg.energy,
            energy_once: cfg.energy_once,
            last_energy_drop: 0,
            hitlag: cfg.hitlag,
            defhalt: cfg.defhalt,
            ignore_burst_energy: cfg.ignore_burst_energy,
            pre_delay: 0,
            particle_delay: [100; 4],
            swap_icd: 60,
            starts: cfg.chars.iter().map(|c| (c.start_hp, c.start_hp_ratio)).collect(),
            done: false,
            error: None,
            damage_mode: cfg.enemy_hp > 0.0,
            enemy_hp: cfg.enemy_hp,
            target_dist: cfg.target_dist,
            target_radius: cfg.target_radius,
            enemy_alive: true,
            no_more_actions: false,
            utility_dash: None,
            diona_a1: false,
            electro_res: None,
            dendro_res: false,
            anemo_res: false,
            player_swirl_gcd: [-1; N_AURAS],
            subs: Default::default(),
            constructs: Vec::with_capacity(4),
            gadgets: Vec::with_capacity(4),
            shards: Vec::with_capacity(4),
            shields: Vec::with_capacity(4),
            shield_bonus_mods: Vec::new(),
            geo_res_src: -1,
            next_expiry: i32::MAX,
            infusion: [None; 4],
            statuses: Vec::new(),
            flags: Vec::new(),
            hit_flat: 0.0,
            na_icd: 0,
            na_chance: [0.0; 4],
            window_start: ((warmup * 60.0) as i32 / 30) * 30,
            total: 0.0,
            window_char: [0.0; 4],
            waits: [[0; N_FAILURES]; 4],
            wait_open: [[None; N_ACTIONS]; 4],
            abils: Default::default(),
        };
        s.setup();
        for (i, c) in cfg.chars.iter().enumerate() {
            if c.start_energy >= 0.0 {
                s.chars[i].energy = c.start_energy.min(s.chars[i].energy_max);
            }
        }
        // initialize advances one frame before the first action
        s.done = s.next_frame() || s.error.is_some();
        s
    }

    fn make_char(cs: &CharSetup) -> Char {
        let d = crate::data::CHARS.iter().find(|d| d.key == crate::gcsl::char_name(cs.key)).expect("character data");
        let (element, class, body) = (Element::from_gcsim(d.element), WeaponClass::from_gcsim(d.weapon_class), Body::from_gcsim(d.body));
        let (energy_max, normal_hit_num) = (d.energy_max, d.normal_hit_num);
        // the template character's DashLength and DashToJumpLength
        let dash_len = match body {
            Body::Boy | Body::Loli => 21,
            Body::Male => 19,
            Body::Lady => 22,
            Body::Girl => 20,
        };
        let dash_jump = match body {
            Body::Girl | Body::Loli => 4,
            Body::Boy => 2,
            _ => 3,
        };
        Char {
            kind: cs.kind,
            key: cs.key,
            weapon_key: cs.weapon_key,
            set_counts: cs.set_counts.clone(),
            base: cs.base,
            level: cs.level,
            cons: cs.cons,
            ascension: cs.ascension,
            talents: cs.talents,
            element,
            class,
            body,
            talent_cons: d.cons,
            anim_delay: d.anim_delay,
            weapon: cs.weapon,
            weapon_state: cs.weapon.new_state(),
            refine: cs.refine,
            weapon_params: cs.weapon_params.clone(),
            sets: cs.sets.iter().map(|&(kind, count)| SetSlot { kind, count, state: kind.new_state() }).collect(),
            energy: energy_max,
            energy_max,
            hp_ratio: 1.0,
            normal_counter: 0,
            normal_hit_num,
            mods: Vec::with_capacity(24),
            frozen: 0,
            time_passed: 0,
            queue: Queue::new(),
            cd: [Cooldown { available: 1, ..Default::default() }; 3],
            remaining_dash_cd: 0,
            dash_lockout: false,
            dash_len,
            dash_jump,
            state: cs.kind.new_state(cs),
            hexerei: crate::profile::HEXEREI.contains(&crate::gcsl::char_name(cs.key)) && !cs.params.iter().any(|(k, v)| k == "hexerei" && *v == 0),
        }
    }

    /// gcsim's simulation.New and initialize: characters with their weapons
    /// and sets, resonance, character inits, then one frame.
    fn setup(&mut self) {
        let n = self.chars.len();
        // AddChar: the weapon, then the sets
        for i in 0..n {
            crate::weapons::dispatch::init(self.chars[i].weapon, self, i);
            for si in 0..self.chars[i].sets.len() {
                let (k, count) = (self.chars[i].sets[si].kind, self.chars[i].sets[si].count);
                crate::sets::dispatch::init(k, self, i, si, count);
            }
        }
        // resonance; profile::config rejects the ones not implemented
        if n >= 4 {
            let count = |e: Element| self.chars.iter().filter(|c| c.element == e).count();
            let (pyro, hydro, electro, cryo, dendro, geo, anemo) = (count(Element::Pyro), count(Element::Hydro), count(Element::Electro), count(Element::Cryo), count(Element::Dendro), count(Element::Geo), count(Element::Anemo));
            self.anemo_res = anemo >= 2;
            for c in &mut self.chars {
                if geo >= 2 {
                    c.mods.push(Mod::owned(GEO_RESONANCE, ModKind::Attack, Owner::Core, st::NONE, false, false, -1));
                }
                if dendro >= 2 {
                    c.mods.push(Mod::stat(DENDRO_RESONANCE, st::EM, 50.0, false, false, -1));
                }
                if cryo >= 2 {
                    c.mods.push(Mod::owned(CRYO_RESONANCE, ModKind::Attack, Owner::Core, st::NONE, false, false, -1));
                }
                if pyro >= 2 {
                    c.mods.push(Mod::stat(PYRO_RESONANCE, st::ATKP, 0.25, false, false, -1));
                }
                if hydro >= 2 {
                    c.mods.push(Mod::stat(HYDRO_RESONANCE, st::HPP, 0.25, false, false, -1));
                }
            }
            if electro >= 2 {
                self.electro_res = Some(0);
            }
            self.dendro_res = dendro >= 2;
            if electro >= 2 || dendro >= 2 {
                self.subscribe(EventKind::Reaction, Owner::Core);
            }
            // geo resonance: shield strength +15%, and DMG and geo shred
            // while shielded; its ticker starts with the first swap or
            // shield
            if geo >= 2 {
                self.shield_bonus_mods.push(ShieldBonusMod { key: GEO_RESONANCE, owner: Owner::Core, expiry: -1 });
                self.geo_res_src = 0;
                self.subscribe(EventKind::Shielded, Owner::Core);
                self.subscribe(EventKind::ShieldBreak, Owner::Core);
            }
        }
        // Core.Init: energy from normal attacks, then the characters' inits
        for (i, c) in self.chars.iter().enumerate() {
            if c.class == WeaponClass::Sword {
                self.na_chance[i] = 0.10;
            }
        }
        self.subscribe(EventKind::EnemyDamage, Owner::Core);
        self.subscribe(EventKind::Swap, Owner::Core);
        for i in 0..n {
            crate::chars::dispatch::init(self.chars[i].kind, self, i);
            crate::weapons::dispatch::post_init(self.chars[i].weapon, self, i);
            for si in 0..self.chars[i].sets.len() {
                crate::sets::dispatch::post_init(self.chars[i].sets[si].kind, self, i, si);
            }
        }
        for i in 0..n {
            let (hp, ratio) = (self.starts[i].0, self.starts[i].1);
            match (hp > 0.0, ratio > 0.0) {
                (true, true) => {
                    self.chars[i].hp_ratio = (ratio / 100.0).clamp(0.0, 1.0);
                    self.modify_hp(i, hp);
                }
                (true, false) => self.set_hp(i, hp),
                (false, true) => self.chars[i].hp_ratio = (ratio / 100.0).clamp(0.0, 1.0),
                _ => self.chars[i].hp_ratio = 1.0,
            }
        }
    }

    pub fn talent(&self, ci: usize, i: usize) -> usize {
        let c = &self.chars[ci];
        let con = c.talent_cons[i];
        let add = if con > 0 && c.cons >= con { 3 } else { 0 };
        (c.talents[i] - 1 + add) as usize
    }

    /// The state of the character in slot ci, of its module's State type.
    #[inline]
    pub fn st<T: StateOf<CharState>>(&self, ci: usize) -> &T {
        T::of(&self.chars[ci].state)
    }
    #[inline]
    pub fn st_mut<T: StateOf<CharState>>(&mut self, ci: usize) -> &mut T {
        T::of_mut(&mut self.chars[ci].state)
    }
    #[inline]
    pub fn weapon_st<T: StateOf<WeaponState>>(&self, ci: usize) -> &T {
        T::of(&self.chars[ci].weapon_state)
    }
    #[inline]
    pub fn weapon_st_mut<T: StateOf<WeaponState>>(&mut self, ci: usize) -> &mut T {
        T::of_mut(&mut self.chars[ci].weapon_state)
    }
    #[inline]
    pub fn set_st<T: StateOf<SetState>>(&self, ci: usize, si: usize) -> &T {
        T::of(&self.chars[ci].sets[si].state)
    }
    #[inline]
    pub fn set_st_mut<T: StateOf<SetState>>(&mut self, ci: usize, si: usize) -> &mut T {
        T::of_mut(&mut self.chars[ci].sets[si].state)
    }

    // ------------------------------------------------------------ events

    pub fn subscribe(&mut self, e: EventKind, o: Owner) {
        self.subs[e as usize].push(o);
    }

    /// Runs an event's subscribers, in the order they subscribed.
    pub fn emit(&mut self, e: Event) {
        let k = e.kind() as usize;
        for i in 0..self.subs[k].len() {
            match self.subs[k][i] {
                Owner::Char(c) => crate::chars::dispatch::event(self.chars[c as usize].kind, self, c as usize, &e),
                Owner::Weapon(c) => crate::weapons::dispatch::event(self.chars[c as usize].weapon, self, c as usize, &e),
                Owner::Set(c, si) => {
                    let kind = self.chars[c as usize].sets[si as usize].kind;
                    crate::sets::dispatch::event(kind, self, c as usize, si as usize, &e)
                }
                Owner::Core => self.core_event(&e),
            }
        }
    }

    /// gcsim's SetupOnNormalHitEnergy.
    fn core_event(&mut self, e: &Event) {
        match *e {
            Event::EnemyDamage(ev) => {
                if self.geo_res_src >= 0 && self.flag(GEO_RESONANCE) == 1.0 && self.player.active == ev.info.actor {
                    self.enemy_add_resist(GEO_RESONANCE, Element::Geo, -0.2, 15 * 60);
                }
                // SetupMisc: superconduct shreds physical
                if ev.info.tag == AttackTag::SuperconductDamage {
                    self.enemy_add_resist(SUPERCONDUCT_SHRED, Element::Physical, -0.4, 12 * 60);
                }
                let ci = ev.info.actor as usize;
                if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) && self.na_icd <= self.f {
                    let (inc, reset) = match self.chars[ci].class {
                        WeaponClass::Sword => (0.05, 0.10),
                        WeaponClass::Claymore => (0.10, 0.0),
                        WeaponClass::Spear => (0.04, 0.0),
                        WeaponClass::Bow => (0.05, 0.0),
                        WeaponClass::Catalyst => (0.10, 0.0),
                    };
                    if self.rng.float64() > self.na_chance[ci] {
                        self.na_chance[ci] += inc;
                    } else {
                        self.add_energy(ci, 1.0);
                        self.na_icd = self.f + 12;
                        self.na_chance[ci] = reset;
                    }
                }
            }
            Event::Reaction(r, _) => {
                // electro resonance: an electro reaction gives a particle,
                // once every 5 s (hyperbloom's comes from the dendro core)
                if let Some(last) = self.electro_res {
                    if matches!(r, Reaction::Overload | Reaction::Superconduct | Reaction::ElectroCharged | Reaction::Quicken | Reaction::Aggravate | Reaction::Hyperbloom) && (self.f - last >= 300 || last == 0) {
                        self.distribute_particle(1.0, Element::Electro);
                        self.electro_res = Some(self.f);
                    }
                }
                // dendro resonance: EM to the party for 6 s
                if self.dendro_res {
                    let em = match r {
                        Reaction::Burning | Reaction::Bloom | Reaction::Quicken => Some((DENDRO_RESONANCE_30, 30.0)),
                        Reaction::Aggravate | Reaction::Spread | Reaction::Hyperbloom | Reaction::Burgeon => Some((DENDRO_RESONANCE_20, 20.0)),
                        _ => None,
                    };
                    if let Some((key, v)) = em {
                        for i in 0..self.chars.len() {
                            self.add_stat_mod(i, key, st::EM, v, 6 * 60, true);
                        }
                    }
                }
            }
            Event::Swap { .. } => {
                if self.geo_res_src >= 0 {
                    self.geo_res_src = self.f;
                    self.geo_res_update(self.f);
                }
                for (i, c) in self.chars.iter().enumerate() {
                    self.na_chance[i] = if c.class == WeaponClass::Sword { 0.10 } else { 0.0 };
                }
            }
            Event::Shielded(_) | Event::ShieldBreak(_) => {
                self.geo_res_src = self.f;
                self.geo_res_update(self.f);
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------ slots

    pub fn alloc(&mut self, ev: AttackEvent) -> u16 {
        if let Some(i) = self.free.pop() {
            self.slots[i as usize] = ev;
            i
        } else {
            self.slots.push(ev);
            (self.slots.len() - 1) as u16
        }
    }

    // ------------------------------------------------------------ stats

    #[inline]
    pub fn mod_amount(&self, ci: usize, m: &Mod, out: &mut Stats) -> bool {
        match m.amount {
            Amount::Fixed => {
                out[m.affected] += m.value;
                true
            }
            Amount::Owner(Owner::Char(o)) => crate::chars::dispatch::mod_amount(self.chars[o as usize].kind, self, o as usize, ci, m, out),
            Amount::Owner(Owner::Weapon(o)) => crate::weapons::dispatch::mod_amount(self.chars[o as usize].weapon, self, o as usize, ci, m, out),
            Amount::Owner(Owner::Set(o, si)) => {
                let kind = self.chars[o as usize].sets[si as usize].kind;
                crate::sets::dispatch::mod_amount(kind, self, o as usize, si as usize, ci, m, out)
            }
            Amount::Owner(Owner::Core) => false,
        }
    }

    /// gcsim's Stats(): base stats plus every stat mod in effect.
    pub fn stats(&self, ci: usize) -> Stats {
        let c = &self.chars[ci];
        let mut s = c.base;
        for m in &c.mods {
            if m.kind == ModKind::Stat && m.applies(self.f) {
                self.mod_amount(ci, m, &mut s);
            }
        }
        s
    }

    /// gcsim's SelectStat over the HP stats.
    pub fn max_hp(&self, ci: usize) -> f64 {
        let c = &self.chars[ci];
        let mut s = [0.0; N_STATS];
        s[st::BASEHP] = c.base[st::BASEHP];
        s[st::HPP] = c.base[st::HPP];
        s[st::HP] = c.base[st::HP];
        for m in &c.mods {
            if m.kind != ModKind::Stat || !(m.affected == st::NONE || m.affected == st::BASEHP || m.affected == st::HPP || m.affected == st::HP) {
                continue;
            }
            if m.applies(self.f) {
                let mut t = [0.0; N_STATS];
                if self.mod_amount(ci, m, &mut t) {
                    s[st::BASEHP] += t[st::BASEHP];
                    s[st::HPP] += t[st::HPP];
                    s[st::HP] += t[st::HP];
                }
            }
        }
        max_hp(&s)
    }

    /// gcsim's Stat(s), or NonExtraStat(s) with non_extra.
    pub fn stat_ex(&self, ci: usize, s: usize, non_extra: bool) -> f64 {
        let c = &self.chars[ci];
        let mut v = c.base[s];
        for m in &c.mods {
            if m.kind != ModKind::Stat || (m.affected != st::NONE && m.affected != s) || (non_extra && m.extra) {
                continue;
            }
            if m.applies(self.f) {
                let mut t = [0.0; N_STATS];
                if self.mod_amount(ci, m, &mut t) {
                    v += t[s];
                }
            }
        }
        v
    }

    pub fn stat(&self, ci: usize, s: usize) -> f64 {
        self.stat_ex(ci, s, false)
    }

    pub fn current_hp(&self, ci: usize) -> f64 {
        self.chars[ci].hp_ratio * self.max_hp(ci)
    }

    pub fn set_hp(&mut self, ci: usize, amt: f64) {
        let r = amt / self.max_hp(ci);
        self.chars[ci].hp_ratio = r.clamp(0.0, 1.0);
    }

    pub fn modify_hp(&mut self, ci: usize, amt: f64) {
        let hp = self.current_hp(ci) + amt;
        self.set_hp(ci, hp);
    }

    /// gcsim's ReactBonus: a character's bonus to a reaction it causes.
    pub fn react_bonus(&self, ci: usize, info: &AttackInfo) -> f64 {
        let mut amt = 0.0;
        for m in &self.chars[ci].mods {
            if m.kind != ModKind::ReactBonus || !m.applies(self.f) {
                continue;
            }
            amt += match m.amount {
                Amount::Owner(Owner::Char(o)) => crate::chars::dispatch::react_bonus(self.chars[o as usize].kind, self, o as usize, ci, m, info),
                Amount::Owner(Owner::Set(o, si)) => {
                    let kind = self.chars[o as usize].sets[si as usize].kind;
                    crate::sets::dispatch::react_bonus(kind, self, o as usize, si as usize, ci, m, info)
                }
                _ => 0.0,
            };
        }
        amt
    }

    /// gcsim's HealBonus: the incoming healing bonus of a character.
    pub fn heal_bonus(&self, ci: usize) -> f64 {
        let mut amt = 0.0;
        for m in &self.chars[ci].mods {
            if m.kind == ModKind::HealBonus && m.applies(self.f) {
                let mut t = [0.0; N_STATS];
                if self.mod_amount(ci, m, &mut t) {
                    amt += t[st::HEAL];
                }
            }
        }
        amt
    }

    /// gcsim's Player.Heal: target None heals the whole party, src is HP
    /// (or a fraction of each healed character's max HP when percent),
    /// bonus the healer's. There is no HP debt.
    pub fn heal_info(&mut self, caller: usize, target: Option<usize>, percent: bool, src: f64, bonus: f64) {
        match target {
            Some(t) => self.heal_one(caller, target, t, percent, src, bonus),
            None => {
                for t in 0..self.chars.len() {
                    self.heal_one(caller, target, t, percent, src, bonus);
                }
            }
        }
    }

    fn heal_one(&mut self, caller: usize, target: Option<usize>, on: usize, percent: bool, src: f64, bonus: f64) {
        let hp = if percent { self.max_hp(on) * src } else { src };
        let heal = hp * (1.0 + self.heal_bonus(on) + bonus);
        let prev = self.current_hp(on);
        self.modify_hp(on, heal);
        let overheal = (prev + heal - self.max_hp(on)).max(0.0);
        self.emit(Event::Heal { caller, target, on, amount: heal, overheal });
    }

    /// A heal of src HP by caller on one character.
    pub fn heal(&mut self, caller: usize, target: usize, src: f64, bonus: f64) {
        self.heal_info(caller, Some(target), false, src, bonus);
    }

    /// gcsim's Player.Drain: the character loses amount HP.
    pub fn drain(&mut self, on: usize, amount: f64) {
        self.modify_hp(on, -amount);
        self.emit(Event::Drain { on, amount, external: false });
    }

    // ------------------------------------------------------------ mods

    /// gcsim's AddStatMod and AddAttackMod: adds the mod for dur frames
    /// (forever when negative), replacing one with its key. Its expiry is
    /// set from dur; its amount and started frame come from m.
    pub fn add_mod(&mut self, ci: usize, mut m: Mod, dur: i32) {
        m.expiry_base = if dur < 0 { -1 } else { self.f + dur };
        m.extension = 0;
        mod_add(&mut self.chars[ci].mods, m);
    }

    /// A stat mod of a fixed amount for dur frames.
    pub fn add_stat_mod(&mut self, ci: usize, key: ModKey, stat: usize, value: f64, dur: i32, hitlag: bool) {
        self.add_mod(ci, Mod::stat(key, stat, value, false, hitlag, -1), dur);
    }

    pub fn add_status(&mut self, ci: usize, key: ModKey, dur: i32, hitlag: bool) {
        self.add_mod(ci, Mod::new(key, ModKind::Status, st::NONE, false, hitlag, -1), dur);
    }

    pub fn mod_active(&self, ci: usize, key: ModKey) -> bool {
        match mod_find(&self.chars[ci].mods, key) {
            Some(i) => self.chars[ci].mods[i].active(self.f),
            None => false,
        }
    }

    pub fn extend_mod(&mut self, ci: usize, key: ModKey, ext: i32) {
        let f = self.f;
        let mods = &mut self.chars[ci].mods;
        if let Some(i) = mod_find(mods, key) {
            if mods[i].active(f) {
                mods[i].extend(ext);
            }
        }
    }

    pub fn delete_mod(&mut self, ci: usize, key: ModKey) {
        self.chars[ci].mods.retain(|m| m.key != key);
    }

    pub fn enemy_status_active(&self, key: ModKey) -> bool {
        match mod_find(&self.enemy.mods, key) {
            Some(i) => self.enemy.mods[i].active(self.f),
            None => false,
        }
    }

    pub fn enemy_add_status(&mut self, key: ModKey, dur: i32, hitlag: bool) {
        let expiry = if dur < 0 { -1 } else { self.f + dur };
        mod_add(&mut self.enemy.mods, Mod::new(key, ModKind::Status, st::NONE, false, hitlag, expiry));
    }

    /// gcsim's AddDefMod, hitlag affected (NewBaseWithHitlag).
    pub fn enemy_add_def(&mut self, key: ModKey, value: f64, dur: i32) {
        let mut m = Mod::new(key, ModKind::Def, st::NONE, false, true, self.f + dur);
        m.value = value;
        mod_add(&mut self.enemy.mods, m);
    }

    pub fn enemy_add_resist(&mut self, key: ModKey, ele: Element, value: f64, dur: i32) {
        let mut m = Mod::new(key, ModKind::Resist, st::NONE, false, true, self.f + dur);
        m.ele = ele;
        m.value = value;
        mod_add(&mut self.enemy.mods, m);
    }

    // ------------------------------------------------------------ infusion and statuses

    /// gcsim's AddWeaponInfuse: the character's attacks with these tags
    /// take the element for dur frames.
    pub fn add_infusion(&mut self, ci: usize, key: ModKey, ele: Element, dur: i32, can_be_overridden: bool, tags: &[AttackTag]) {
        let f = self.f as f64;
        if let Some(cur) = &self.infusion[ci] {
            if !cur.can_be_overridden && cur.expiry > f {
                return;
            }
        }
        let tags = tags.iter().fold(0u32, |m, t| m | 1 << *t as u32);
        let expiry = if dur == -1 { -1.0 } else { (self.f + dur) as f64 };
        self.infusion[ci] = Some(Infusion { key, ele, tags, expiry, can_be_overridden });
    }

    /// gcsim's Infused.
    pub fn infused(&self, ci: usize, tag: AttackTag) -> Option<Element> {
        let inf = self.infusion[ci].as_ref()?;
        (inf.tags & (1 << tag as u32) != 0 && (inf.expiry > self.f as f64 || inf.expiry == -1.0)).then_some(inf.ele)
    }

    /// gcsim's Status.Add: a status of the whole fight, not hitlag affected.
    pub fn add_core_status(&mut self, key: ModKey, dur: i32) {
        let expiry = self.f + dur;
        match self.statuses.iter_mut().find(|s| s.0 == key) {
            Some(s) => s.1 = expiry,
            None => self.statuses.push((key, expiry)),
        }
    }

    /// gcsim's Flags.Custom[key], 0 when unset.
    pub fn flag(&self, key: ModKey) -> f64 {
        self.flags.iter().find(|f| f.0 == key).map_or(0.0, |f| f.1)
    }

    pub fn set_flag(&mut self, key: ModKey, v: f64) {
        match self.flags.iter_mut().find(|f| f.0 == key) {
            Some(f) => f.1 = v,
            None => self.flags.push((key, v)),
        }
    }

    /// gcsim's Status.Delete.
    pub fn delete_core_status(&mut self, key: ModKey) {
        self.statuses.retain(|s| s.0 != key);
    }

    /// gcsim's Status.Duration.
    pub fn core_status(&self, name: &str) -> i32 {
        self.statuses.iter().find(|s| s.0.name == name).map(|s| (s.1 - self.f).max(0)).unwrap_or(0)
    }

    // ------------------------------------------------------------ tasks

    pub fn queue_char_task(&mut self, ci: usize, t: Task, delay: i32) {
        if delay == 0 {
            self.run_task(t);
            return;
        }
        let f = self.f;
        self.chars[ci].queue.add(f, t, delay);
    }

    pub fn queue_enemy_task(&mut self, t: Task, delay: i32) {
        if delay == 0 {
            self.run_task(t);
            return;
        }
        let f = self.f;
        self.enemy.queue.add(f, t, delay);
    }

    pub fn add_task(&mut self, t: Task, delay: i32) {
        let f = self.f;
        self.core.add(f, t, delay);
    }

    fn run_task(&mut self, t: Task) {
        match t {
            Task::QueueAttack { slot, snap, dmg } => self.queue_attack_slot(slot, snap, dmg),
            Task::Snapshot { slot, dmg } => {
                self.snapshot(slot);
                self.queue_dmg(slot, dmg);
            }
            Task::ApplyAttack { slot } => self.apply_attack(slot),
            Task::DamageApplied { slot } => self.damage_applied(slot),
            Task::SetCd { c, a, dur } => self.set_cd(c as usize, a, dur),
            Task::CdWorker { c, a, token } => self.cd_worker(c as usize, a, token),
            Task::ConsumeEnergy { c, .. } => self.drain_energy(c as usize),
            Task::AddEnergy { c, amt } => self.add_energy(c as usize, amt),
            Task::Particle { num, ele } => self.distribute_particle(num, ele),
            Task::DashStam { c } => {
                let req = self.abil_stam_cost(c as usize, Action::Dash);
                self.use_stam(req);
            }
            Task::IcdResetDmg { i } => {
                let s = &mut self.enemy.icd[i as usize];
                s.dmg_count = 0;
                s.dmg_on = false;
            }
            Task::IcdResetTag { i } => {
                let s = &mut self.enemy.icd[i as usize];
                s.tag_count = 0;
                s.tag_on = false;
            }
            Task::Swap { to } => self.do_swap(to as usize),
            Task::Char { c, id, arg } => crate::chars::dispatch::task(self.chars[c as usize].kind, self, c as usize, id, arg),
            Task::Weapon { c, id, arg } => crate::weapons::dispatch::task(self.chars[c as usize].weapon, self, c as usize, id, arg),
            Task::EcTick { src } => self.ec_tick(src),
            Task::EcWane => self.ec_wane(),
            Task::BurningTick { src, counter } => self.burning_tick(src, counter),
            Task::QuickenBloom { slot } => self.quicken_bloom(slot),
            Task::SpawnCore { slot } => self.spawn_core(slot),
            Task::CoreAttack { kind, actor, on_player } => self.core_attack(kind, actor as usize, on_player),
            Task::GadgetAttack { slot } => {
                let ev = self.slots[slot as usize];
                self.free.push(slot);
                if !self.gadgets.is_empty() {
                    self.hit_gadgets(&ev);
                }
            }
            Task::SpawnShard { actor, ele } => self.spawn_shard(actor as usize, ele),
            Task::ShieldTrack => {
                self.shield_bonus();
            }
            Task::GeoRes { src } => self.geo_res_update(src),
            Task::PlayerIcdResetDmg { i } => {
                let s = &mut self.player_icd[i as usize];
                s.dmg_count = 0;
                s.dmg_on = false;
            }
            Task::Set { c, si, id, arg } => {
                let kind = self.chars[c as usize].sets[si as usize].kind;
                crate::sets::dispatch::task(kind, self, c as usize, si as usize, id, arg)
            }
        }
    }

    // ------------------------------------------------------------ attacks

    /// gcsim's QueueAttack for an event already in a slot.
    pub fn queue_attack_slot(&mut self, slot: u16, snap: i32, dmg: i32) {
        if snap < 0 {
            self.queue_dmg(slot, dmg);
        } else if snap == 0 {
            self.snapshot(slot);
            self.queue_dmg(slot, dmg);
        } else {
            self.add_task(Task::Snapshot { slot, dmg: dmg - snap }, snap);
        }
    }

    pub fn queue_attack(&mut self, info: AttackInfo, callbacks: u16, snap: i32, dmg: i32) {
        let slot = self.alloc(AttackEvent::new(info, callbacks));
        self.queue_attack_slot(slot, snap, dmg);
    }

    /// gcsim's QueueAttackWithSnap.
    pub fn queue_attack_with_snap(&mut self, mut ev: AttackEvent, callbacks: u16, dmg: i32) {
        ev.callbacks = callbacks;
        let slot = self.alloc(ev);
        self.queue_dmg(slot, dmg);
    }

    /// gcsim's QueueAttack (snapshot now) for an attack whose pattern
    /// misses the enemy: `dmg` frames later it reaches only the gadgets it
    /// covers.
    pub fn queue_gadget_attack(&mut self, info: AttackInfo, dmg: i32) {
        let ev = self.snapshot_event(info, 0);
        let slot = self.alloc(ev);
        self.add_task(Task::GadgetAttack { slot }, dmg);
    }

    /// Queues QueueAttack on a character's queue, as attacks queued at a
    /// hitmark with QueueCharTask are.
    pub fn queue_attack_at(&mut self, ci: usize, info: AttackInfo, callbacks: u16, delay: i32) {
        let slot = self.alloc(AttackEvent::new(info, callbacks));
        self.queue_char_task(ci, Task::QueueAttack { slot, snap: 0, dmg: 0 }, delay);
    }

    fn queue_dmg(&mut self, slot: u16, delay: i32) {
        if delay == 0 {
            self.apply_attack(slot);
        } else {
            self.add_task(Task::ApplyAttack { slot }, delay);
        }
    }

    /// A character's Snapshot of an attack.
    pub fn make_snapshot(&self, info: &mut AttackInfo) -> (Stats, i32) {
        let ci = info.actor as usize;
        let stats = self.stats(ci);
        if !info.ignore_infusion {
            if let Some(e) = self.infused(ci, info.tag) {
                info.element = e;
            }
        }
        crate::chars::dispatch::snapshot(self.chars[ci].kind, self, ci, info);
        (stats, self.chars[ci].level)
    }

    fn snapshot(&mut self, slot: u16) {
        let mut info = self.slots[slot as usize].info;
        let (stats, lvl) = self.make_snapshot(&mut info);
        let f = self.f;
        let ev = &mut self.slots[slot as usize];
        ev.stats = stats;
        ev.char_lvl = lvl;
        ev.snap_f = f;
        ev.info = info;
    }

    /// An attack with its snapshot taken now, for QueueAttackWithSnap.
    pub fn snapshot_event(&self, mut info: AttackInfo, callbacks: u16) -> AttackEvent {
        let (stats, lvl) = self.make_snapshot(&mut info);
        let mut ev = AttackEvent::new(info, callbacks);
        ev.stats = stats;
        ev.char_lvl = lvl;
        ev.snap_f = self.f;
        ev
    }

    fn apply_attack(&mut self, slot: u16) {
        // one enemy, always in range while it lives
        if !self.enemy_alive {
            self.free.push(slot);
            return;
        }
        // gadgets see the attack as it was before the enemy took it
        let orig = if self.gadgets.is_empty() { None } else { Some(self.slots[slot as usize]) };
        self.handle_attack(slot);
        if let Some(orig) = orig {
            self.hit_gadgets(&orig);
        }
        let info = self.slots[slot as usize].info;
        if self.hitlag && !info.deployable {
            let mut dur = info.hitlag_halt;
            if self.defhalt && info.def_halt {
                dur += 3.6;
            }
            if dur > 0.0 {
                self.player_hitlag(info.actor as usize, info.hitlag_factor, dur);
            }
        }
    }

    fn icd_state(&mut self, ci: u8, tag: IcdTag, grp: IcdGroup) -> usize {
        let (t, g) = (tag.0, grp.0);
        if let Some(i) = self.enemy.icd.iter().position(|s| s.char == ci && s.tag == t && s.group == g) {
            return i;
        }
        self.enemy.icd.push(IcdState { char: ci, tag: t, group: g, ..Default::default() });
        self.enemy.icd.len() - 1
    }

    fn group_dmg_mult(&mut self, info: &AttackInfo) -> f64 {
        let i = self.icd_state(info.actor, info.icd_tag, info.icd_group);
        if !self.enemy.icd[i].dmg_on {
            self.enemy.icd[i].dmg_on = true;
            self.add_task(Task::IcdResetDmg { i: i as u16 }, info.icd_group.reset_timer() - 1);
        }
        let s = &mut self.enemy.icd[i];
        let val = s.dmg_count;
        s.dmg_count += 1;
        let seq = info.icd_group.dmg_seq();
        if val < seq.len() { seq[val] } else { seq[seq.len() - 1] }
    }

    fn will_apply_ele(&mut self, info: &AttackInfo) -> f64 {
        self.will_apply_ele_of(info, info.actor)
    }

    /// WillApplyEle of an attack's ICD tag and group, counted for character
    /// `ci`.
    fn will_apply_ele_of(&mut self, info: &AttackInfo, ci: u8) -> f64 {
        if info.icd_tag == icd::TAG_NONE {
            return 1.0;
        }
        let i = self.icd_state(ci, info.icd_tag, info.icd_group);
        if !self.enemy.icd[i].tag_on {
            self.enemy.icd[i].tag_on = true;
            self.add_task(Task::IcdResetTag { i: i as u16 }, info.icd_group.reset_timer() - 1);
        }
        let s = &mut self.enemy.icd[i];
        let val = s.tag_count;
        s.tag_count += 1;
        let seq = info.icd_group.ele_seq();
        if val < seq.len() { seq[val] } else { seq[seq.len() - 1] }
    }

    fn apply_attack_mods(&mut self, ci: usize, ev: &mut AttackEvent) {
        let f = self.f;
        for k in 0..self.chars[ci].mods.len() {
            let m = self.chars[ci].mods[k];
            if m.kind != ModKind::Attack || !m.applies(f) {
                continue;
            }
            match m.amount {
                Amount::Owner(Owner::Char(o)) => crate::chars::dispatch::attack_mod(self.chars[o as usize].kind, self, o as usize, ci, &m, ev),
                Amount::Owner(Owner::Weapon(o)) => crate::weapons::dispatch::attack_mod(self.chars[o as usize].weapon, self, o as usize, ci, &m, ev),
                Amount::Owner(Owner::Set(o, si)) => {
                    let kind = self.chars[o as usize].sets[si as usize].kind;
                    crate::sets::dispatch::attack_mod(kind, self, o as usize, si as usize, ci, &m, ev)
                }
                Amount::Fixed => ev.stats[m.affected] += m.value,
                // cryo resonance: CRIT Rate against an enemy with cryo or
                // frozen
                Amount::Owner(Owner::Core) => {
                    let a = &self.enemy.aura;
                    if m.key == CRYO_RESONANCE && (a.get(AURA_CRYO) > ZERO_DUR || a.get(AURA_FROZEN) > ZERO_DUR) {
                        ev.stats[st::CR] += 0.15;
                    }
                    if m.key == GEO_RESONANCE && self.flag(GEO_RESONANCE) == 1.0 && self.player.active as usize == ci {
                        ev.stats[st::DMGP] += 0.15;
                    }
                }
            }
        }
    }

    fn handle_attack(&mut self, slot: u16) {
        let mut ev = self.slots[slot as usize];
        let grp = if ev.info.sim { 1.0 } else { self.group_dmg_mult(&ev.info) };
        let actor = ev.info.actor as usize;
        // OnEnemyHit, unless the group takes no damage from this hit:
        // subscribers (sets and weapons first), then the attacker's own
        if grp > 0.0 {
            if !self.subs[EventKind::EnemyHit as usize].is_empty() {
                self.hit_flat = 0.0;
                self.emit(Event::EnemyHit(&ev));
                ev.info.flat_dmg += self.hit_flat;
            }
            crate::chars::dispatch::own_hit(self.chars[actor].kind, self, actor, &mut ev);
        }
        // gcsim's ApplyAttackMods skips reaction damage
        if !ev.info.sim && !ev.info.tag.is_reaction() {
            self.apply_attack_mods(actor, &mut ev);
        }
        // poise, then shatter, on a frozen enemy
        if self.enemy.aura.get(AURA_FROZEN) >= ZERO_DUR {
            self.poise_check(&ev);
            self.shatter_check(&ev);
        }

        if ev.info.durability > 0.0 {
            ev.info.durability *= self.will_apply_ele(&ev.info);
            // burning's ICD is global: every other character's counter is
            // checked (and counts) too, and any of them on ICD wipes the gauge
            if ev.info.icd_tag == icd::TAG_BURNING_DAMAGE {
                for i in 0..self.chars.len() as u8 {
                    if i != ev.info.actor {
                        ev.info.durability *= self.will_apply_ele_of(&ev.info, i);
                    }
                }
            }
            if ev.info.durability > 0.0 && ev.info.element != Element::Physical {
                self.react(&mut ev);
            }
        }
        let (dmg, crit) = self.calc(&mut ev, grp);
        // enemy hitlag
        let mut dur = ev.info.hitlag_halt;
        if self.defhalt && ev.info.def_halt {
            dur += 3.6;
        }
        let dur = dur.ceil();
        if self.hitlag && dur > 0.0 {
            self.enemy_hitlag(ev.info.hitlag_factor, dur);
        }
        ev.damage = dmg;
        ev.crit = crit;
        self.slots[slot as usize] = ev;
        self.add_task(Task::DamageApplied { slot }, 0);
    }

    /// gcsim's Reactable.React, for the elements the engine applies:
    /// each reaction in gcsim's order, its event emitted as it happens.
    fn react(&mut self, ev: &mut AttackEvent) {
        match ev.info.element {
            Element::Electro => {
                self.try_catalyze(ev, Reaction::Aggravate);
                self.try_overload(ev);
                self.try_add_ec(ev);
                self.try_frozen_superconduct(ev);
                self.try_superconduct(ev);
                self.try_quicken(ev);
            }
            Element::Pyro => {
                self.try_overload(ev);
                self.try_vaporize(ev);
                self.try_melt(ev);
                self.try_burning(ev);
            }
            Element::Cryo => {
                self.try_superconduct(ev);
                self.try_melt(ev);
                self.try_freeze(ev);
            }
            Element::Hydro => {
                self.try_vaporize(ev);
                self.try_freeze(ev);
                self.try_bloom(ev);
                self.try_add_ec(ev);
            }
            Element::Anemo => {
                self.try_swirl(ev, AURA_ELECTRO);
                self.try_swirl(ev, AURA_PYRO);
                self.try_swirl(ev, AURA_HYDRO);
                self.try_swirl(ev, AURA_CRYO);
                self.try_swirl(ev, AURA_FROZEN);
            }
            Element::Geo => {
                for (m, r) in [
                    (AURA_ELECTRO, Reaction::CrystallizeElectro),
                    (AURA_HYDRO, Reaction::CrystallizeHydro),
                    (AURA_CRYO, Reaction::CrystallizeCryo),
                    (AURA_PYRO, Reaction::CrystallizePyro),
                    (AURA_FROZEN, Reaction::CrystallizeCryo),
                ] {
                    // pyro crystallizes off burning too, then checks it
                    let burning = m == AURA_PYRO && self.enemy.aura.get(AURA_BURNING) > ZERO_DUR;
                    if self.enemy.aura.get(m) > ZERO_DUR || burning {
                        self.try_crystallize(ev, m, r);
                        if m == AURA_PYRO {
                            self.burning_check();
                        }
                    }
                }
            }
            Element::Dendro => {
                self.try_catalyze(ev, Reaction::Spread);
                self.try_quicken(ev);
                self.try_burning(ev);
                self.try_bloom(ev);
            }
            _ => {}
        }
    }

    fn try_vaporize(&mut self, ev: &mut AttackEvent) {
        let dur = ev.info.durability;
        if dur < ZERO_DUR {
            return;
        }
        let a = &mut self.enemy.aura;
        let consumed = match ev.info.element {
            Element::Pyro => {
                // frozen rejects vaporize
                if a.get(AURA_HYDRO) < ZERO_DUR || a.get(AURA_FROZEN) > ZERO_DUR {
                    return;
                }
                ev.info.amp_mult = 1.5;
                a.reduce(AURA_HYDRO, dur, 0.5)
            }
            Element::Hydro => {
                if a.get(AURA_PYRO) < ZERO_DUR && a.get(AURA_BURNING) < ZERO_DUR {
                    return;
                }
                ev.info.amp_mult = 2.0;
                let c = a.reduce(AURA_PYRO, dur, 2.0);
                self.burning_check();
                c
            }
            _ => return,
        };
        ev.info.durability = (dur - consumed).max(0.0);
        ev.reacted = true;
        ev.info.amped = true;
        self.emit(Event::Reaction(Reaction::Vaporize, ev));
    }

    fn try_overload(&mut self, ev: &mut AttackEvent) {
        let dur = ev.info.durability;
        if dur < ZERO_DUR {
            return;
        }
        let a = &mut self.enemy.aura;
        let consumed = match ev.info.element {
            Element::Electro if a.get(AURA_PYRO) >= ZERO_DUR || a.get(AURA_BURNING) >= ZERO_DUR => {
                let c = a.reduce(AURA_PYRO, dur, 1.0);
                self.burning_check();
                c
            }
            Element::Pyro if a.get(AURA_ELECTRO) >= ZERO_DUR => a.reduce(AURA_ELECTRO, dur, 1.0),
            _ => return,
        };
        ev.info.durability = (dur - consumed).max(0.0);
        ev.reacted = true;
        // the event comes before the attack is queued
        self.emit(Event::Reaction(Reaction::Overload, ev));
        if self.enemy.overload_gcd == -1 || self.f >= self.enemy.overload_gcd {
            self.enemy.overload_gcd = self.f + 6;
            let mut ai = AttackInfo::new(ev.info.actor as usize, AttackTag::OverloadDamage, icd::TAG_OVERLOAD_DAMAGE, icd::GROUP_REACTION_B, Element::Pyro, 0.0, 0.0);
            ai.abil = "overload";
            ai.ignore_def = 1.0;
            ai.pattern = Pattern::Circle { x: 0.0, y: self.target_dist, r: 3.0 };
            let mut atk = self.reaction_attack(ai);
            atk.info.flat_dmg *= 2.75;
            self.queue_attack_with_snap(atk, 0, 1);
        }
    }

    fn try_melt(&mut self, ev: &mut AttackEvent) {
        let dur = ev.info.durability;
        if dur < ZERO_DUR {
            return;
        }
        let a = &mut self.enemy.aura;
        let consumed = match ev.info.element {
            Element::Pyro => {
                if a.get(AURA_CRYO) < ZERO_DUR && a.get(AURA_FROZEN) < ZERO_DUR {
                    return;
                }
                let c = a.reduce(AURA_CRYO, dur, 2.0);
                let f = a.reduce(AURA_FROZEN, dur, 2.0);
                ev.info.amp_mult = 2.0;
                c.max(f)
            }
            Element::Cryo => {
                if a.get(AURA_PYRO) < ZERO_DUR && a.get(AURA_BURNING) < ZERO_DUR {
                    return;
                }
                // gcsim keeps the cryo's durability
                a.reduce(AURA_PYRO, dur, 0.5);
                ev.info.amp_mult = 1.5;
                self.burning_check();
                0.0
            }
            _ => return,
        };
        ev.info.durability = (dur - consumed).max(0.0);
        ev.reacted = true;
        ev.info.amped = true;
        self.emit(Event::Reaction(Reaction::Melt, ev));
    }

    fn try_freeze(&mut self, ev: &mut AttackEvent) {
        let dur = ev.info.durability;
        if dur < ZERO_DUR {
            return;
        }
        let a = &mut self.enemy.aura;
        let consumed = match ev.info.element {
            Element::Hydro => {
                // every cryo source contributes to frozen
                let mut srcs = 0u8;
                for (i, &d) in a.dur[AURA_CRYO].iter().enumerate() {
                    if d > ZERO_DUR {
                        srcs |= 1 << i;
                    }
                }
                if srcs == 0 {
                    return;
                }
                let d = a.get(AURA_CRYO).min(dur);
                a.attach_overlap_srcs(AURA_FROZEN, 2.0 * d, ZERO_DUR, srcs);
                a.reduce_mod(AURA_CRYO, d);
                d
            }
            Element::Cryo => {
                if a.get(AURA_HYDRO) < ZERO_DUR {
                    return;
                }
                let d = a.get(AURA_HYDRO).min(dur);
                a.attach_overlap_srcs(AURA_FROZEN, 2.0 * d, ZERO_DUR, 1 << ev.info.actor);
                a.reduce_mod(AURA_HYDRO, d);
                d
            }
            _ => return,
        };
        ev.reacted = true;
        ev.info.durability = (dur - consumed).max(0.0);
        self.emit(Event::Reaction(Reaction::Frozen, ev));
    }

    fn try_superconduct(&mut self, ev: &mut AttackEvent) {
        let dur = ev.info.durability;
        if dur < ZERO_DUR {
            return;
        }
        let a = &mut self.enemy.aura;
        // this is the one without frozen
        if a.get(AURA_FROZEN) >= ZERO_DUR {
            return;
        }
        let consumed = match ev.info.element {
            Element::Electro if a.get(AURA_CRYO) >= ZERO_DUR => a.reduce(AURA_CRYO, dur, 1.0),
            Element::Cryo if a.get(AURA_ELECTRO) >= ZERO_DUR => a.reduce(AURA_ELECTRO, dur, 1.0),
            _ => return,
        };
        ev.info.durability = (dur - consumed).max(0.0);
        ev.reacted = true;
        self.queue_superconduct(ev);
    }

    /// Electro on frozen: cryo first, then frozen with what is left, and
    /// the attack's durability is all spent.
    fn try_frozen_superconduct(&mut self, ev: &mut AttackEvent) {
        if ev.info.durability < ZERO_DUR || ev.info.element != Element::Electro {
            return;
        }
        let a = &mut self.enemy.aura;
        if a.get(AURA_FROZEN) < ZERO_DUR {
            return;
        }
        ev.info.durability -= a.reduce(AURA_CRYO, ev.info.durability, 1.0);
        a.reduce(AURA_FROZEN, ev.info.durability, 1.0);
        ev.info.durability = 0.0;
        ev.reacted = true;
        self.queue_superconduct(ev);
    }

    fn queue_superconduct(&mut self, ev: &AttackEvent) {
        self.emit(Event::Reaction(Reaction::Superconduct, ev));
        if self.enemy.superconduct_gcd != -1 && self.f < self.enemy.superconduct_gcd {
            return;
        }
        self.enemy.superconduct_gcd = self.f + 6;
        let mut ai = AttackInfo::new(ev.info.actor as usize, AttackTag::SuperconductDamage, icd::TAG_SUPERCONDUCT_DAMAGE, icd::GROUP_REACTION_A, Element::Cryo, 0.0, 0.0);
        ai.abil = "superconduct";
        ai.ignore_def = 1.0;
        ai.pattern = Pattern::Circle { x: 0.0, y: self.target_dist, r: 3.0 };
        let mut atk = self.reaction_attack(ai);
        atk.info.flat_dmg *= 1.5;
        self.queue_attack_with_snap(atk, 0, 1);
    }

    /// gcsim's PoiseDMGCheck: a blunt attack wears frozen down by its
    /// poise damage.
    fn poise_check(&mut self, ev: &AttackEvent) {
        if self.enemy.aura.get(AURA_FROZEN) < ZERO_DUR || !ev.info.blunt {
            return;
        }
        self.enemy.aura.reduce_mod(AURA_FROZEN, 0.15 * ev.info.poise);
        self.check_freeze();
    }

    /// gcsim's ShatterCheck: a blunt or geo attack shatters frozen.
    fn shatter_check(&mut self, ev: &AttackEvent) {
        if self.enemy.aura.get(AURA_FROZEN) < ZERO_DUR || (!ev.info.blunt && ev.info.element != Element::Geo) {
            return;
        }
        self.enemy.aura.reduce_mod(AURA_FROZEN, 200.0);
        self.check_freeze();
        self.emit(Event::Reaction(Reaction::Shatter, ev));
        if self.enemy.shatter_gcd == -1 || self.f >= self.enemy.shatter_gcd {
            self.enemy.shatter_gcd = self.f + 12;
            let mut ai = AttackInfo::new(ev.info.actor as usize, AttackTag::Shatter, icd::TAG_SHATTER, icd::GROUP_REACTION_A, Element::Physical, 0.0, 0.0);
            ai.abil = "shatter";
            ai.ignore_def = 1.0;
            let mut atk = self.reaction_attack(ai);
            atk.info.flat_dmg *= 3.0;
            self.queue_attack_with_snap(atk, 0, 0);
        }
    }

    /// gcsim's checkFreeze: when frozen is gone, an empty attack from the
    /// fight itself ("Freeze Broken").
    fn check_freeze(&mut self) {
        if self.enemy.aura.get(AURA_FROZEN) > ZERO_DUR {
            return;
        }
        let mut ai = AttackInfo::new(0, AttackTag::None, icd::TAG_NONE, icd::GROUP_DEFAULT, Element::NoElement, 0.0, 0.0);
        ai.abil = "Freeze Broken";
        ai.sim = true;
        self.queue_attack(ai, 0, -1, 0);
    }

    /// gcsim's CalcReactionDmg: the attack with its flat damage and the
    /// snapshot it takes (the level and EM only).
    fn reaction_attack(&self, mut ai: AttackInfo) -> AttackEvent {
        let ci = ai.actor as usize;
        let em = self.stat(ci, st::EM);
        let lvl = self.chars[ci].level;
        ai.flat_dmg = (1.0 + (16.0 * em) / (2000.0 + em) + self.react_bonus(ci, &ai)) * crate::gcsimdata::combat::REACTION_LVL_BASE[(lvl - 1).clamp(0, 99) as usize];
        let mut atk = AttackEvent::new(ai, 0);
        atk.stats[st::EM] = em;
        atk.char_lvl = lvl;
        atk
    }

    /// gcsim's burningCheck: burning gone, its fuel goes too.
    fn burning_check(&mut self) {
        let a = &mut self.enemy.aura;
        if a.get(AURA_BURNING) < ZERO_DUR && a.get(AURA_BURNING_FUEL) > ZERO_DUR {
            a.burning_src = -1;
            a.remove(AURA_BURNING_FUEL);
        }
    }

    /// gcsim's TryBurning: pyro on dendro or quicken, or dendro on pyro or
    /// burning. It neither consumes the attack's element nor counts as a
    /// reaction, so the element attaches after.
    fn try_burning(&mut self, ev: &mut AttackEvent) {
        let dur = ev.info.durability;
        let need = if ev.info.element == Element::Pyro { 1 << AURA_DENDRO | 1 << AURA_QUICKEN } else { 1 << AURA_PYRO | 1 << AURA_BURNING };
        if dur < ZERO_DUR || self.enemy.aura.live & need == 0 {
            return;
        }
        let a = &mut self.enemy.aura;
        let mut dendro = a.get(AURA_DENDRO);
        match ev.info.element {
            Element::Pyro => {
                if a.get(AURA_DENDRO) < ZERO_DUR && a.get(AURA_QUICKEN) < ZERO_DUR {
                    return;
                }
            }
            Element::Dendro => {
                if a.get(AURA_PYRO) < ZERO_DUR && a.get(AURA_BURNING) < ZERO_DUR {
                    return;
                }
                dendro = dendro.max(0.8 * dur);
            }
            _ => return,
        }
        let src = ev.info.actor as usize;
        if a.get(AURA_BURNING_FUEL) < ZERO_DUR {
            let fuel = dendro.max(a.get(AURA_QUICKEN));
            a.attach_burning_fuel(fuel, 1.0, src);
            a.attach_burning(src);
            self.emit(Event::Reaction(Reaction::Burning, ev));
            self.calc_burning(ev);
            if self.enemy.aura.burning_src == -1 {
                let f = self.f;
                self.enemy.aura.burning_src = f;
                self.queue_enemy_task(Task::BurningTick { src: f, counter: 1 }, 15);
            }
            return;
        }
        // fed: the fuel is overwritten and the damage recalculated
        if ev.info.element == Element::Dendro {
            a.attach_burning_fuel(dur, 0.8, src);
        }
        self.calc_burning(ev);
    }

    /// gcsim's calcBurningDmg: the attack of burning's ticks, from the
    /// attacker that started or fed it.
    fn calc_burning(&mut self, ev: &AttackEvent) {
        let mut ai = AttackInfo::new(ev.info.actor as usize, AttackTag::BurningDamage, icd::TAG_BURNING_DAMAGE, icd::GROUP_BURNING, Element::Pyro, 25.0, 0.0);
        ai.abil = "burning";
        ai.ignore_def = 1.0;
        ai.pattern = Pattern::Circle { x: 0.0, y: self.target_dist, r: 1.0 };
        let mut atk = self.reaction_attack(ai);
        atk.info.flat_dmg *= 0.25;
        self.enemy.burning_atk = atk;
    }

    /// gcsim's nextBurningTick: every 0.25 s while burning and its fuel
    /// last, skipping the ninth. Its self damage, a circle of radius 1 on
    /// the enemy, is checked against the player.
    fn burning_tick(&mut self, src: i32, counter: i32) {
        let a = &self.enemy.aura;
        if a.burning_src != src || a.get(AURA_BURNING_FUEL) < ZERO_DUR || a.get(AURA_BURNING) < ZERO_DUR {
            return;
        }
        if counter != 9 {
            let mut atk = self.enemy.burning_atk;
            self.queue_attack_with_snap(atk, 0, 0);
            if self.player_in_circle(self.target_dist, 1.0) {
                atk.info.abil = "burning (self damage)";
                self.player_hit(atk);
            }
        }
        self.queue_enemy_task(Task::BurningTick { src, counter: counter + 1 }, 15);
    }

    /// gcsim's NewCircleHitOnTarget on the player, with its offset.
    pub fn circle_on_player(&self, ox: f64, oy: f64, r: f64) -> Pattern {
        Pattern::Circle { x: ox, y: oy, r }
    }
    /// gcsim's NewCircleHitOnTargetFanAngle on the player.
    pub fn fan_on_player(&self, ox: f64, oy: f64, r: f64, fan: f64) -> Pattern {
        Pattern::Fan { x: ox, y: oy, r, fan }
    }
    /// gcsim's NewBoxHitOnTarget on the player.
    pub fn box_on_player(&self, ox: f64, oy: f64, w: f64, h: f64) -> Pattern {
        Pattern::Box { x: ox, y: oy, w, h }
    }
    /// gcsim's NewCircleHitOnTarget on the enemy, without offset.
    pub fn circle_on_target(&self, r: f64) -> Pattern {
        Pattern::Circle { x: 0.0, y: self.target_dist, r }
    }
    /// gcsim's NewBoxHit from the player centered on the enemy, with its
    /// offset along the way to it.
    pub fn box_on_target(&self, ox: f64, oy: f64, w: f64, h: f64) -> Pattern {
        Pattern::Box { x: ox, y: self.target_dist + oy, w, h }
    }

    /// Whether the player (at the origin, radius 0.3: gcsim's default) is
    /// within a circle of radius r centered y ahead of it, toward the enemy.
    pub fn player_in_circle(&self, y: f64, r: f64) -> bool {
        y.abs() <= r + 0.3
    }

    /// gcsim's TryAggravate and TrySpread: on quicken, flat damage from the
    /// attacker's EM now; the attack goes on to react and attach.
    fn try_catalyze(&mut self, ev: &mut AttackEvent, r: Reaction) {
        if ev.info.durability < ZERO_DUR || self.enemy.aura.live & 1 << AURA_QUICKEN == 0 || self.enemy.aura.get(AURA_QUICKEN) < ZERO_DUR {
            return;
        }
        self.emit(Event::Reaction(r, ev));
        let ci = ev.info.actor as usize;
        let em = self.stat(ci, st::EM);
        ev.info.catalyzed = Some(r);
        let lvl = self.chars[ci].level;
        let base = (1.0 + (5.0 * em) / (1200.0 + em) + self.react_bonus(ci, &ev.info)) * crate::gcsimdata::combat::REACTION_LVL_BASE[(lvl - 1).clamp(0, 99) as usize];
        ev.info.flat_dmg += if r == Reaction::Aggravate { 1.15 } else { 1.25 } * base;
    }

    /// gcsim's TryQuicken: dendro on electro or electro on dendro.
    fn try_quicken(&mut self, ev: &mut AttackEvent) {
        let dur = ev.info.durability;
        let need = if ev.info.element == Element::Dendro { 1 << AURA_ELECTRO } else { 1 << AURA_DENDRO };
        if dur < ZERO_DUR || self.enemy.aura.live & need == 0 {
            return;
        }
        let a = &mut self.enemy.aura;
        let consumed = match ev.info.element {
            Element::Dendro if a.get(AURA_ELECTRO) >= ZERO_DUR => a.reduce(AURA_ELECTRO, dur, 1.0),
            // electro on dendro takes burning fuel too
            Element::Electro if a.get(AURA_DENDRO) >= ZERO_DUR => a.reduce(AURA_DENDRO, dur, 1.0),
            _ => return,
        };
        ev.info.durability = (dur - consumed).max(0.0);
        ev.reacted = true;
        self.emit(Event::Reaction(Reaction::Quicken, ev));
        let a = &mut self.enemy.aura;
        a.attach_refresh(AURA_QUICKEN, consumed, 12.0 * consumed + 360.0, ev.info.actor as usize);
        if a.get(AURA_HYDRO) >= ZERO_DUR {
            let slot = self.alloc(*ev);
            self.add_task(Task::QuickenBloom { slot }, 0);
        }
    }

    /// gcsim's tryQuickenBloom: quicken blooms with the hydro left.
    fn quicken_bloom(&mut self, slot: u16) {
        let a = &mut self.enemy.aura;
        if a.get(AURA_QUICKEN) < ZERO_DUR || a.get(AURA_HYDRO) < ZERO_DUR {
            self.free.push(slot);
            return;
        }
        let avail = a.get(AURA_QUICKEN);
        let consumed = a.reduce(AURA_HYDRO, avail, 2.0);
        a.reduce_mod(AURA_QUICKEN, consumed);
        self.check_lunar_bloom();
        let ev = self.slots[slot as usize];
        self.add_task(Task::SpawnCore { slot }, 30);
        self.emit(Event::Reaction(Reaction::Bloom, &ev));
    }

    /// Lunar-Bloom (gcsim's Flags.Custom "lunarbloom-enabled") would replace
    /// bloom; nothing the engine implements enables it.
    fn check_lunar_bloom(&mut self) {
        if self.flag(LUNAR_BLOOM) != 0.0 {
            self.unsupported("lunar-bloom");
        }
    }

    /// gcsim's TryBloom: hydro on dendro, quicken or burning fuel, or
    /// dendro on hydro. The core appears 0.5 s later.
    fn try_bloom(&mut self, ev: &mut AttackEvent) {
        let dur = ev.info.durability;
        let need = if ev.info.element == Element::Hydro { 1 << AURA_DENDRO | 1 << AURA_QUICKEN | 1 << AURA_BURNING_FUEL } else { 1 << AURA_HYDRO };
        if dur < ZERO_DUR || self.enemy.aura.live & need == 0 {
            return;
        }
        let a = &mut self.enemy.aura;
        let consumed = match ev.info.element {
            Element::Hydro => {
                if !(a.get(AURA_DENDRO) > ZERO_DUR || a.get(AURA_QUICKEN) > ZERO_DUR || a.get(AURA_BURNING_FUEL) > ZERO_DUR) {
                    return;
                }
                let c = a.reduce(AURA_DENDRO, dur, 0.5);
                c.max(a.reduce(AURA_QUICKEN, dur, 0.5))
            }
            Element::Dendro => {
                if a.get(AURA_HYDRO) < ZERO_DUR {
                    return;
                }
                a.reduce(AURA_HYDRO, dur, 2.0)
            }
            _ => return,
        };
        ev.info.durability = (dur - consumed).max(0.0);
        ev.reacted = true;
        self.check_lunar_bloom();
        let slot = self.alloc(*ev);
        self.add_task(Task::SpawnCore { slot }, 30);
        self.emit(Event::Reaction(Reaction::Bloom, ev));
    }

    /// dendrocore.New: a core at a random point 0.5 beyond the enemy's
    /// edge, for 5 s. A sixth core makes the oldest explode.
    fn spawn_core(&mut self, slot: u16) {
        let actor = self.slots[slot as usize].info.actor;
        self.free.push(slot);
        // CalcRandomPointFromCenter, between r and r away
        let r = self.target_radius + 0.5;
        let (px, py) = loop {
            let x = -1.0 + self.rng.float64() * 2.0;
            let y = -1.0 + self.rng.float64() * 2.0;
            if x * x + y * y <= 1.0 {
                let rr = (r * r + self.rng.float64() * (r * r - r * r)).sqrt();
                let (x, y) = if x == 0.0 && y == 0.0 { (0.0, 1.0) } else { (x, y) };
                let factor = rr / (x * x + y * y).sqrt();
                break (x * factor, self.target_dist + y * factor);
            }
        };
        let mut oldest: Option<usize> = None;
        let mut count = 0;
        for (i, g) in self.gadgets.iter().enumerate() {
            if g.owner == CORE {
                count += 1;
                if oldest.is_none_or(|o| g.src < self.gadgets[o].src) {
                    oldest = Some(i);
                }
            }
        }
        if count == 5 {
            let g = self.gadgets.remove(oldest.unwrap());
            self.core_explode(g.actor, g.x, g.y);
        }
        let f = self.f;
        self.gadgets.push(Gadget { owner: CORE, id: 0, timer: 0, duration: 300, aura: Aura::new(), src: f, x: px, y: py, r: 2.0, actor, takes_hits: false, enemy_camp: true });
    }

    /// A core expired or was killed: its bloom 1 frame later, a circle of
    /// radius 5 on it.
    fn core_explode(&mut self, actor: u8, x: f64, y: f64) {
        let on_player = x * x + y * y <= (5.0 + 0.3) * (5.0 + 0.3);
        self.add_task(Task::CoreAttack { kind: AttackTag::Bloom, actor, on_player }, 1);
    }

    /// The attack of a core: bloom (2), burgeon (3) or hyperbloom (3) times
    /// the attacker's reaction damage, and 2% of it to the player when its
    /// area reaches the player.
    fn core_attack(&mut self, kind: AttackTag, actor: usize, on_player: bool) {
        let (icd_tag, abil, mult) = match kind {
            AttackTag::Bloom => (icd::TAG_BLOOM_DAMAGE, "bloom", 2.0),
            AttackTag::Burgeon => (icd::TAG_BURGEON_DAMAGE, "burgeon", 3.0),
            _ => (icd::TAG_HYPERBLOOM_DAMAGE, "hyperbloom", 3.0),
        };
        let mut ai = AttackInfo::new(actor, kind, icd_tag, icd::GROUP_REACTION_A, Element::Dendro, 0.0, 0.0);
        ai.abil = abil;
        ai.ignore_def = 1.0;
        ai.pattern = Pattern::Single;
        let mut atk = self.reaction_attack(ai);
        atk.info.flat_dmg *= mult;
        self.queue_attack_with_snap(atk, 0, 0);
        if on_player {
            atk.info.abil = match kind {
                AttackTag::Bloom => "bloom (self damage)",
                AttackTag::Burgeon => "burgeon (self damage)",
                _ => "hyperbloom (self damage)",
            };
            atk.info.flat_dmg *= 0.02;
            self.player_hit(atk);
        }
    }

    /// The gadgets an attack reaches besides the enemy: a dendro core hit
    /// by electro (hyperbloom, 1 s later, on the enemy) or pyro (burgeon,
    /// a circle of radius 5 on the core) is used up. gcsim hands each
    /// target its own copy of the attack, so the core sees it before ICD
    /// or reactions on the enemy.
    fn hit_gadgets(&mut self, ev: &AttackEvent) {
        if self.gadgets.iter().any(|g| g.takes_hits) {
            let mut k = 0;
            while k < self.gadgets.len() {
                let g = self.gadgets[k];
                if g.takes_hits {
                    let lands = ev.info.pattern.hits_circle(g.x, g.y, g.r);
                    crate::chars::dispatch::gadget_hit(self.chars[g.owner as usize].kind, self, g.owner as usize, g.id, ev, lands);
                }
                // the gadget may have been removed
                if k < self.gadgets.len() && self.gadgets[k].owner == g.owner && self.gadgets[k].id == g.id && self.gadgets[k].src == g.src {
                    k += 1;
                }
            }
        }
        // gcsim's Guoba swirls its pyro window when Sucrose's skill (or
        // Faruzan's, not ported) reaches it: not ported either
        if ev.info.tag == AttackTag::ElementalArt && self.chars[ev.info.actor as usize].kind == crate::chars::CharKind::Sucrose {
            let guoba = self.gadgets.iter().any(|g| g.owner != CORE && self.chars[g.owner as usize].kind == crate::chars::CharKind::Xiangling && g.aura.contains(Element::Pyro) && ev.info.pattern.hits_circle(g.x, g.y, g.r) != Some(false));
            if guoba {
                self.unsupported("sucrose skill swirling guoba");
                return;
            }
        }
        if ev.info.durability < ZERO_DUR || !matches!(ev.info.element, Element::Electro | Element::Pyro) {
            return;
        }
        let mut k = 0;
        while k < self.gadgets.len() {
            let g = self.gadgets[k];
            if g.owner != CORE {
                k += 1;
                continue;
            }
            match ev.info.pattern.hits_circle(g.x, g.y, 2.0) {
                None => {
                    let who = crate::gcsl::char_name(self.chars[ev.info.actor as usize].key);
                    self.unsupported(&format!("hit pattern {who} {}", ev.info.abil));
                    return;
                }
                Some(false) => {
                    k += 1;
                    continue;
                }
                Some(true) => {}
            }
            self.gadgets.remove(k);
            let actor = ev.info.actor;
            if ev.info.element == Element::Electro {
                // a circle of radius 1 on the enemy, which the player is not in
                let on_player = self.player_in_circle(self.target_dist, 1.0);
                self.add_task(Task::CoreAttack { kind: AttackTag::Hyperbloom, actor, on_player }, 60);
                self.emit(Event::Reaction(Reaction::Hyperbloom, ev));
            } else {
                let on_player = g.x * g.x + g.y * g.y <= (5.0 + 0.3) * (5.0 + 0.3);
                self.add_task(Task::CoreAttack { kind: AttackTag::Burgeon, actor, on_player }, 1);
                self.emit(Event::Reaction(Reaction::Burgeon, ev));
            }
        }
    }

    /// gcsim's Player.HandleAttack: OnPlayerHit, the damage as the player
    /// takes it, then a drain of the active character. No reactions on
    /// the player, and it does not count as damage.
    fn player_hit(&mut self, mut ev: AttackEvent) {
        let active = self.player.active as usize;
        self.emit(Event::PlayerHit { on: active });
        // the shield stats collector looks at the shields at the end of
        // the frame (its ShieldBonus drops the mods that are done)
        if !self.shield_bonus_mods.is_empty() {
            self.add_task(Task::ShieldTrack, 0);
        }
        let s = &mut ev.stats;
        let st_ele = ev.info.element.dmg_stat();
        let ele_per = if st_ele != st::NONE { s[st_ele] } else { 0.0 };
        let base = ev.info.mult * total_atk(s) + ev.info.flat_dmg;
        let mut damage = base * (1.0 + ele_per + s[st::DMGP]);
        s[st::CR] = s[st::CR].clamp(0.0, 1.0);
        let def = total_def(&self.stats(active)) * (1.0 - ev.info.ignore_def.min(1.0));
        damage *= 1.0 - def / (def + (5 * ev.char_lvl) as f64 + 500.0);
        let (cr, cd, em) = (ev.stats[st::CR], ev.stats[st::CD], ev.stats[st::EM]);
        if self.rng.float64() <= cr {
            damage *= 1.0 + cd;
        }
        if ev.info.amped {
            let react_bonus = self.react_bonus(ev.info.actor as usize, &ev.info);
            damage *= ev.info.amp_mult * (1.0 + (2.78 * em) / (1400.0 + em) + react_bonus);
        }
        if !ev.info.sim {
            damage *= self.player_group_dmg_mult(&ev.info);
        }
        let left = self.shields_on_damage(active, active, damage, ev.info.element);
        if left > 0.0 {
            self.modify_hp(active, -left);
            self.emit(Event::Drain { on: active, amount: left, external: true });
        }
    }

    /// gcsim's Shields.Add: replaces the shield of its type and target.
    pub fn add_shield(&mut self, sh: Shield) {
        match self.shields.iter().rposition(|x| x.typ == sh.typ && x.target == sh.target) {
            Some(i) => self.shields[i] = sh,
            None => self.shields.push(sh),
        }
        self.update_next_expiry();
        self.emit(Event::Shielded(&sh));
        // the shield stats collector, last to subscribe
        if !self.shield_bonus_mods.is_empty() {
            self.shield_bonus();
        }
    }

    fn update_next_expiry(&mut self) {
        let shields = self.shields.iter().map(|s| s.expiry).filter(|&e| e >= 0).min().unwrap_or(i32::MAX);
        let shards = self.shards.first().map_or(i32::MAX, |sh| sh.expiry);
        self.next_expiry = shields.min(shards);
    }

    /// gcsim's CharacterIsShielded.
    pub fn is_shielded(&self, ci: usize) -> bool {
        let active = self.player.active as usize;
        self.shields.iter().any(|s| (s.target == -1 && ci == active) || s.target as isize == ci as isize)
    }

    /// gcsim's ShieldBonus: the sum of the mods in effect; a mod whose
    /// amount says it is done is dropped, as are expired ones.
    pub fn shield_bonus(&mut self) -> f64 {
        let f = self.f;
        let mut amt = 0.0;
        let mut k = 0;
        while k < self.shield_bonus_mods.len() {
            let m = self.shield_bonus_mods[k];
            let keep = if m.expiry > f || m.expiry == -1 {
                let (a, done) = match m.owner {
                    Owner::Char(o) => crate::chars::dispatch::shield_bonus(self.chars[o as usize].kind, self, o as usize, m.key),
                    _ => (0.15, true), // geo resonance
                };
                amt += a;
                !done
            } else {
                false
            };
            if keep {
                k += 1;
            } else {
                self.shield_bonus_mods.remove(k);
            }
        }
        amt
    }

    /// gcsim's Shields.OnDamage: every shield on the character hit takes
    /// the damage; the least left over goes through.
    fn shields_on_damage(&mut self, ci: usize, active: usize, dmg: f64, ele: Element) -> f64 {
        if self.shields.is_empty() {
            return dmg;
        }
        let bonus = self.shield_bonus();
        let mut taken_min = dmg;
        let mut broken: Vec<Shield> = Vec::new();
        let mut k = 0;
        while k < self.shields.len() {
            let t = self.shields[k].target;
            if (t == -1 && ci != active) || (t != -1 && t as usize != ci) {
                k += 1;
                continue;
            }
            let (taken, ok) = self.shields[k].on_damage(dmg, ele, bonus);
            taken_min = taken_min.min(taken);
            let sh = self.shields[k];
            if ok {
                k += 1;
            } else {
                self.shields.remove(k);
                broken.push(sh);
            }
            if sh.typ != ShieldType::Crystallize && sh.typ != ShieldType::CandaceSkill {
                crate::chars::dispatch::shield_damaged(self.chars[sh.owner as usize].kind, self, sh.owner as usize, sh.key, !ok);
            }
        }
        if !broken.is_empty() {
            self.update_next_expiry();
        }
        for sh in &broken {
            self.emit(Event::ShieldBreak(sh));
        }
        taken_min
    }

    /// Shields.Tick: shields expire on their frame.
    fn tick_shields(&mut self) {
        let f = self.f;
        let mut k = 0;
        let mut expired: Vec<Shield> = Vec::new();
        while k < self.shields.len() {
            if self.shields[k].expiry == f {
                let sh = self.shields.remove(k);
                if matches!(sh.typ, ShieldType::ZhongliJadeShield | ShieldType::BaizhuBurst) {
                    crate::chars::dispatch::shield_expired(self.chars[sh.owner as usize].kind, self, sh.owner as usize, sh.key);
                }
                expired.push(sh);
            } else {
                k += 1;
            }
        }
        self.update_next_expiry();
        for sh in &expired {
            self.emit(Event::ShieldBreak(sh));
        }
    }

    /// addCrystallizeShard's task: a shard with a crystallize shield from
    /// the character's level and EM now, at a random point beyond the
    /// enemy (which nothing in the engine reads, so it is not drawn). A
    /// fourth shard removes the oldest.
    fn spawn_shard(&mut self, ci: usize, ele: Element) {
        let em = self.stats(ci)[st::EM];
        let lvl = (self.chars[ci].level - 1).clamp(0, 99) as usize;
        let f = self.f;
        let shield = Shield {
            owner: ci as u8,
            key: ModKey::new("crystallize"),
            typ: ShieldType::Crystallize,
            target: -1,
            ele: if ele == Element::Frozen { Element::Cryo } else { ele },
            hp: crate::gcsimdata::crystallize::SHIELD_BASE_HP[lvl],
            expiry: -1,
            em_bonus: (40.0 / 9.0) * (em / (1400.0 + em)),
        };
        if self.shards.len() == 3 {
            let oldest = (0..3).min_by_key(|&i| self.shards[i].src).unwrap();
            self.shards.remove(oldest);
        }
        self.shards.push(Shard { src: f, expiry: f + 15 * 60, earliest_pickup: f + 31, shield });
        self.update_next_expiry();
    }

    /// pick_up_crystallize: the first shard of the element (any with
    /// None) that can be picked up gives its shield for 15.1 s.
    pub fn pick_up_crystallize(&mut self, ele: Option<Element>) -> bool {
        let f = self.f;
        for k in 0..self.shards.len() {
            let sh = &self.shards[k];
            if ele.is_some_and(|e| e != sh.shield.ele) || f < sh.earliest_pickup {
                continue;
            }
            let mut shield = sh.shield;
            shield.expiry = f + (15.1 * 60.0) as i32;
            self.shards.remove(k);
            // add_shield updates next_expiry
            self.add_shield(shield);
            return true;
        }
        false
    }

    /// updateGeoResonance: DMG +15% and geo shred for the active character
    /// while shielded, checked on swaps, shields and every second.
    fn geo_res_update(&mut self, src: i32) {
        if self.geo_res_src != src {
            return;
        }
        let active = self.player.active as usize;
        let on = self.is_shielded(active);
        self.set_flag(GEO_RESONANCE, if on { 1.0 } else { 0.0 });
        self.queue_char_task(active, Task::GeoRes { src }, 60);
    }

    fn player_group_dmg_mult(&mut self, info: &AttackInfo) -> f64 {
        let (t, g) = (info.icd_tag.0, info.icd_group.0);
        let i = match self.player_icd.iter().position(|s| s.char == info.actor && s.tag == t && s.group == g) {
            Some(i) => i,
            None => {
                self.player_icd.push(IcdState { char: info.actor, tag: t, group: g, ..Default::default() });
                self.player_icd.len() - 1
            }
        };
        if !self.player_icd[i].dmg_on {
            self.player_icd[i].dmg_on = true;
            self.add_task(Task::PlayerIcdResetDmg { i: i as u16 }, info.icd_group.reset_timer() - 1);
        }
        let s = &mut self.player_icd[i];
        let val = s.dmg_count;
        s.dmg_count += 1;
        let seq = info.icd_group.dmg_seq();
        if val < seq.len() { seq[val] } else { seq[seq.len() - 1] }
    }

    /// Something the fight found it does not implement: the run stops and
    /// returns it as an error.
    pub fn unsupported(&mut self, what: &str) {
        if self.error.is_none() {
            self.error = Some(format!("unsupported: {what}"));
        }
        self.done = true;
    }

    fn try_add_ec(&mut self, ev: &mut AttackEvent) {
        let dur = ev.info.durability;
        if dur < ZERO_DUR {
            return;
        }
        // frozen rejects electro-charged
        if self.enemy.aura.get(AURA_FROZEN) > ZERO_DUR {
            return;
        }
        let src = ev.info.actor as usize;
        // a hit that already reacted adds no aura
        match ev.info.element {
            Element::Hydro => {
                if self.enemy.aura.get(AURA_ELECTRO) < ZERO_DUR {
                    return;
                }
                if !ev.reacted {
                    self.enemy.aura.attach_overlap(AURA_HYDRO, 0.8 * dur, 6.0 * dur + 420.0, src);
                }
            }
            Element::Electro => {
                if self.enemy.aura.get(AURA_HYDRO) < ZERO_DUR {
                    return;
                }
                if !ev.reacted {
                    self.enemy.aura.attach_overlap(AURA_ELECTRO, 0.8 * dur, 6.0 * dur + 420.0, src);
                }
            }
            _ => return,
        }
        ev.reacted = true;
        self.emit(Event::Reaction(Reaction::ElectroCharged, ev));
        // electro-charged is refreshed: the next ticks are this attacker's
        let mut ai = AttackInfo::new(src, AttackTag::ECDamage, icd::TAG_EC_DAMAGE, icd::GROUP_REACTION_B, Element::Electro, 0.0, 0.0);
        ai.abil = "electrocharged";
        ai.ignore_def = 1.0;
        ai.pattern = Pattern::Single;
        let mut atk = self.reaction_attack(ai);
        atk.info.flat_dmg *= 2.0;
        self.enemy.ec_atk = atk;
        // a new electro-charged ticks at once, then every second
        if self.enemy.ec_tick_src == -1 {
            let f = self.f;
            self.enemy.ec_tick_src = f;
            self.queue_attack_with_snap(atk, 0, 10);
            self.add_task(Task::EcTick { src: f }, 60 + 10);
            self.enemy.ec_wane_sub = true;
        }
    }

    fn ec_tick(&mut self, src: i32) {
        if self.enemy.ec_tick_src != src || self.enemy.aura.get(AURA_ELECTRO) < ZERO_DUR || self.enemy.aura.get(AURA_HYDRO) < ZERO_DUR {
            return;
        }
        let atk = self.enemy.ec_atk;
        self.queue_attack_with_snap(atk, 0, 0);
        self.add_task(Task::EcTick { src }, 60);
    }

    /// The wane hook: electro-charged damage that landed takes 10 off
    /// both auras 0.1 s later.
    fn ec_wane_hook(&mut self, ev: &AttackEvent) {
        if !self.enemy.ec_wane_sub || ev.info.tag != AttackTag::ECDamage || ev.damage == 0.0 {
            return;
        }
        if self.enemy.aura.get(AURA_ELECTRO) < ZERO_DUR || self.enemy.aura.get(AURA_HYDRO) < ZERO_DUR {
            self.enemy.ec_wane_sub = false;
            return;
        }
        self.add_task(Task::EcWane, 6);
    }

    fn ec_wane(&mut self) {
        self.enemy.aura.reduce_mod(AURA_ELECTRO, 10.0);
        self.enemy.aura.reduce_mod(AURA_HYDRO, 10.0);
        self.check_ec();
    }

    fn check_ec(&mut self) {
        if self.enemy.aura.get(AURA_ELECTRO) < ZERO_DUR || self.enemy.aura.get(AURA_HYDRO) < ZERO_DUR {
            self.enemy.ec_tick_src = -1;
            self.enemy.ec_wane_sub = false;
        }
    }

    /// gcsim's Player.ApplySelfInfusion: the element on the player for f
    /// frames, its source the active character.
    pub fn apply_self_infusion(&mut self, m: usize, dur: f64, f: i32) {
        let src = self.player.active as usize;
        let a = &mut self.player_aura;
        let active = a.get(m);
        if active > ZERO_DUR {
            if active < dur {
                a.dur[m][src] = dur;
                a.live |= 1 << m;
            }
            return;
        }
        a.dur[m][src] = dur;
        a.live |= 1 << m;
        a.decay[m] = dur / f as f64;
    }

    /// gcsim's Combat.AbsorbCheck for one enemy always in range: the first
    /// element of prio on the enemy, else on a gadget, else on the player.
    /// Every gadget is taken to be in range.
    pub fn absorb_check(&self, prio: &[Element]) -> Element {
        for &e in prio {
            if self.enemy.aura.contains(e) || self.gadgets.iter().any(|g| g.aura.contains(e)) || self.player_aura.contains(e) {
                return e;
            }
        }
        Element::NoElement
    }

    /// absorb_check where the area is p, which reaches only some of the
    /// enemy, the gadgets and the player (gcsim's AttackWillLand).
    pub fn absorb_check_in(&self, p: Pattern, prio: &[Element]) -> Element {
        let hit = |x: f64, y: f64, r: f64| p.hits_circle(x, y, r).expect("absorb check with an unknown pattern");
        let enemy = hit(0.0, self.target_dist, self.target_radius);
        let player = hit(0.0, 0.0, 0.3);
        for &e in prio {
            if (enemy && self.enemy.aura.contains(e)) || self.gadgets.iter().any(|g| g.aura.contains(e) && hit(g.x, g.y, g.r)) || (player && self.player_aura.contains(e)) {
                return e;
            }
        }
        Element::NoElement
    }

    /// gcsim's Constructs.New: with refresh, the last construct of the same
    /// owner and key is destroyed; past three, the oldest are. A destroyed
    /// construct calls its owner's construct_expired (OnDestruct).
    pub fn new_construct(&mut self, c: Construct, refresh: bool) {
        if refresh {
            if let Some(i) = self.constructs.iter().rposition(|x| x.owner == c.owner && x.key == c.key) {
                let old = self.constructs.remove(i);
                crate::chars::dispatch::construct_expired(self.chars[old.owner as usize].kind, self, old.owner as usize, old.key);
            }
        }
        self.constructs.push(c);
        while self.constructs.len() > 3 {
            let old = self.constructs.remove(0);
            crate::chars::dispatch::construct_expired(self.chars[old.owner as usize].kind, self, old.owner as usize, old.key);
        }
        self.emit(Event::ConstructSpawned);
    }

    /// gcsim's Player.Exec of a swap from inside the fight (a character
    /// forcing it): it bypasses the swap cooldown and replaces the action
    /// under way. A swap to the active character does nothing.
    pub fn force_swap(&mut self, to: usize) {
        if to == self.player.active as usize {
            return;
        }
        self.exec(to, Action::Swap, &Params::default());
    }

    /// gcsim's AuraContains for one element on the enemy: every aura slot
    /// of that element, and frozen for cryo.
    pub fn enemy_aura_contains(&self, e: Element) -> bool {
        self.enemy.aura.contains(e)
    }

    /// Whether a box hit lands on the enemy: a box `w` wide and `h` long,
    /// centered `y` ahead of the player, who stands at the origin facing
    /// the enemy (gcsim's direction from setup; the player never moves).
    pub fn box_hits(&self, y: f64, w: f64, h: f64) -> bool {
        let r = self.target_radius;
        let dy = (self.target_dist - y).abs() - h / 2.0;
        let dx = -w / 2.0;
        let (dx, dy) = (dx.max(0.0), dy.max(0.0));
        dx * dx + dy * dy <= r * r
    }

    /// gcsim's Combat.AddGadget.
    /// The gadget is at (x, y) from the player, of radius r.
    pub fn add_gadget(&mut self, owner: usize, id: u8, duration: i32, x: f64, y: f64, r: f64) {
        self.gadgets.push(Gadget { owner: owner as u8, id, timer: 0, duration, aura: Aura::new(), src: self.f, x, y, r, actor: 0, takes_hits: false, enemy_camp: false });
    }

    /// add_gadget for a gadget whose owner handles the attacks reaching it
    /// (its gadget_hit), and whether it is of the enemy camp.
    pub fn add_hittable_gadget(&mut self, owner: usize, id: u8, duration: i32, x: f64, y: f64, r: f64, enemy_camp: bool) {
        self.add_gadget(owner, id, duration, x, y, r);
        let g = self.gadgets.last_mut().unwrap();
        g.takes_hits = true;
        g.enemy_camp = enemy_camp;
    }

    /// Removes a character's gadget (gcsim's Gadget.Kill), when it is there.
    pub fn kill_gadget(&mut self, owner: usize, id: u8) {
        if let Some(k) = self.gadget_index(owner, id) {
            self.gadgets.remove(k);
        }
    }

    /// The index of a character's gadget, if it is there.
    pub fn gadget_index(&self, owner: usize, id: u8) -> Option<usize> {
        self.gadgets.iter().position(|g| g.owner as usize == owner && g.id == id)
    }

    /// Sets a gadget's aura durability in one slot (SetAuraDurability, no
    /// decay), when the gadget is still there.
    pub fn set_gadget_aura(&mut self, owner: usize, id: u8, m: usize, dur: f64) {
        if let Some(k) = self.gadget_index(owner, id) {
            let a = &mut self.gadgets[k].aura;
            a.dur[m][0] = dur;
            a.live |= 1 << m;
        }
    }

    /// Each gadget's Tick: its aura, then Gadget.Tick (expiry), then its
    /// owner's hook with the ticks so far.
    fn tick_gadgets(&mut self) {
        let n = self.gadgets.len();
        let mut k = 0;
        for _ in 0..n {
            let g = &mut self.gadgets[k];
            if !g.aura.idle() {
                g.aura.tick();
            }
            let mut expired = false;
            if g.duration > 0 {
                g.duration -= 1;
                expired = g.duration == 0;
            }
            g.timer += 1;
            let (owner, id, timer) = (g.owner as usize, g.id, g.timer);
            let core = g.owner == CORE;
            let (actor, gx, gy) = (g.actor, g.x, g.y);
            if expired {
                self.gadgets.remove(k);
            } else {
                k += 1;
            }
            if core {
                if expired {
                    self.core_explode(actor, gx, gy);
                }
            } else {
                crate::chars::dispatch::gadget_tick(self.chars[owner].kind, self, owner, id, timer);
            }
        }
    }

    /// gcsim's TrySwirlElectro, Pyro, Hydro, Cryo and Frozen. Swirl
    /// hits the enemy itself without gauge, then its aoe the others: with
    /// one enemy, the aoe hits nothing.
    fn try_swirl(&mut self, ev: &mut AttackEvent, m: usize) {
        let dur = ev.info.durability;
        if dur < ZERO_DUR || self.enemy.aura.get(m) < ZERO_DUR {
            return;
        }
        let rd = self.enemy.aura.reduce(m, dur, 0.5);
        // calcSwirlAtkDurability: the aoe's gauge
        let aoe_dur = if rd < dur { 1.25 * (0.5 * rd - 1.0) + 25.0 } else { 1.25 * (dur - 1.0) + 25.0 };
        ev.info.durability -= rd;
        ev.reacted = true;
        if m == AURA_PYRO {
            self.burning_check();
        }
        let (r, tag, icd_tag, ele, abil) = match m {
            AURA_ELECTRO => (Reaction::SwirlElectro, AttackTag::SwirlElectro, icd::TAG_SWIRL_ELECTRO, Element::Electro, "swirl-electro"),
            AURA_PYRO => (Reaction::SwirlPyro, AttackTag::SwirlPyro, icd::TAG_SWIRL_PYRO, Element::Pyro, "swirl-pyro"),
            AURA_HYDRO => (Reaction::SwirlHydro, AttackTag::SwirlHydro, icd::TAG_SWIRL_HYDRO, Element::Hydro, "swirl-hydro"),
            _ => (Reaction::SwirlCryo, AttackTag::SwirlCryo, icd::TAG_SWIRL_CRYO, Element::Cryo, "swirl-cryo"),
        };
        self.emit(Event::Reaction(r, ev));
        // frozen shares cryo's GCD
        let gcd = &mut self.enemy.swirl_gcd[if m == AURA_FROZEN { AURA_CRYO } else { m }];
        if *gcd == -1 || self.f >= *gcd {
            *gcd = self.f + 6;
            let mut ai = AttackInfo::new(ev.info.actor as usize, tag, icd_tag, icd::GROUP_REACTION_A, ele, 0.0, 0.0);
            ai.abil = abil;
            ai.ignore_def = 1.0;
            ai.pattern = Pattern::Single;
            let mut atk = self.reaction_attack(ai);
            atk.info.flat_dmg *= 0.6;
            self.queue_attack_with_snap(atk, 0, 1);
            // the aoe, 4 frames later, ignores the enemy: it reaches only
            // gadgets, where pyro and electro use up dendro cores (and
            // every element reaches Lynette's Bogglecat Box)
            if matches!(ele, Element::Pyro | Element::Electro) || self.gadgets.iter().any(|g| g.takes_hits) {
                atk.info.durability = aoe_dur;
                atk.info.pattern = Pattern::Circle { x: 0.0, y: self.target_dist, r: 5.0 };
                let slot = self.alloc(atk);
                self.add_task(Task::GadgetAttack { slot }, 1 + 4);
            }
        }
        // swirling electro off electro-charged swirls the hydro too
        if m == AURA_ELECTRO && ev.info.durability > ZERO_DUR && self.enemy.aura.get(AURA_HYDRO) > ZERO_DUR {
            self.try_swirl(ev, AURA_HYDRO);
            self.check_ec();
        }
        if m == AURA_FROZEN {
            self.check_freeze();
        }
    }

    /// gcsim's Player.ReactWithSelf: an attack on the player's own aura
    /// (Jean's field swirls it). Only anemo reaches it here: the swirls of
    /// the player's aura, whose aoe around the player hits the enemy and
    /// gadgets; the swirl's own hit, a single target on the player, skips
    /// the player and lands nowhere.
    pub fn react_with_self(&mut self, info: AttackInfo) {
        // AuraCount() == 0
        if (0..N_AURAS).all(|m| self.player_aura.get(m) <= ZERO_DUR) {
            return;
        }
        if info.element != Element::Anemo {
            self.unsupported("a reaction on the player other than swirl");
            return;
        }
        let mut ev = AttackEvent::new(info, 0);
        ev.on_player = true;
        for m in [AURA_ELECTRO, AURA_PYRO, AURA_HYDRO, AURA_CRYO, AURA_FROZEN] {
            self.player_swirl(&mut ev, m);
        }
    }

    /// TrySwirl* on the player's aura.
    fn player_swirl(&mut self, ev: &mut AttackEvent, m: usize) {
        let dur = ev.info.durability;
        if dur < ZERO_DUR || self.player_aura.get(m) < ZERO_DUR {
            return;
        }
        if m == AURA_FROZEN {
            self.unsupported("swirl of frozen on the player");
            return;
        }
        let rd = self.player_aura.reduce(m, dur, 0.5);
        let aoe_dur = if rd < dur { 1.25 * (0.5 * rd - 1.0) + 25.0 } else { 1.25 * (dur - 1.0) + 25.0 };
        ev.info.durability -= rd;
        ev.reacted = true;
        let (r, tag, icd_tag, ele, abil) = match m {
            AURA_ELECTRO => (Reaction::SwirlElectro, AttackTag::SwirlElectro, icd::TAG_SWIRL_ELECTRO, Element::Electro, "swirl-electro (aoe)"),
            AURA_PYRO => (Reaction::SwirlPyro, AttackTag::SwirlPyro, icd::TAG_SWIRL_PYRO, Element::Pyro, "swirl-pyro (aoe)"),
            AURA_HYDRO => (Reaction::SwirlHydro, AttackTag::SwirlHydro, icd::TAG_SWIRL_HYDRO, Element::Hydro, "swirl-hydro (aoe)"),
            _ => (Reaction::SwirlCryo, AttackTag::SwirlCryo, icd::TAG_SWIRL_CRYO, Element::Cryo, "swirl-cryo (aoe)"),
        };
        self.emit(Event::Reaction(r, ev));
        let gcd = &mut self.player_swirl_gcd[m];
        if *gcd == -1 || self.f >= *gcd {
            *gcd = self.f + 6;
            let mut ai = AttackInfo::new(ev.info.actor as usize, tag, icd_tag, icd::GROUP_REACTION_A, ele, aoe_dur, 0.0);
            ai.abil = abil;
            ai.ignore_def = 1.0;
            ai.pattern = self.circle_on_player(0.0, 0.0, 5.0);
            let mut atk = self.reaction_attack(ai);
            // hydro swirls only spread the element
            atk.info.flat_dmg *= if ele == Element::Hydro { 0.0 } else { 0.6 };
            self.queue_attack_with_snap(atk, 0, 1 + 4);
        }
        // electro off electro-charged swirls the hydro too
        if m == AURA_ELECTRO && ev.info.durability > ZERO_DUR && self.player_aura.get(AURA_HYDRO) > ZERO_DUR {
            self.player_swirl(ev, AURA_HYDRO);
        }
    }

    fn try_crystallize(&mut self, ev: &mut AttackEvent, m: usize, r: Reaction) {
        if ev.info.durability < ZERO_DUR {
            return;
        }
        if self.enemy.crystallize_gcd != -1 && self.f < self.enemy.crystallize_gcd {
            return;
        }
        self.enemy.crystallize_gcd = self.f + 60;
        let ele = match m {
            AURA_ELECTRO => Element::Electro,
            AURA_HYDRO => Element::Hydro,
            AURA_PYRO => Element::Pyro,
            _ => Element::Cryo,
        };
        self.add_task(Task::SpawnShard { actor: ev.info.actor, ele }, 23);
        let consumed = self.enemy.aura.reduce(m, ev.info.durability, 0.5);
        ev.info.durability = (ev.info.durability - consumed).max(0.0);
        ev.reacted = true;
        self.emit(Event::Reaction(r, ev));
        if m == AURA_ELECTRO && self.enemy.aura.get(AURA_HYDRO) > ZERO_DUR {
            self.check_ec();
        } else if m == AURA_FROZEN {
            self.check_freeze();
        }
    }

    fn calc(&mut self, ev: &mut AttackEvent, grp: f64) -> (f64, bool) {
        let s = &mut ev.stats;
        let st_ele = ev.info.element.dmg_stat();
        let ele_per = if st_ele != st::NONE { s[st_ele] } else { 0.0 };
        // reaction damage takes no DMG bonus
        let dmg_bonus = if ev.info.tag.is_reaction() { 0.0 } else { ele_per + s[st::DMGP] };
        let a = total_atk(s);
        let base = ev.info.mult * a * (1.0 + 0.0) + ev.info.flat_dmg;
        let mut damage = base * (1.0 + dmg_bonus);
        s[st::CR] = s[st::CR].clamp(0.0, 1.0);
        let (res, defadj) = self.enemy_resist_def(ev.info.element);
        let defadj = defadj.min(0.9);
        let defmod = (ev.char_lvl + 100) as f64 / ((ev.char_lvl + 100) as f64 + (self.enemy.level + 100) as f64 * (1.0 + defadj) * (1.0 - ev.info.ignore_def.min(1.0)));
        damage *= defmod;
        let resmod = if res < 0.0 {
            1.0 - res / 2.0
        } else if res < 0.75 {
            1.0 - res
        } else {
            1.0 / (4.0 * res + 1.0)
        };
        damage *= resmod;
        let mut crit = false;
        if self.rng.float64() <= s[st::CR] {
            damage *= 1.0 + s[st::CD];
            crit = true;
        }
        if ev.info.amped {
            let em = s[st::EM];
            let em_bonus = (2.78 * em) / (1400.0 + em);
            let react_bonus = self.react_bonus(ev.info.actor as usize, &ev.info);
            damage *= ev.info.amp_mult * (1.0 + em_bonus + react_bonus);
        }
        damage *= grp;
        (damage, crit)
    }

    /// The enemy's resistance to an element and its DEF adjustment (gcsim's
    /// resist and defAdj), from the mods that count now.
    fn enemy_resist_def(&self, e: Element) -> (f64, f64) {
        let mut r = self.enemy.resist[e as usize];
        let mut d = 0.0;
        for m in &self.enemy.mods {
            if m.expiry() > self.f {
                if m.kind == ModKind::Resist && m.ele == e {
                    r += m.value;
                } else if m.kind == ModKind::Def {
                    d += m.value;
                }
            }
        }
        (r, d)
    }

    fn damage_applied(&mut self, slot: u16) {
        let mut ev = self.slots[slot as usize];
        // applyDamage: in damage mode the hit can kill, which returns before
        // the element attaches
        let mut killed = false;
        if self.damage_mode {
            let actual = ev.damage.min(self.enemy_hp);
            self.enemy_hp -= actual;
            if self.enemy_hp <= 0.0 {
                self.enemy_alive = false;
                ev.damage = actual;
                killed = true;
            }
        }
        // an attack that did not react attaches its element
        if !killed && ev.info.durability > 0.0 && !ev.reacted && ev.info.element != Element::Physical && ev.info.durability >= ZERO_DUR {
            let d = ev.info.durability;
            let src = ev.info.actor as usize;
            match ev.info.element {
                Element::Pyro => self.enemy.aura.attach_refresh(AURA_PYRO, 0.8 * d, 6.0 * d + 420.0, src),
                Element::Hydro => self.enemy.aura.attach_overlap(AURA_HYDRO, 0.8 * d, 6.0 * d + 420.0, src),
                Element::Electro => self.enemy.aura.attach_overlap(AURA_ELECTRO, 0.8 * d, 6.0 * d + 420.0, src),
                Element::Cryo => self.enemy.aura.attach_overlap(AURA_CRYO, 0.8 * d, 6.0 * d + 420.0, src),
                Element::Dendro => self.enemy.aura.attach_overlap(AURA_DENDRO, 0.8 * d, 6.0 * d + 420.0, src),
                _ => {}
            }
        }
        self.total += ev.damage;
        let ci = ev.info.actor as usize;
        if self.f >= self.window_start && !ev.info.sim {
            self.window_char[ci] += ev.damage;
            let abils = &mut self.abils[ci];
            let name = ev.info.abil;
            match abils.iter_mut().find(|a| std::ptr::eq(a.0, name) || a.0 == name) {
                Some(a) => {
                    a.1 += 1;
                    a.2 += ev.damage;
                }
                None => abils.push((name, 1, ev.damage)),
            }
        }
        self.emit(Event::EnemyDamage(&ev));
        // electro-charged's hook subscribes when it starts, after the others
        self.ec_wane_hook(&ev);
        if ev.callbacks != 0 {
            crate::chars::dispatch::attack_cb(self.chars[ci].kind, self, ci, ev.callbacks, &ev);
        }
        self.free.push(slot);
    }

    // ------------------------------------------------------------ hitlag

    fn player_hitlag(&mut self, ci: usize, factor: f64, dur: f64) {
        if ci != self.player.active as usize {
            return;
        }
        let f = self.f;
        let c = &mut self.chars[ci];
        let new = (dur * (1.0 - factor)).ceil() as i32;
        let old = c.frozen;
        c.frozen = new.max(c.frozen);
        let ext = c.frozen - old;
        for m in &mut c.mods {
            if m.hitlag && m.expiry() != -1 && m.expiry() > f {
                m.extend(ext);
            }
        }
        c.queue.extend(ext);
        // ExtendInfusion
        if let Some(inf) = &mut self.infusion[ci] {
            if !(inf.expiry < f as f64 || inf.expiry == -1.0) {
                inf.expiry += dur * (1.0 - factor);
            }
        }
        if self.player.dash_cd_exp > f {
            self.player.dash_cd_exp += (dur * (1.0 - factor)).ceil() as i32;
        }
    }

    fn enemy_hitlag(&mut self, factor: f64, dur: f64) {
        let f = self.f;
        let e = &mut self.enemy;
        let new = (dur * (1.0 - factor)).ceil() as i32;
        let old = e.frozen;
        e.frozen = new.max(e.frozen);
        let ext = e.frozen - old;
        for m in &mut e.mods {
            if m.hitlag && m.expiry() != -1 && m.expiry() > f {
                m.extend(ext);
            }
        }
        e.queue.extend(ext);
    }

    // ------------------------------------------------------------ energy

    pub fn add_energy(&mut self, ci: usize, amt: f64) {
        let c = &mut self.chars[ci];
        c.energy = (c.energy + amt).min(c.energy_max).max(0.0);
    }

    pub fn queue_particle(&mut self, num: f64, ele: Element, delay: i32) {
        if delay == 0 {
            self.distribute_particle(num, ele);
        } else {
            self.add_task(Task::Particle { num, ele }, delay);
        }
    }

    fn distribute_particle(&mut self, num: f64, ele: Element) {
        let n = self.chars.len();
        for i in 0..n {
            let r = if i == self.player.active as usize { 1.0 } else { 1.0 - 0.1 * n as f64 };
            let amt = if ele == self.chars[i].element {
                3.0
            } else if ele == Element::NoElement {
                2.0
            } else {
                1.0
            };
            let er = self.stat(i, st::ER);
            let c = &mut self.chars[i];
            c.energy = (c.energy + amt * r * er * num).min(c.energy_max);
        }
        self.emit(Event::ParticleReceived);
    }

    /// gcsim's ConsumeEnergy, which emits OnEnergyBurst.
    fn drain_energy(&mut self, ci: usize) {
        let c = &mut self.chars[ci];
        let amount = c.energy_max;
        c.energy = (c.energy - amount).max(0.0);
        self.emit(Event::EnergyBurst { on: ci, amount });
    }

    pub fn consume_energy(&mut self, ci: usize, delay: i32) {
        if delay == 0 {
            self.drain_energy(ci);
        } else {
            self.add_task(Task::ConsumeEnergy { c: ci as u8, delay_amount: false }, delay);
        }
    }

    // ------------------------------------------------------------ cooldowns

    pub fn set_cd_with_delay(&mut self, ci: usize, a: Action, dur: i32, delay: i32) {
        if delay == 0 {
            self.set_cd(ci, a, dur);
        } else {
            self.add_task(Task::SetCd { c: ci as u8, a, dur }, delay);
        }
    }

    pub fn set_cd(&mut self, ci: usize, a: Action, dur: i32) {
        // CDReduction: anemo resonance is the only cooldown mod
        let dur = if self.anemo_res { (dur as f64 * (1.0 - 0.05)) as i32 } else { dur };
        let cd = &mut self.chars[ci].cd[Char::cd_index(a)];
        cd.queue[cd.len] = dur;
        cd.len += 1;
        if cd.len == 1 {
            self.start_cd_worker(ci, a);
        }
        let cd = &mut self.chars[ci].cd[Char::cd_index(a)];
        cd.available -= 1;
    }

    fn start_cd_worker(&mut self, ci: usize, a: Action) {
        let f = self.f;
        let cd = &mut self.chars[ci].cd[Char::cd_index(a)];
        if cd.len == 0 {
            return;
        }
        cd.started = f;
        cd.token += 1;
        let (token, dur) = (cd.token, cd.queue[0]);
        self.add_task(Task::CdWorker { c: ci as u8, a, token }, dur);
    }

    fn cd_worker(&mut self, ci: usize, a: Action, token: u32) {
        let cd = &mut self.chars[ci].cd[Char::cd_index(a)];
        if token != cd.token {
            return;
        }
        cd.available += 1;
        cd.queue.copy_within(1..4, 0);
        cd.len -= 1;
        if cd.len > 0 {
            self.start_cd_worker(ci, a);
        }
    }

    pub fn cooldown(&self, ci: usize, a: Action) -> i32 {
        let cd = &self.chars[ci].cd[Char::cd_index(a)];
        if cd.len == 0 {
            return 0;
        }
        cd.started + cd.queue[0] - self.f
    }

    /// gcsim's ResetActionCooldown.
    /// gcsim's ReduceActionCooldown.
    pub fn reduce_cd(&mut self, ci: usize, a: Action, v: i32) {
        let f = self.f;
        let cd = &mut self.chars[ci].cd[Char::cd_index(a)];
        if cd.available == 1 + cd.max_extra {
            return;
        }
        let remain = cd.started + cd.queue[0] - f;
        if v >= remain {
            self.reset_cd(ci, a);
            return;
        }
        cd.queue[0] = remain - v;
        self.start_cd_worker(ci, a);
    }

    pub fn reset_cd(&mut self, ci: usize, a: Action) {
        let f = self.f;
        let cd = &mut self.chars[ci].cd[Char::cd_index(a)];
        if cd.available == 1 + cd.max_extra {
            return;
        }
        cd.available += 1;
        cd.queue.copy_within(1..4, 0);
        cd.len -= 1;
        cd.started = f;
        cd.token += 1; // the running worker no longer counts
        if cd.len > 0 {
            self.start_cd_worker(ci, a);
        }
    }

    pub fn action_ready(&self, ci: usize, a: Action, p: &Params) -> bool {
        self.action_failure(ci, a, p).is_none()
    }

    /// gcsim's ActionReady: why the action cannot be used, if it cannot.
    pub fn action_failure(&self, ci: usize, a: Action, p: &Params) -> Option<Failure> {
        let c = &self.chars[ci];
        if let Some(r) = crate::chars::dispatch::action_ready(c.kind, self, ci, a, p) {
            return r;
        }
        match a {
            Action::Burst => {
                if !self.ignore_burst_energy && c.energy != c.energy_max {
                    return Some(Failure::InsufficientEnergy);
                }
                if c.cd[1].available <= 0 {
                    return Some(Failure::BurstCd);
                }
                None
            }
            Action::Skill if c.cd[0].available <= 0 => Some(Failure::SkillCd),
            Action::Charge if self.player.stam < self.abil_stam_cost(ci, a) => Some(Failure::InsufficientStamina),
            Action::Dash => {
                if self.player.stam < self.abil_stam_cost(ci, a) {
                    return Some(Failure::InsufficientStamina);
                }
                if self.player.active as usize == ci && self.player.dash_lockout && self.player.dash_cd_exp > self.f {
                    return Some(Failure::DashCd);
                }
                if self.player.active as usize != ci && c.dash_lockout && c.remaining_dash_cd > 0 {
                    return Some(Failure::DashCd);
                }
                None
            }
            _ => None,
        }
    }

    // ------------------------------------------------------------ stamina

    pub fn abil_stam_cost(&self, ci: usize, a: Action) -> f64 {
        let c = &self.chars[ci];
        let base = match a {
            Action::Charge => match crate::chars::dispatch::charge_stam(c.kind, self, ci) {
                Some(x) => x,
                None => match c.class {
                    WeaponClass::Sword => 20.0,
                    WeaponClass::Spear => 25.0,
                    WeaponClass::Catalyst => 50.0,
                    _ => 0.0,
                },
            },
            Action::Dash => 18.0,
            _ => 0.0,
        };
        let pct = self.stam_percent_mod(a);
        if pct == 0.0 {
            return base;
        }
        (1.0 + pct as f64).max(0.0) * base
    }

    /// gcsim's StamPercentMod, in the order the mods were added: anemo
    /// resonance's -15% on everything, then gcsim's "utility-dash"
    /// (Kazuha's and others') -20% on dashes, then Diona's A1.
    pub fn stam_percent_mod(&self, a: Action) -> f64 {
        let mut pct = 0.0;
        if self.anemo_res {
            pct += -0.15;
        }
        if a == Action::Dash && self.utility_dash.is_some_and(|o| self.chars[o as usize].hp_ratio > 0.0) {
            pct += -0.2;
        }
        if self.diona_a1 && self.shields.iter().any(|x| x.typ == ShieldType::DionaSkill) {
            pct += -0.1;
        }
        pct
    }

    /// gcsim's UseStam.
    pub fn use_stam(&mut self, amt: f64) {
        self.player.stam = (self.player.stam - amt).max(0.0);
        self.player.last_stam_use = self.f;
    }

    // ------------------------------------------------------------ swap

    fn do_swap(&mut self, to: usize) {
        let f = self.f;
        let prev = self.player.active as usize;
        self.player.active = to as u8;
        if self.player.dash_cd_exp > f {
            self.chars[prev].remaining_dash_cd = self.player.dash_cd_exp - f;
            self.chars[prev].dash_lockout = self.player.dash_lockout;
        }
        self.player.dash_cd_exp = f + self.chars[to].remaining_dash_cd;
        self.player.dash_lockout = self.chars[to].dash_lockout;
        self.chars[to].remaining_dash_cd = 0;
        self.player.swap_cd = self.swap_icd;
        for c in &mut self.chars {
            c.normal_counter = 0;
        }
        self.emit(Event::Swap { prev, next: to });
        // the shield stats collector, last to subscribe
        if !self.shield_bonus_mods.is_empty() {
            self.shield_bonus();
        }
    }

    // ------------------------------------------------------------ frames

    fn frames_of(&self, a: &ActInfo, next: Action) -> i32 {
        match a.frames {
            Frames::Table(t) => t[next as usize],
            Frames::Attack { table, atkspd } => atk_spd_adjust(table[next as usize], atkspd),
            Frames::Char { id, arg } => crate::chars::dispatch::frames(self.chars[a.char as usize].kind, id, arg, next),
            Frames::Dash { len, jump } => {
                if next == Action::Jump {
                    jump
                } else {
                    len
                }
            }
            Frames::Const(n) => n,
        }
    }

    fn can_use(&self, next: Action) -> bool {
        let Some(a) = &self.player.anim else { return true };
        if a.norm_for == Some(next) {
            return a.norm_time >= self.frames_of(a, next) as f64;
        }
        if a.pausable && self.chars[a.char as usize].frozen > 0 {
            return false;
        }
        a.time >= self.frames_of(a, next) as f64
    }

    fn can_queue_next(&self) -> bool {
        match &self.player.anim {
            None => true,
            Some(a) => a.time >= a.can_queue_after as f64,
        }
    }

    pub fn current_state(&self) -> AnimState {
        if self.player.anim.is_none() { AnimState::Idle } else { self.player.state }
    }

    fn set_action_used(&mut self, mut info: ActInfo) {
        info.char = self.player.active;
        if let Some(old) = self.player.anim {
            self.on_removed(&old, info.state);
        }
        self.player.state_start = self.f;
        self.player.anim = Some(info);
        self.on_state_change(info.state);
        self.player.state = info.state;
    }

    fn on_removed(&mut self, a: &ActInfo, next: AnimState) {
        match a.on_removed {
            OnRemoved::None => {}
            OnRemoved::Char(id) => crate::chars::dispatch::on_removed(self.chars[a.char as usize].kind, self, a.char as usize, id, next),
        }
    }

    fn anim_tick(&mut self) {
        // in place: the action is copied out only as it ends
        let Some(a) = self.player.anim.as_mut() else { return };
        a.norm_time += 1.0;
        if !a.pausable || self.chars[a.char as usize].frozen == 0 {
            a.time += 1.0;
        }
        if let Some((delay, to)) = a.swap {
            if delay <= a.time {
                a.swap = None;
                self.do_swap(to as usize);
            }
        }
        let Some(a) = self.player.anim.as_mut() else { return };
        if let Some((delay, id, arg)) = a.queued {
            if delay as f64 <= a.time {
                a.queued = None;
                let c = a.char;
                self.run_task(Task::Char { c, id, arg });
            }
        }
        if let Some(a) = self.player.anim.as_ref().filter(|a| a.time > a.anim_len as f64).copied() {
            self.on_removed(&a, AnimState::Idle);
            self.player.anim = None;
            self.player.state = AnimState::Idle;
            self.player.state_start = self.f;
        }
    }

    // ------------------------------------------------------------ bursts that watch normal attacks

    /// The active character's AnimationStartDelay for a key.
    pub fn start_delay(&self, key: usize) -> i32 {
        self.chars[self.player.active as usize].anim_delay[key]
    }

    fn on_state_change(&mut self, next: AnimState) {
        self.emit(Event::StateChange(next));
    }

    // ------------------------------------------------------------ frame loop

    /// gcsim's nextFrame; returns true when the fight is over.
    pub fn next_frame(&mut self) -> bool {
        self.f += 1;
        let f = self.f;
        // Combat.Tick: the player's aura, then the enemy
        if !self.player_aura.idle() {
            self.player_aura.tick();
        }
        if self.enemy.frozen > 0 {
            self.enemy.frozen -= 1;
            while let Some(t) = self.enemy.queue.pop_due(f) {
                self.run_task(t);
            }
        } else {
            while let Some(t) = self.enemy.queue.pop_due(f) {
                self.run_task(t);
            }
            if self.enemy.aura.tick() {
                self.check_freeze();
            }
            // electro-charged ends with either aura
            if self.enemy.ec_tick_src > -1 && (self.enemy.aura.below(AURA_ELECTRO, ZERO_DUR) || self.enemy.aura.below(AURA_HYDRO, ZERO_DUR)) {
                self.enemy.ec_tick_src = -1;
            }
        }
        // gadgets, those already there when the tick starts
        if !self.gadgets.is_empty() {
            self.tick_gadgets();
        }
        // shards last the same 15 s, so they go in the order they came
        if f >= self.next_expiry {
            while self.shards.first().is_some_and(|sh| sh.expiry == f) {
                self.shards.remove(0);
            }
            self.update_next_expiry();
        }
        // Constructs.Tick
        let mut k = 0;
        while k < self.constructs.len() {
            if self.constructs[k].expiry == f {
                let c = self.constructs.remove(k);
                crate::chars::dispatch::construct_expired(self.chars[c.owner as usize].kind, self, c.owner as usize, c.key);
            } else {
                k += 1;
            }
        }
        // Player.Tick
        if self.player.stam < 240.0 && f - self.player.last_stam_use > 90 {
            self.player.stam = (self.player.stam + 25.0 / 60.0).min(240.0);
        }
        if self.player.swap_cd > 0 {
            self.player.swap_cd -= 1;
        }
        if f >= self.next_expiry {
            self.tick_shields();
        }
        self.anim_tick();
        for i in 0..self.chars.len() {
            let c = &mut self.chars[i];
            if c.frozen > 0 {
                c.frozen -= 1;
            } else {
                c.time_passed += 1;
            }
            while let Some(t) = self.chars[i].queue.pop_due(f) {
                self.run_task(t);
            }
        }
        while let Some(t) = self.core.pop_due(f) {
            self.run_task(t);
        }
        // energy drops
        if let Some((start, end, amount)) = self.energy {
            if self.energy_once {
                self.energy = None;
                self.add_task(Task::Particle { num: amount as f64, ele: Element::NoElement }, start);
            } else if f - self.last_energy_drop >= start {
                let d = self.rng.intn(end - start);
                self.last_energy_drop = f + d;
                self.add_task(Task::Particle { num: amount as f64, ele: Element::NoElement }, d);
            }
        }
        if self.damage_mode {
            return self.no_more_actions || !self.enemy_alive;
        }
        f == self.end
    }

    // ------------------------------------------------------------ actions

    fn ready_check(&mut self, c: usize, a: Action, p: &Params) -> bool {
        if !self.can_use(a) {
            return false;
        }
        let active = self.player.active as usize;
        if a == Action::Swap {
            if active == c {
                return true;
            }
            if self.player.swap_cd > 0 {
                self.wait_failed(active, a, Failure::SwapCd);
                return false;
            }
            return true;
        }
        if let Some(why) = self.action_failure(active, a, p) {
            self.wait_failed(active, a, why);
            return false;
        }
        true
    }

    fn wait_failed(&mut self, ci: usize, a: Action, why: Failure) {
        let w = &mut self.wait_open[ci][a as usize];
        if w.is_none() {
            *w = Some((self.f, why));
        }
    }

    /// Ends an action's wait, counting its frames in the window.
    fn wait_done(&mut self, ci: usize, a: usize) {
        if let Some((start, why)) = self.wait_open[ci][a].take() {
            let n = self.f - start.max(self.window_start);
            if n > 0 {
                self.waits[ci][why as usize] += n as u32;
            }
        }
    }

    /// Counts the waits still open when the fight ends (gcsim's Flush).
    pub fn close_waits(&mut self) {
        for ci in 0..4 {
            for a in 0..N_ACTIONS {
                self.wait_done(ci, a);
            }
        }
    }

    fn exec(&mut self, c: usize, a: Action, p: &Params) {
        use crate::chars;
        let ci = self.player.active as usize;
        let kind = self.chars[ci].kind;
        let info = match a {
            Action::Charge => {
                let req = self.abil_stam_cost(ci, a);
                self.use_stam(req);
                chars::dispatch::charge(kind, self, ci, p)
            }
            Action::Dash => {
                let info = self.char_dash(ci);
                chars::dispatch::dash_info(kind, self, ci, info)
            }
            Action::Skill => {
                self.emit(Event::Skill);
                chars::dispatch::skill(kind, self, ci, p)
            }
            Action::Burst => {
                self.emit(Event::Burst);
                chars::dispatch::burst(kind, self, ci, p)
            }
            Action::Attack => chars::dispatch::attack(kind, self, ci, p),
            Action::Aim => chars::dispatch::aim(kind, self, ci, p),
            Action::HighPlunge => chars::dispatch::high_plunge(kind, self, ci, p),
            Action::LowPlunge => chars::dispatch::low_plunge(kind, self, ci, p),
            Action::Swap => {
                if self.player.swap_cd > 0 {
                    self.player.swap_cd = 0;
                }
                let d = self.swap_delay;
                let mut x = ActInfo::new(Frames::Const(d), d, d, AnimState::Swap);
                x.swap = Some((d as f64, c as u8));
                x.pausable = false;
                x
            }
            Action::Jump => self.char_jump(ci),
            Action::Walk => {
                let f = p.get("f").unwrap_or(1) as i32;
                ActInfo::new(Frames::Const(f), f, f, AnimState::Walk)
            }
            _ => unreachable!("unsupported action {a:?}"),
        };
        self.set_action_used(info);
        self.player.last = LastAction { char: ci as u8, action: a };
        if a != Action::Attack {
            for c in &mut self.chars {
                c.normal_counter = 0;
            }
        }
    }

    /// The template character's Dash.
    fn char_dash(&mut self, ci: usize) -> ActInfo {
        let f = self.f;
        if self.player.dash_cd_exp > f {
            self.player.dash_lockout = true;
            self.player.dash_cd_exp = f + 90;
        } else {
            self.player.dash_lockout = false;
            self.player.dash_cd_exp = f + 48;
        }
        let (len, jump) = (self.chars[ci].dash_len, self.chars[ci].dash_jump);
        self.add_task(Task::DashStam { c: ci as u8 }, len - 1);
        ActInfo::new(Frames::Dash { len, jump }, len, jump, AnimState::Dash)
    }

    /// The template character's Jump, outside Xianyun's and Stellar
    /// Swirl's airborne states.
    fn char_jump(&mut self, ci: usize) -> ActInfo {
        let c = &self.chars[ci];
        let after_dash = self.player.last.action == Action::Dash;
        // JumpLength
        let f = match (c.body, after_dash) {
            (Body::Girl | Body::Boy, true) => 34,
            (_, true) => 37,
            (Body::Boy | Body::Girl, false) => 31,
            (Body::Male, false) => 28,
            (Body::Lady, false) => 32,
            (Body::Loli, false) => 29,
        };
        let low_plunge = if matches!(c.class, WeaponClass::Sword | WeaponClass::Spear) { 5 } else { 4 };
        ActInfo::new(Frames::Const(f), 60, low_plunge, AnimState::Jump)
    }

    pub fn advance_normal(&mut self, ci: usize) {
        let c = &mut self.chars[ci];
        c.normal_counter += 1;
        if c.normal_counter == c.normal_hit_num {
            c.normal_counter = 0;
        }
    }

    // ------------------------------------------------------------ the program's side

    fn step(&mut self) -> bool {
        self.done = self.next_frame() || self.error.is_some();
        self.done
    }

    /// The team index of a character key, if it is on the team.
    pub fn index_of(&self, key: u16) -> Option<usize> {
        self.chars.iter().position(|c| c.key == key)
    }

    /// gcsim's execAction: waits until the action can be used, runs it, then
    /// runs frames until the next action can be queued. Returns false once
    /// the fight is over.
    pub fn queue_action(&mut self, c: usize, a: Action, p: &Params) -> bool {
        if self.done {
            return false;
        }
        loop {
            while !self.ready_check(c, a, p) {
                if self.step() {
                    return false;
                }
            }
            if self.pre_delay == 0 {
                break;
            }
            // executeActionDelay, then the ready check again
            while self.pre_delay > 0 {
                if self.chars[self.player.active as usize].frozen == 0 {
                    self.pre_delay -= 1;
                }
                if self.step() {
                    return false;
                }
            }
        }
        if a == Action::Swap && self.player.active as usize == c {
            return true;
        }
        self.wait_done(self.player.active as usize, a as usize);
        self.exec(c, a, p);
        while !self.can_queue_next() {
            if self.step() {
                return false;
            }
        }
        true
    }

    /// gcsim's wait(f): frames pass, with no ready check.
    pub fn wait(&mut self, f: i32) -> bool {
        for _ in 0..f {
            if self.done || self.step() {
                return false;
            }
        }
        !self.done
    }

    /// gcsim's delay(f): frames to wait before the next action is used.
    pub fn add_delay(&mut self, f: i32) {
        self.pre_delay += f;
    }

    /// Runs the fight to its end once the program has no more actions.
    pub fn finish(&mut self) {
        self.no_more_actions = true;
        while !self.done {
            self.step();
        }
    }

    // ------------------------------------------------------------ fields

    /// The duration left of a character's mod or status, by gcsim key.
    pub fn mod_duration(&self, ci: usize, key: &str) -> i32 {
        self.chars[ci]
            .mods
            .iter()
            .find(|m| m.key.name == key)
            .map(|m| if m.expiry() > self.f { m.expiry() - self.f } else { 0 })
            .unwrap_or(0)
    }

    /// gcsim's conditional.Eval: the value of a field such as
    /// .hutao.burst.ready.
    pub fn field(&self, fields: &[String]) -> Result<Num, String> {
        use crate::gcsl::{action_key, char_key, char_name, element_key, set_key, weapon_key};
        let need = |n: usize, what: &str| -> Result<(), String> {
            if fields.len() < n {
                Err(format!("bad {what} condition; invalid num of fields; expecting at least {n}; got {}", fields.len()))
            } else {
                Ok(())
            }
        };
        let b = Num::from_bool;
        let int = |v: i64| Num::Int(v);
        match fields[0].as_str() {
            "stam" => Ok(Num::Float(self.player.stam)),
            "state" => Ok(int(self.current_state() as i64)),
            "previous-action" => Ok(int(self.player.last.action as i64)),
            "previous-char" => Ok(int(self.chars[self.player.last.char as usize].key as i64)),
            "airborne" => Ok(b(false)),
            "status" => {
                need(2, "status")?;
                Ok(int(self.core_status(&fields[1]) as i64))
            }
            "dew" | "verdant-dew" | "moonridge-dew" => Ok(int(0)),
            "action" => {
                need(2, "action")?;
                action_key(&fields[1]).map(|a| int(a as i64)).ok_or(format!("bad action condition: invalid action {}", fields[1]))
            }
            "keys" => {
                need(3, "keys")?;
                let name = &fields[2];
                let k = match fields[1].as_str() {
                    "weapon" => weapon_key(name).map(|k| k as i64),
                    "set" => set_key(name).map(|k| k as i64),
                    "char" => char_key(name).map(|k| k as i64),
                    "element" => element_key(name).map(|e| e as i64),
                    t => return Err(format!("bad key condition: invalid type {t}")),
                };
                k.map(int).ok_or(format!("bad key condition: invalid {} {name}", fields[1]))
            }
            "element" => {
                need(3, "element")?;
                self.target(&fields[1])?;
                let d = match fields[2].as_str() {
                    "pyro" => self.enemy.aura.get(AURA_PYRO),
                    "hydro" => self.enemy.aura.get(AURA_HYDRO),
                    "electro" => self.enemy.aura.get(AURA_ELECTRO),
                    "cryo" | "dendro" | "quicken" | "frozen" | "geo" | "burning" => 0.0,
                    e => return Err(format!("bad element condition: invalid element {e}")),
                };
                Ok(Num::Float(if d < ZERO_DUR { 0.0 } else { d }))
            }
            "debuff" => {
                need(4, "debuff")?;
                self.target(&fields[2])?;
                let f = self.f;
                let active = |kind: ModKind| self.enemy.mods.iter().any(|m| m.kind == kind && m.key.name == fields[3] && m.active(f));
                match fields[1].as_str() {
                    "res" => Ok(b(active(ModKind::Resist))),
                    "def" => Ok(b(active(ModKind::Def))),
                    t => Err(format!("bad debuff condition: invalid type {t}")),
                }
            }
            "tags" => {
                need(3, "tags")?;
                self.target(&fields[1])?;
                // the enemy's tags (gcsim's GetTag, 0 when unset): those of
                // ported characters the engine keeps, or an error
                match fields[2].as_str() {
                    crate::gcsimdata::chars::lisa::CONDUCTIVE_TAG => Ok(int(crate::chars::lisa::conductive(self))),
                    t @ (crate::gcsimdata::chars::yelan::SKILL_MARKED_TAG | crate::gcsimdata::chars::lynette::SKILL_TAG | crate::gcsimdata::chars::hutao::BB_DEBUFF) => Err(format!("unsupported: enemy tag {t}")),
                    _ => Ok(int(0)),
                }
            }
            "gadgets" => {
                need(3, "gadgets")?;
                let f = self.f;
                match (fields[1].as_str(), fields[2].as_str()) {
                    ("dendrocore", "count") => Ok(int(self.gadgets.iter().filter(|g| g.owner == CORE).count() as i64)),
                    ("sourcewaterdroplet", "count") => Ok(int(self.gadgets.iter().filter(|g| g.owner != CORE && crate::chars::neuvillette::is_droplet(self, g)).count() as i64)),
                    ("crystallizeshard", e) => {
                        let ele = match e {
                            "all" => None,
                            "pyro" => Some(Element::Pyro),
                            "hydro" => Some(Element::Hydro),
                            "electro" => Some(Element::Electro),
                            "cryo" => Some(Element::Cryo),
                            _ => return Err(format!("bad gadgets (crystallizeshard) condition: invalid criteria {e}")),
                        };
                        Ok(int(self.shards.iter().filter(|sh| f >= sh.earliest_pickup && ele.is_none_or(|e| sh.shield.ele == e)).count() as i64))
                    }
                    ("dendrocore" | "sourcewaterdroplet", v) => Err(format!("bad gadgets ({}) condition: invalid criteria {v}", fields[1])),
                    (t, _) => Err(format!("bad gadgets condition: invalid criteria {t}")),
                }
            }
            "construct" => {
                need(3, "construct")?;
                let owner = char_key(&fields[2]).and_then(|k| self.index_of(k));
                let mine = || self.constructs.iter().filter(move |c| Some(c.owner as usize) == owner);
                match fields[1].as_str() {
                    "count" => Ok(int(mine().count() as i64)),
                    "duration" => Ok(int(mine().map(|c| c.expiry as i64).max().unwrap_or(0))),
                    v => Err(format!("bad construct condition: invalid criteria {v}")),
                }
            }
            name => {
                let key = char_key(name).ok_or(format!("invalid character {name} in character condition"))?;
                need(2, "character")?;
                let ci = self.index_of(key).ok_or(format!("character {} not in team when evaluating condition", char_name(key)))?;
                let c = &self.chars[ci];
                let typ = fields[1].as_str();
                if let Some(a) = action_key(typ) {
                    need(3, "character ability")?;
                    let active = self.player.active as usize == ci;
                    return match fields[2].as_str() {
                        "cd" => Ok(int(match a {
                            Action::Swap => self.player.swap_cd as i64,
                            Action::Dash => {
                                if active && self.player.dash_lockout {
                                    (self.player.dash_cd_exp - self.f) as i64
                                } else if !active && c.dash_lockout {
                                    c.remaining_dash_cd as i64
                                } else {
                                    0
                                }
                            }
                            _ => self.cooldown(ci, a) as i64,
                        })),
                        "charge" => Ok(int(c.cd[Char::cd_index(a)].available as i64)),
                        "ready" => Ok(b(if a == Action::Swap { self.player.swap_cd == 0 || active } else { self.action_ready(ci, a, &Params::default()) })),
                        t => Err(format!("bad character ability condition: invalid type {t}")),
                    };
                }
                if let Some(r) = crate::chars::dispatch::condition(c.kind, self, ci, &fields[1..]) {
                    return r;
                }
                let what = format!("character {typ}");
                match typ {
                    "id" => Ok(int(c.key as i64)),
                    "cons" => Ok(int(c.cons as i64)),
                    "energy" => Ok(Num::Float(c.energy)),
                    "energymax" => Ok(Num::Float(c.energy_max)),
                    "hp" => Ok(Num::Float(self.current_hp(ci))),
                    "hpmax" => Ok(Num::Float(self.max_hp(ci))),
                    "hpratio" => Ok(Num::Float(c.hp_ratio)),
                    "normal" => Ok(int(c.normal_counter as i64 + 1)),
                    "onfield" => Ok(b(self.player.active as usize == ci)),
                    "weapon" => Ok(int(c.weapon_key as i64)),
                    "status" | "mods" => {
                        need(3, &what)?;
                        Ok(int(self.mod_duration(ci, &fields[2]) as i64))
                    }
                    // gcsim's WeaponInfuseIsActive: the character's infusion
                    // has this key and has not expired
                    "infusion" => {
                        need(3, &what)?;
                        let f = self.f as f64;
                        Ok(b(self.infusion[ci].is_some_and(|i| i.key.name == fields[2] && !(i.expiry < f && i.expiry > -1.0))))
                    }
                    // the character's tags (0 when unset); those ported
                    // characters set are not kept
                    "tags" => {
                        need(3, &what)?;
                        match fields[2].as_str() {
                            t @ (crate::gcsimdata::chars::yaemiko::YAE_TOTEM_COUNT | crate::gcsimdata::chars::shenhe::QUILL_KEY) => Err(format!("unsupported: character tag {t}")),
                            _ => Ok(int(0)),
                        }
                    }
                    "stats" => {
                        need(3, &what)?;
                        let s = crate::types::st::by_name(&fields[2]).ok_or(format!("invalid stat key {} in character stat condition", fields[2]))?;
                        Ok(Num::Float(self.stat(ci, s)))
                    }
                    "bol" | "bolratio" => Ok(Num::Float(0.0)),
                    "sets" => {
                        need(3, &what)?;
                        let k = set_key(&fields[2]).ok_or(format!("invalid set key {} in character set condition", fields[2]))?;
                        Ok(Num::Float(c.set_counts.iter().find(|s| s.0 == k).map(|s| s.1 as f64).unwrap_or(0.0)))
                    }
                    _ => Err(format!("unsupported: field .{}", fields.join("."))),
                }
            }
        }
    }

    fn target(&self, t: &str) -> Result<(), String> {
        let id: i64 = t.trim_start_matches('t').parse().map_err(|_| format!("invalid target {t}"))?;
        if id != 0 {
            return Err(format!("invalid target {id}"));
        }
        Ok(())
    }
}

/// The action, as gcsl names it, if the engine does not implement it for
/// the character with these params.
pub fn unsupported_action(kind: CharKind, a: Action, p: &Params) -> Option<String> {
    match a {
        Action::Swap | Action::Dash | Action::Jump | Action::Walk => None,
        _ => crate::chars::dispatch::unsupported(kind, a, p),
    }
}

/// An action's params, such as hold=1.
#[derive(Default, Clone)]
pub struct Params(pub Vec<(String, i64)>);

impl Params {
    pub fn get(&self, k: &str) -> Option<i64> {
        self.0.iter().find(|p| p.0 == k).map(|p| p.1)
    }
}


#[cfg(test)]
#[path = "pattern_tests.rs"]
mod pattern_tests;
