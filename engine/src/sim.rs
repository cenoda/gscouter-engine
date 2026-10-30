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
    /// gcsim's HealBonusMod: incoming healing bonus, its amount in
    /// st::HEAL.
    HealBonus,
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

// ---------------------------------------------------------------- attacks

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
}

impl AttackEvent {
    pub fn new(info: AttackInfo, callbacks: u16) -> Self {
        AttackEvent { info, stats: [0.0; N_STATS], char_lvl: 0, snap_f: 0, callbacks, reacted: false, damage: 0.0, crit: false }
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
}

// ---------------------------------------------------------------- actions

#[derive(Clone, Copy, Debug)]
pub enum Frames {
    Table(&'static FrameTable),
    Attack { table: &'static FrameTable, atkspd: f64 },
    /// The character's own frames function, with an argument.
    Char { id: u8, arg: i8 },
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
    /// Pauses on the acting character's hitlag; a swap does not.
    pub pausable: bool,
}

impl ActInfo {
    pub fn new(frames: Frames, anim_len: i32, can_queue_after: i32, state: AnimState) -> Self {
        ActInfo { frames, anim_len, can_queue_after, state, char: 0, time: 0.0, norm_time: 0.0, norm_for: None, on_removed: OnRemoved::None, swap: None, pausable: true }
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
/// Aura slots the engine models, in gcsim's ReactionModKey order:
/// electro, pyro, hydro. Cryo, dendro and their reactions do not exist in
/// the engine yet, and nothing it implements applies them.
pub const AURA_ELECTRO: usize = 0;
pub const AURA_PYRO: usize = 1;
pub const AURA_HYDRO: usize = 2;
const N_AURAS: usize = 3;

#[derive(Clone, Copy, Default)]
pub struct Aura {
    /// Durability by aura and by the character that applied it.
    pub dur: [[f64; 4]; N_AURAS],
    pub decay: [f64; N_AURAS],
    pub live: [bool; N_AURAS],
}

impl Aura {
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
        self.live[m] = true;
    }
    /// gcsim's reduce: takes up to dur*factor off the aura and returns the
    /// attack durability that consumed.
    fn reduce(&mut self, m: usize, dur: f64, factor: f64) -> f64 {
        if self.below(m, ZERO_DUR) {
            return 0.0;
        }
        let red = (dur * factor).min(self.get(m));
        self.reduce_mod(m, red);
        red / factor
    }
    fn tick(&mut self) {
        for m in 0..N_AURAS {
            if !self.live[m] {
                continue;
            }
            if self.decay[m] != 0.0 && self.above(m, ZERO_DUR) {
                self.reduce_mod(m, self.decay[m]);
                if self.at_most(m, ZERO_DUR) {
                    self.decay[m] = 0.0;
                }
            } else if self.below(m, ZERO_DUR) {
                self.live[m] = false;
            }
        }
    }
    fn attach_overlap(&mut self, m: usize, amt: f64, length: f64, src: usize) {
        if self.get(m) > ZERO_DUR {
            let add = (amt - self.dur[m][src]).max(0.0);
            if add > 0.0 {
                self.dur[m][src] += add;
                self.live[m] = true;
            }
        } else {
            self.dur[m][src] = amt;
            self.live[m] = true;
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
        self.live[m] = true;
        if amt < self.get(m) {
            return;
        }
        self.decay[m] = amt / length;
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
    /// Electro-charged: the attack of its next ticks, the frame its ticks
    /// started (-1 without electro-charged), and whether the wane hook on
    /// enemy damage is subscribed.
    pub ec_atk: AttackEvent,
    pub ec_tick_src: i32,
    pub ec_wane_sub: bool,
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

#[derive(Clone, Copy, Debug)]
pub struct Shield {
    pub owner: u8,
    pub key: ModKey,
    pub expiry: i32,
}

pub struct Sim {
    pub f: i32,
    pub rng: Rng,
    pub core: Queue<Task>,
    pub chars: Vec<Char>,
    pub enemy: Enemy,
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
    /// Damage mode: the enemy has HP, and the fight ends when it dies or
    /// the program has no more actions.
    pub damage_mode: bool,
    pub enemy_hp: f64,
    pub enemy_alive: bool,
    pub no_more_actions: bool,
    /// Subscribers to each event, in the order gcsim subscribes them.
    pub subs: [Vec<Owner>; N_EVENTS],
    /// Constructs (gcsim's Constructs) and shields, by owner.
    pub constructs: Vec<Construct>,
    pub shields: Vec<Shield>,
    /// Weapon infusions by character (gcsim's infusion handler).
    pub infusion: [Option<Infusion>; 4],
    /// Statuses of the whole fight (gcsim's Status), with their expiry.
    pub statuses: Vec<(ModKey, i32)>,
    // normal attack energy
    pub na_icd: i32,
    pub na_chance: [f64; 4],
    // results
    pub window_start: i32,
    pub total: f64,
    pub window_char: [f64; 4],
    /// Frames in the window that actions waited, by character and reason.
    pub waits: [[u32; N_FAILURES]; 4],
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
            enemy: Enemy {
                level: cfg.enemy_level,
                resist: cfg.resist,
                frozen: 0,
                queue: Queue::new(),
                mods: Vec::with_capacity(16),
                icd: Vec::with_capacity(16),
                aura: Aura::default(),
                crystallize_gcd: -1,
                overload_gcd: -1,
                ec_atk: AttackEvent::new(AttackInfo::new(0, AttackTag::ECDamage, icd::TAG_EC_DAMAGE, icd::GROUP_REACTION_B, Element::Electro, 0.0, 0.0), 0),
                ec_tick_src: -1,
                ec_wane_sub: false,
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
            damage_mode: cfg.enemy_hp > 0.0,
            enemy_hp: cfg.enemy_hp,
            enemy_alive: true,
            no_more_actions: false,
            subs: Default::default(),
            constructs: Vec::with_capacity(4),
            shields: Vec::with_capacity(4),
            infusion: [None; 4],
            statuses: Vec::new(),
            na_icd: 0,
            na_chance: [0.0; 4],
            window_start: ((warmup * 60.0) as i32 / 30) * 30,
            total: 0.0,
            window_char: [0.0; 4],
            waits: [[0; N_FAILURES]; 4],
            abils: Default::default(),
        };
        s.setup();
        for (i, c) in cfg.chars.iter().enumerate() {
            if c.start_energy >= 0.0 {
                s.chars[i].energy = c.start_energy.min(s.chars[i].energy_max);
            }
        }
        // initialize advances one frame before the first action
        s.done = s.next_frame();
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
            let (pyro, hydro) = (count(Element::Pyro), count(Element::Hydro));
            for c in &mut self.chars {
                if pyro >= 2 {
                    c.mods.push(Mod::stat(PYRO_RESONANCE, st::ATKP, 0.25, false, false, -1));
                }
                if hydro >= 2 {
                    c.mods.push(Mod::stat(HYDRO_RESONANCE, st::HPP, 0.25, false, false, -1));
                }
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
            Event::Swap { .. } => {
                for (i, c) in self.chars.iter().enumerate() {
                    self.na_chance[i] = if c.class == WeaponClass::Sword { 0.10 } else { 0.0 };
                }
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
        self.emit(Event::Drain { on, amount });
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
            Task::ConsumeEnergy { c, .. } => {
                let ch = &mut self.chars[c as usize];
                ch.energy = (ch.energy - ch.energy_max).max(0.0);
            }
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
        self.handle_attack(slot);
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
        if info.icd_tag == icd::TAG_NONE {
            return 1.0;
        }
        let i = self.icd_state(info.actor, info.icd_tag, info.icd_group);
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
                Amount::Owner(Owner::Core) => {}
            }
        }
    }

    fn handle_attack(&mut self, slot: u16) {
        let mut ev = self.slots[slot as usize];
        let grp = self.group_dmg_mult(&ev.info);
        self.apply_attack_mods(ev.info.actor as usize, &mut ev);

        if ev.info.durability > 0.0 {
            ev.info.durability *= self.will_apply_ele(&ev.info);
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
                self.try_overload(ev);
                self.try_add_ec(ev);
            }
            Element::Pyro => {
                self.try_overload(ev);
                self.try_vaporize(ev);
            }
            Element::Hydro => {
                self.try_vaporize(ev);
                self.try_add_ec(ev);
            }
            Element::Geo => {
                for (m, r) in [(AURA_ELECTRO, Reaction::CrystallizeElectro), (AURA_HYDRO, Reaction::CrystallizeHydro), (AURA_PYRO, Reaction::CrystallizePyro)] {
                    if self.enemy.aura.get(m) > ZERO_DUR {
                        self.try_crystallize(ev, m, r);
                    }
                }
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
                if a.get(AURA_HYDRO) < ZERO_DUR {
                    return;
                }
                ev.info.amp_mult = 1.5;
                a.reduce(AURA_HYDRO, dur, 0.5)
            }
            Element::Hydro => {
                if a.get(AURA_PYRO) < ZERO_DUR {
                    return;
                }
                ev.info.amp_mult = 2.0;
                a.reduce(AURA_PYRO, dur, 2.0)
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
            Element::Electro if a.get(AURA_PYRO) >= ZERO_DUR => a.reduce(AURA_PYRO, dur, 1.0),
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
            let mut atk = self.reaction_attack(ai);
            atk.info.flat_dmg *= 2.75;
            self.queue_attack_with_snap(atk, 0, 1);
        }
    }

    /// gcsim's CalcReactionDmg: the attack with its flat damage and the
    /// snapshot it takes (the level and EM only).
    fn reaction_attack(&self, mut ai: AttackInfo) -> AttackEvent {
        let ci = ai.actor as usize;
        let em = self.stat(ci, st::EM);
        let lvl = self.chars[ci].level;
        ai.flat_dmg = (1.0 + (16.0 * em) / (2000.0 + em)) * crate::gcsimdata::combat::REACTION_LVL_BASE[(lvl - 1).clamp(0, 99) as usize];
        let mut atk = AttackEvent::new(ai, 0);
        atk.stats[st::EM] = em;
        atk.char_lvl = lvl;
        atk
    }

    fn try_add_ec(&mut self, ev: &mut AttackEvent) {
        let dur = ev.info.durability;
        if dur < ZERO_DUR {
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

    fn try_crystallize(&mut self, ev: &mut AttackEvent, m: usize, r: Reaction) {
        if ev.info.durability < ZERO_DUR {
            return;
        }
        if self.enemy.crystallize_gcd != -1 && self.f < self.enemy.crystallize_gcd {
            return;
        }
        self.enemy.crystallize_gcd = self.f + 60;
        let consumed = self.enemy.aura.reduce(m, ev.info.durability, 0.5);
        ev.info.durability = (ev.info.durability - consumed).max(0.0);
        ev.reacted = true;
        self.emit(Event::Reaction(r, ev));
        if m == AURA_ELECTRO && self.enemy.aura.get(AURA_HYDRO) > ZERO_DUR {
            self.check_ec();
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
        let res = self.enemy_resist(ev.info.element);
        let defmod = (ev.char_lvl + 100) as f64 / ((ev.char_lvl + 100) as f64 + (self.enemy.level + 100) as f64 * (1.0 + 0.0) * (1.0 - ev.info.ignore_def.min(1.0)));
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
            damage *= ev.info.amp_mult * (1.0 + em_bonus);
        }
        damage *= grp;
        (damage, crit)
    }

    fn enemy_resist(&self, e: Element) -> f64 {
        let mut r = self.enemy.resist[e as usize];
        for m in &self.enemy.mods {
            if m.kind == ModKind::Resist && m.expiry() > self.f && m.ele == e {
                r += m.value;
            }
        }
        r
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
                _ => {}
            }
        }
        self.total += ev.damage;
        let ci = ev.info.actor as usize;
        if self.f >= self.window_start {
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
            crate::chars::dispatch::attack_cb(self.chars[ci].kind, self, ci, ev.callbacks);
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
    }

    pub fn consume_energy(&mut self, ci: usize, delay: i32) {
        if delay == 0 {
            let c = &mut self.chars[ci];
            c.energy = (c.energy - c.energy_max).max(0.0);
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
        match a {
            Action::Charge => {
                if let Some(x) = crate::chars::dispatch::charge_stam(c.kind, self, ci) {
                    return x;
                }
                match c.class {
                    WeaponClass::Sword => 20.0,
                    WeaponClass::Spear => 25.0,
                    WeaponClass::Catalyst => 50.0,
                    _ => 0.0,
                }
            }
            Action::Dash => 18.0,
            _ => 0.0,
        }
    }

    fn use_stam(&mut self, amt: f64) {
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
        let Some(mut a) = self.player.anim else { return };
        a.norm_time += 1.0;
        if !a.pausable || self.chars[a.char as usize].frozen == 0 {
            a.time += 1.0;
        }
        if let Some((delay, to)) = a.swap {
            if delay <= a.time {
                a.swap = None;
                self.player.anim = Some(a);
                self.do_swap(to as usize);
            }
        }
        if a.time > a.anim_len as f64 {
            self.on_removed(&a, AnimState::Idle);
            self.player.anim = None;
            self.player.state = AnimState::Idle;
            self.player.state_start = self.f;
            return;
        }
        self.player.anim = Some(a);
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
        // Combat.Tick: the enemy
        if self.enemy.frozen > 0 {
            self.enemy.frozen -= 1;
            while let Some(t) = self.enemy.queue.pop_due(f) {
                self.run_task(t);
            }
        } else {
            while let Some(t) = self.enemy.queue.pop_due(f) {
                self.run_task(t);
            }
            self.enemy.aura.tick();
            // electro-charged ends with either aura
            if self.enemy.ec_tick_src > -1 && (self.enemy.aura.below(AURA_ELECTRO, ZERO_DUR) || self.enemy.aura.below(AURA_HYDRO, ZERO_DUR)) {
                self.enemy.ec_tick_src = -1;
            }
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
        self.shields.retain(|s| s.expiry != f);
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
        let counted = self.f >= self.window_start;
        if a == Action::Swap {
            if active == c {
                return true;
            }
            if self.player.swap_cd > 0 {
                if counted {
                    self.waits[active][Failure::SwapCd as usize] += 1;
                }
                return false;
            }
            return true;
        }
        if let Some(why) = self.action_failure(active, a, p) {
            if counted {
                self.waits[active][why as usize] += 1;
            }
            return false;
        }
        true
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
            Action::Dash => self.char_dash(ci),
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
        self.done = self.next_frame();
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
                    "def" => Ok(b(false)),
                    t => Err(format!("bad debuff condition: invalid type {t}")),
                }
            }
            "tags" => {
                need(3, "tags")?;
                self.target(&fields[1])?;
                Ok(int(0))
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
                    "infusion" => {
                        need(3, &what)?;
                        Ok(b(false))
                    }
                    "tags" => {
                        need(3, &what)?;
                        Ok(int(0))
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

