//! The fight: gcsim's frame loop, task queues, actions, hitlag, mods and
//! damage, kept in the order gcsim runs them.

use crate::queue::Queue;
use crate::rng::Rng;
use crate::types::*;

// ---------------------------------------------------------------- mods

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModKey {
    HomaHp,
    HomaAtk,
    Shim2,
    Shim4,
    Shim4Icd,
    HutaoA4,
    HutaoA1,
    Paramita,
    ParamitaIcd,
    // team stage
    HydroResonance,
    Emblem2,
    Emblem4,
    Tenacity4Icd,
    Tenacity4,
    SacIcd,
    AquaSim,
    XingqiuC2,
    XingqiuOrbital,
    XingqiuBurstStatus,
    XingqiuBurstIcd,
    YelanBurst,
    YelanA4,
    YelanBurstIcd,
    ZhongliShield,
    Millelith,
    HomaAtkZhongli,
    Other(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModKind {
    Stat,
    Attack,
    Status,
}

#[derive(Clone, Copy, Debug)]
pub struct Mod {
    pub key: ModKey,
    pub kind: ModKind,
    pub affected: usize,
    pub extra: bool,
    pub hitlag: bool,
    pub expiry_base: i32,
    pub extension: i32,
}

impl Mod {
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

fn mod_add(mods: &mut Vec<Mod>, m: Mod) {
    match mod_find(mods, m.key) {
        Some(i) => mods[i] = m,
        None => mods.push(m),
    }
}

// ---------------------------------------------------------------- attacks

pub const CB_HUTAO_PARTICLE: u16 = 1 << 0;
pub const CB_HUTAO_BB: u16 = 1 << 1;
pub const CB_HUTAO_HEAL: u16 = 1 << 2;

#[derive(Clone, Copy, Debug)]
pub struct AttackInfo {
    pub actor: u8,
    pub abil: u16,
    pub tag: AttackTag,
    pub icd_tag: IcdTag,
    pub icd_group: IcdGroup,
    pub element: Element,
    pub durability: f64,
    pub mult: f64,
    pub flat_dmg: f64,
    pub use_hp: bool,
    pub hitlag_factor: f64,
    pub hitlag_halt: f64,
    pub def_halt: bool,
    pub deployable: bool,
    pub amped: bool,
    pub amp_mult: f64,
}

impl AttackInfo {
    pub fn new(actor: usize, tag: AttackTag, icd_tag: IcdTag, icd_group: IcdGroup, element: Element, durability: f64, mult: f64) -> Self {
        AttackInfo {
            actor: actor as u8,
            abil: 0,
            tag,
            icd_tag,
            icd_group,
            element,
            durability,
            mult,
            flat_dmg: 0.0,
            use_hp: false,
            hitlag_factor: 0.0,
            hitlag_halt: 0.0,
            def_halt: false,
            deployable: false,
            amped: false,
            amp_mult: 1.0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct AttackEvent {
    pub info: AttackInfo,
    pub stats: Stats,
    pub char_lvl: i32,
    pub callbacks: u16,
    pub reacted: bool,
    pub damage: f64,
    pub crit: bool,
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
    ConsumeEnergy { c: u8 },
    AddEnergy { c: u8, amt: f64 },
    Particle { num: f64, ele: Element },
    DashStam { c: u8 },
    IcdResetDmg { i: u16 },
    IcdResetTag { i: u16 },
    Swap { to: u8 },
    HutaoA1 { c: u8 },
    HutaoBBTick { c: u8, src: i32 },
}

// ---------------------------------------------------------------- actions

#[derive(Clone, Copy, Debug)]
pub enum Frames {
    Table(&'static FrameTable),
    Attack { table: &'static FrameTable, atkspd: f64 },
    HutaoPPCharge { prev: i8 },
    Dash { len: i32, jump: i32 },
    Const(i32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OnRemoved {
    None,
    HutaoPPCharge,
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
pub enum CharKind {
    Hutao,
    Xingqiu,
    Zhongli,
    Yelan,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WeaponClass {
    Sword,
    Claymore,
    Spear,
    Bow,
    Catalyst,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Weapon {
    Homa,
    SacrificialSword,
    AquaSimulacra,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Set {
    Shimenawa4,
    Emblem4,
    Tenacity4,
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

#[derive(Clone, Copy, Default)]
pub struct HutaoState {
    pub ppbuff_atk: f64,
    pub apply_a1: bool,
    pub heal_count: i32,
    pub heal_src: f64,
}

pub struct Char {
    pub kind: CharKind,
    pub base: Stats,
    pub level: i32,
    pub cons: i32,
    pub ascension: i32,
    pub talents: [i32; 3],
    pub element: Element,
    pub class: WeaponClass,
    pub weapon: Weapon,
    pub refine: i32,
    pub set: Option<Set>,
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
    pub hutao: HutaoState,
}

impl Char {
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
    pub tag: u8,
    pub group: u8,
    pub tag_on: bool,
    pub tag_count: usize,
    pub dmg_on: bool,
    pub dmg_count: usize,
}

pub struct Enemy {
    pub level: i32,
    pub resist: f64,
    pub frozen: i32,
    pub queue: Queue<Task>,
    pub mods: Vec<Mod>,
    pub bb_src: i32,
    pub icd: Vec<IcdState>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EModKey {
    BloodBlossom,
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
    pub last: LastAction,
}

// ---------------------------------------------------------------- program

#[derive(Clone, Copy, Debug)]
pub enum Cond {
    BurstReady(u8),
    SkillReady(u8),
    True,
}

#[derive(Clone, Copy, Debug)]
pub enum Ins {
    Act(u8, Action, i32),
    /// Jump to target when the condition is false (negate flips it).
    JmpUnless(Cond, bool, usize),
    Jmp(usize),
    SetCtr(u8, i32),
    IncCtr(u8),
    /// Jump to target when the counter is at least the value.
    JmpCtrGe(u8, i32, usize),
}

#[derive(Clone)]
pub struct Program {
    pub ins: Vec<Ins>,
    pub pc: usize,
    pub ctr: [i32; 4],
    pub swapped: bool,
}

// ---------------------------------------------------------------- sim

pub struct Config {
    pub chars: Vec<CharSetup>,
    pub active: usize,
    pub duration: f64,
    pub enemy_level: i32,
    pub resist: f64,
    pub energy_interval: (i32, i32),
    pub swap_delay: i32,
    pub program: Vec<Ins>,
}

#[derive(Clone)]
pub struct CharSetup {
    pub kind: CharKind,
    pub base: Stats,
    pub level: i32,
    pub cons: i32,
    pub ascension: i32,
    pub talents: [i32; 3],
    pub weapon: Weapon,
    pub refine: i32,
    pub set: Option<Set>,
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
    pub energy_interval: (i32, i32),
    pub last_energy_drop: i32,
    pub program: Program,
    // normal attack energy
    pub na_icd: i32,
    pub na_chance: [f64; 4],
    // results
    pub window_start: i32,
    pub total: f64,
    pub window_char: [f64; 4],
    pub waits: [f64; 4],
    /// Hits and damage in the window by character and ability code.
    pub abils: [[(f64, f64); 32]; 4],
}

impl Sim {
    pub fn new(cfg: &Config, seed: u64, warmup: f64) -> Sim {
        let chars = cfg.chars.iter().map(Sim::make_char).collect();
        let mut s = Sim {
            f: 0,
            rng: Rng::new(seed),
            core: Queue::new(),
            chars,
            enemy: Enemy { level: cfg.enemy_level, resist: cfg.resist, frozen: 0, queue: Queue::new(), mods: Vec::with_capacity(8), bb_src: -1, icd: Vec::with_capacity(16) },
            player: Player {
                active: cfg.active as u8,
                stam: 240.0,
                last_stam_use: 0,
                swap_cd: 0,
                dash_cd_exp: 0,
                dash_lockout: false,
                anim: None,
                state: AnimState::Idle,
                last: LastAction { char: 0, action: Action::Invalid },
            },
            slots: Vec::with_capacity(64),
            free: Vec::with_capacity(64),
            end: (cfg.duration * 60.0) as i32,
            swap_delay: cfg.swap_delay,
            energy_interval: cfg.energy_interval,
            last_energy_drop: 0,
            program: Program { ins: cfg.program.clone(), pc: 0, ctr: [0; 4], swapped: false },
            na_icd: 0,
            na_chance: [0.0; 4],
            window_start: ((warmup * 60.0) as i32 / 30) * 30,
            total: 0.0,
            window_char: [0.0; 4],
            waits: [0.0; 4],
            abils: [[(0.0, 0.0); 32]; 4],
        };
        s.init();
        s
    }

    fn make_char(cs: &CharSetup) -> Char {
        let (element, class, energy_max, normal_hit_num, dash_len, dash_jump) = match cs.kind {
            CharKind::Hutao => (Element::Pyro, WeaponClass::Spear, 60.0, 6, 20, 4),
            CharKind::Xingqiu => (Element::Hydro, WeaponClass::Sword, 80.0, 5, 21, 2),
            CharKind::Zhongli => (Element::Geo, WeaponClass::Spear, 40.0, 6, 19, 3),
            CharKind::Yelan => (Element::Hydro, WeaponClass::Bow, 70.0, 4, 22, 3),
        };
        let mut c = Char {
            kind: cs.kind,
            base: cs.base,
            level: cs.level,
            cons: cs.cons,
            ascension: cs.ascension,
            talents: cs.talents,
            element,
            class,
            weapon: cs.weapon,
            refine: cs.refine,
            set: cs.set,
            energy: energy_max,
            energy_max,
            hp_ratio: 1.0,
            normal_counter: 0,
            normal_hit_num,
            mods: Vec::with_capacity(20),
            frozen: 0,
            time_passed: 0,
            queue: Queue::new(),
            cd: [Cooldown { available: 1, ..Default::default() }; 3],
            remaining_dash_cd: 0,
            dash_lockout: false,
            dash_len,
            dash_jump,
            hutao: HutaoState::default(),
        };
        // weapon, then set, as gcsim's AddChar
        match cs.weapon {
            Weapon::Homa => {
                c.mods.push(perm(ModKey::HomaHp, st::NONE, false));
                c.mods.push(perm(ModKey::HomaAtk, st::ATK, true));
            }
            _ => {}
        }
        match cs.set {
            Some(Set::Shimenawa4) => c.mods.push(perm(ModKey::Shim2, st::ATKP, false)),
            _ => {}
        }
        c
    }

    fn init(&mut self) {
        for i in 0..self.chars.len() {
            match self.chars[i].kind {
                CharKind::Hutao => self.hutao_init(i),
                _ => {}
            }
        }
        for c in &mut self.chars {
            c.hp_ratio = 1.0;
        }
        // initialize advances one frame before the first action
        self.next_frame();
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
        let c = &self.chars[ci];
        match m.key {
            ModKey::HomaHp => {
                out[st::HPP] += 0.15 + c.refine as f64 * 0.05;
            }
            ModKey::HomaAtk => {
                let maxhp = self.max_hp(ci);
                let mut per = 0.006 + c.refine as f64 * 0.002;
                if c.hp_ratio <= 0.5 {
                    per += 0.008 + c.refine as f64 * 0.002;
                }
                out[st::ATK] += per * maxhp;
            }
            ModKey::Shim2 => out[st::ATKP] += 0.18,
            ModKey::HutaoA4 => {
                if c.hp_ratio <= 0.5 {
                    out[st::PYROP] += 0.33;
                } else {
                    return false;
                }
            }
            ModKey::HutaoA1 => out[st::CR] += 0.12,
            ModKey::Paramita => out[st::ATK] += c.hutao.ppbuff_atk,
            _ => return false,
        }
        true
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

    /// gcsim's Stat(s).
    pub fn stat(&self, ci: usize, s: usize) -> f64 {
        let c = &self.chars[ci];
        let mut v = c.base[s];
        for m in &c.mods {
            if m.kind != ModKind::Stat || (m.affected != st::NONE && m.affected != s) {
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

    pub fn heal(&mut self, ci: usize, src: f64, bonus: f64) {
        let amt = src * (1.0 + bonus);
        self.modify_hp(ci, amt);
    }

    // ------------------------------------------------------------ mods

    pub fn add_mod(&mut self, ci: usize, key: ModKey, kind: ModKind, affected: usize, extra: bool, dur: i32, hitlag: bool) {
        let expiry = if dur < 0 { -1 } else { self.f + dur };
        mod_add(&mut self.chars[ci].mods, Mod { key, kind, affected, extra, hitlag, expiry_base: expiry, extension: 0 });
    }

    pub fn add_status(&mut self, ci: usize, key: ModKey, dur: i32, hitlag: bool) {
        self.add_mod(ci, key, ModKind::Status, st::NONE, false, dur, hitlag);
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
        mod_add(&mut self.enemy.mods, Mod { key, kind: ModKind::Status, affected: st::NONE, extra: false, hitlag, expiry_base: expiry, extension: 0 });
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
            Task::ConsumeEnergy { c } => {
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
            Task::HutaoA1 { c } => self.hutao_a1(c as usize),
            Task::HutaoBBTick { c, src } => self.hutao_bb_tick(c as usize, src),
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
        let slot = self.alloc(AttackEvent { info, stats: [0.0; N_STATS], char_lvl: 0, callbacks, reacted: false, damage: 0.0, crit: false });
        self.queue_attack_slot(slot, snap, dmg);
    }

    /// Queues an attack from a character's queue at a hitmark: QueueAttack
    /// runs when the task does.
    pub fn queue_attack_at(&mut self, ci: usize, info: AttackInfo, callbacks: u16, delay: i32) {
        let slot = self.alloc(AttackEvent { info, stats: [0.0; N_STATS], char_lvl: 0, callbacks, reacted: false, damage: 0.0, crit: false });
        self.queue_char_task(ci, Task::QueueAttack { slot, snap: 0, dmg: 0 }, delay);
    }

    fn queue_dmg(&mut self, slot: u16, delay: i32) {
        if delay == 0 {
            self.apply_attack(slot);
        } else {
            self.add_task(Task::ApplyAttack { slot }, delay);
        }
    }

    fn snapshot(&mut self, slot: u16) {
        let ci = self.slots[slot as usize].info.actor as usize;
        let stats = self.stats(ci);
        let mut info = self.slots[slot as usize].info;
        match self.chars[ci].kind {
            CharKind::Hutao => self.hutao_snapshot(ci, &mut info),
            _ => {}
        }
        let ev = &mut self.slots[slot as usize];
        ev.stats = stats;
        ev.char_lvl = self.chars[ci].level;
        ev.info = info;
    }

    fn apply_attack(&mut self, slot: u16) {
        // one enemy, always in range
        self.handle_attack(slot);
        let info = self.slots[slot as usize].info;
        if !info.deployable {
            let mut dur = info.hitlag_halt;
            if info.def_halt {
                dur += 3.6;
            }
            if dur > 0.0 {
                self.player_hitlag(info.actor as usize, info.hitlag_factor, dur);
            }
        }
    }

    fn icd_state(&mut self, ci: u8, tag: IcdTag, grp: IcdGroup) -> usize {
        let (t, g) = (tag as u8, grp as u8);
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
        if info.icd_tag == IcdTag::None {
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

    fn apply_attack_mods(&self, ci: usize, ev: &mut AttackEvent) {
        let c = &self.chars[ci];
        for m in &c.mods {
            if m.kind != ModKind::Attack || !m.applies(self.f) {
                continue;
            }
            match m.key {
                ModKey::Shim4 => {
                    if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra | AttackTag::Plunge) {
                        ev.stats[st::DMGP] += 0.5;
                    }
                }
                _ => {}
            }
        }
    }

    fn handle_attack(&mut self, slot: u16) {
        let mut ev = self.slots[slot as usize];
        let grp = self.group_dmg_mult(&ev.info);
        self.apply_attack_mods(ev.info.actor as usize, &mut ev);

        if ev.info.durability > 0.0 {
            ev.info.durability *= self.will_apply_ele(&ev.info);
            // reactions come with the team stage
        }
        let (dmg, crit) = self.calc(&mut ev, grp);
        // enemy hitlag
        let mut dur = ev.info.hitlag_halt;
        if ev.info.def_halt {
            dur += 3.6;
        }
        let dur = dur.ceil();
        if dur > 0.0 {
            self.enemy_hitlag(ev.info.hitlag_factor, dur);
        }
        ev.damage = dmg;
        ev.crit = crit;
        self.slots[slot as usize] = ev;
        self.add_task(Task::DamageApplied { slot }, 0);
    }

    fn calc(&mut self, ev: &mut AttackEvent, grp: f64) -> (f64, bool) {
        let s = &mut ev.stats;
        let st_ele = ev.info.element.dmg_stat();
        let ele_per = if st_ele != st::NONE { s[st_ele] } else { 0.0 };
        let dmg_bonus = ele_per + s[st::DMGP];
        let a = if ev.info.use_hp { max_hp(s) } else { total_atk(s) };
        let base = ev.info.mult * a * (1.0 + 0.0) + ev.info.flat_dmg;
        let mut damage = base * (1.0 + dmg_bonus);
        s[st::CR] = s[st::CR].clamp(0.0, 1.0);
        let res = self.enemy_resist(ev.info.element);
        let defadj = 0.0f64.min(0.9);
        let def_ign = 0.0f64;
        let defmod = (ev.char_lvl + 100) as f64 / ((ev.char_lvl + 100) as f64 + (self.enemy.level + 100) as f64 * (1.0 + defadj) * (1.0 - def_ign));
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

    fn enemy_resist(&self, _e: Element) -> f64 {
        self.enemy.resist
    }

    fn damage_applied(&mut self, slot: u16) {
        let ev = self.slots[slot as usize];
        self.total += ev.damage;
        if self.f >= self.window_start {
            self.window_char[ev.info.actor as usize] += ev.damage;
            let a = &mut self.abils[ev.info.actor as usize][ev.info.abil as usize];
            a.0 += 1.0;
            a.1 += ev.damage;
        }
        // random energy from normal and charged attacks
        if matches!(ev.info.tag, AttackTag::Normal | AttackTag::Extra) && self.na_icd <= self.f {
            let ci = ev.info.actor as usize;
            let (base_inc, reset) = match self.chars[ci].class {
                WeaponClass::Sword => (0.05, 0.10),
                WeaponClass::Claymore => (0.10, 0.0),
                WeaponClass::Spear => (0.04, 0.0),
                WeaponClass::Bow => (0.05, 0.0),
                WeaponClass::Catalyst => (0.10, 0.0),
            };
            if self.rng.float64() > self.na_chance[ci] {
                self.na_chance[ci] += base_inc;
            } else {
                self.add_energy(ci, 1.0);
                self.na_icd = self.f + 12;
                self.na_chance[ci] = reset;
            }
        }
        let cbs = ev.callbacks;
        let ci = ev.info.actor as usize;
        if cbs & CB_HUTAO_PARTICLE != 0 {
            self.hutao_particle_cb(ci);
        }
        if cbs & CB_HUTAO_BB != 0 {
            self.hutao_apply_bb(ci);
        }
        if cbs & CB_HUTAO_HEAL != 0 {
            self.hutao_burst_heal_cb(ci);
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

    // ------------------------------------------------------------ cooldowns

    pub fn set_cd_with_delay(&mut self, ci: usize, a: Action, dur: i32, delay: i32) {
        if delay == 0 {
            self.set_cd(ci, a, dur);
        } else {
            self.add_task(Task::SetCd { c: ci as u8, a, dur }, delay);
        }
    }

    fn set_cd(&mut self, ci: usize, a: Action, dur: i32) {
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

    pub fn action_ready(&self, ci: usize, a: Action) -> bool {
        let c = &self.chars[ci];
        match a {
            Action::Burst => c.energy == c.energy_max && c.cd[1].available > 0,
            Action::Skill => c.cd[0].available > 0,
            Action::Charge => self.player.stam >= self.abil_stam_cost(ci, a),
            Action::Dash => {
                if self.player.stam < self.abil_stam_cost(ci, a) {
                    return false;
                }
                if self.player.active as usize == ci && self.player.dash_lockout && self.player.dash_cd_exp > self.f {
                    return false;
                }
                if self.player.active as usize != ci && c.dash_lockout && c.remaining_dash_cd > 0 {
                    return false;
                }
                true
            }
            _ => true,
        }
    }

    // ------------------------------------------------------------ stamina

    pub fn abil_stam_cost(&self, ci: usize, a: Action) -> f64 {
        let c = &self.chars[ci];
        match a {
            Action::Charge => {
                if c.kind == CharKind::Hutao && c.cons >= 1 && self.mod_active(ci, ModKey::Paramita) {
                    return 0.0;
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
        self.player.swap_cd = 60;
        for c in &mut self.chars {
            c.normal_counter = 0;
        }
        self.on_swap(prev, to);
    }

    fn on_swap(&mut self, prev: usize, _to: usize) {
        for i in 0..self.chars.len() {
            if self.chars[i].kind == CharKind::Hutao && self.mod_active(i, ModKey::Paramita) {
                self.hutao_a1(i);
                self.delete_mod(i, ModKey::Paramita);
            }
        }
        let _ = prev;
        for c in &mut self.na_chance {
            *c = 0.0;
        }
        for (i, c) in self.chars.iter().enumerate() {
            if c.class == WeaponClass::Sword {
                self.na_chance[i] = 0.10;
            }
        }
    }

    // ------------------------------------------------------------ frames

    fn frames_of(&self, a: &ActInfo, next: Action) -> i32 {
        match a.frames {
            Frames::Table(t) => t[next as usize],
            Frames::Attack { table, atkspd } => atk_spd_adjust(table[next as usize], atkspd),
            Frames::HutaoPPCharge { prev } => crate::hutao::pp_charge_frames(prev, next),
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

    fn set_action_used(&mut self, mut info: ActInfo) {
        info.char = self.player.active;
        if let Some(old) = self.player.anim {
            self.on_removed(&old, info.state);
        }
        self.player.state = info.state;
        self.player.anim = Some(info);
    }

    fn on_removed(&mut self, a: &ActInfo, next: AnimState) {
        match a.on_removed {
            OnRemoved::None => {}
            OnRemoved::HutaoPPCharge => {
                if next != AnimState::Burst {
                    self.extend_mod(a.char as usize, ModKey::Paramita, -108);
                }
            }
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
            return;
        }
        self.player.anim = Some(a);
    }

    // ------------------------------------------------------------ frame loop

    /// gcsim's nextFrame; returns true when the fight is over.
    pub fn next_frame(&mut self) -> bool {
        self.f += 1;
        let f = self.f;
        // Combat.Tick: the enemy
        if self.enemy.frozen > 0 {
            self.enemy.frozen -= 1;
        }
        while let Some(t) = self.enemy.queue.pop_due(f) {
            self.run_task(t);
        }
        // Player.Tick
        if self.player.stam < 240.0 && f - self.player.last_stam_use > 90 {
            self.player.stam = (self.player.stam + 25.0 / 60.0).min(240.0);
        }
        if self.player.swap_cd > 0 {
            self.player.swap_cd -= 1;
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
        let (start, end) = self.energy_interval;
        if start > 0 && f - self.last_energy_drop >= start {
            let d = self.rng.intn(end - start);
            self.last_energy_drop = f + d;
            self.add_task(Task::Particle { num: 1.0, ele: Element::NoElement }, d);
        }
        f == self.end
    }

    // ------------------------------------------------------------ actions

    fn ready_check(&mut self, c: usize, a: Action) -> bool {
        if !self.can_use(a) {
            return false;
        }
        let active = self.player.active as usize;
        if a == Action::Swap {
            if active == c {
                return true;
            }
            if self.player.swap_cd > 0 {
                self.waits[active] += 1.0 / 60.0;
                return false;
            }
            return true;
        }
        if !self.action_ready(active, a) {
            if self.f >= self.window_start {
                self.waits[active] += 1.0 / 60.0;
            }
            return false;
        }
        true
    }

    fn exec(&mut self, c: usize, a: Action, param: i32) {
        let ci = self.player.active as usize;
        let info = match a {
            Action::Charge => {
                let req = self.abil_stam_cost(ci, a);
                self.use_stam(req);
                self.char_charge(ci)
            }
            Action::Dash => self.char_dash(ci),
            Action::Skill => {
                self.on_skill_event(ci);
                self.char_skill(ci, param)
            }
            Action::Burst => self.char_burst(ci),
            Action::Attack => self.char_attack(ci),
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
            _ => unimplemented!("{a:?}"),
        };
        self.set_action_used(info);
        self.player.last = LastAction { char: ci as u8, action: a };
        if a != Action::Attack {
            for c in &mut self.chars {
                c.normal_counter = 0;
            }
        }
    }

    fn on_skill_event(&mut self, ci: usize) {
        // Shimenawa 4pc
        let active = self.player.active as usize;
        for i in 0..self.chars.len() {
            if self.chars[i].set != Some(Set::Shimenawa4) || active != i {
                continue;
            }
            if self.chars[i].energy < 15.0 || self.mod_active(i, ModKey::Shim4Icd) {
                continue;
            }
            self.add_status(i, ModKey::Shim4Icd, 600, true);
            self.add_task(Task::AddEnergy { c: i as u8, amt: -15.0 }, 10);
            self.add_mod(i, ModKey::Shim4, ModKind::Attack, st::NONE, false, 600, true);
        }
        let _ = ci;
    }

    fn char_attack(&mut self, ci: usize) -> ActInfo {
        match self.chars[ci].kind {
            CharKind::Hutao => self.hutao_attack(ci),
            _ => unimplemented!(),
        }
    }
    fn char_charge(&mut self, ci: usize) -> ActInfo {
        match self.chars[ci].kind {
            CharKind::Hutao => self.hutao_charge(ci),
            _ => unimplemented!(),
        }
    }
    fn char_skill(&mut self, ci: usize, _param: i32) -> ActInfo {
        match self.chars[ci].kind {
            CharKind::Hutao => self.hutao_skill(ci),
            _ => unimplemented!(),
        }
    }
    fn char_burst(&mut self, ci: usize) -> ActInfo {
        match self.chars[ci].kind {
            CharKind::Hutao => self.hutao_burst(ci),
            _ => unimplemented!(),
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

    pub fn advance_normal(&mut self, ci: usize) {
        let c = &mut self.chars[ci];
        c.normal_counter += 1;
        if c.normal_counter == c.normal_hit_num {
            c.normal_counter = 0;
        }
    }

    // ------------------------------------------------------------ program

    fn cond(&self, c: Cond) -> bool {
        match c {
            Cond::BurstReady(i) => self.action_ready(i as usize, Action::Burst),
            Cond::SkillReady(i) => self.action_ready(i as usize, Action::Skill),
            Cond::True => true,
        }
    }

    fn next_action(&mut self) -> Option<(usize, Action, i32)> {
        loop {
            let ins = *self.program.ins.get(self.program.pc)?;
            match ins {
                Ins::Act(c, a, p) => {
                    if !self.program.swapped && c != self.player.active {
                        self.program.swapped = true;
                        return Some((c as usize, Action::Swap, 0));
                    }
                    self.program.swapped = false;
                    self.program.pc += 1;
                    return Some((c as usize, a, p));
                }
                Ins::JmpUnless(cond, neg, t) => {
                    if self.cond(cond) == neg {
                        self.program.pc = t;
                    } else {
                        self.program.pc += 1;
                    }
                }
                Ins::Jmp(t) => self.program.pc = t,
                Ins::SetCtr(i, v) => {
                    self.program.ctr[i as usize] = v;
                    self.program.pc += 1;
                }
                Ins::IncCtr(i) => {
                    self.program.ctr[i as usize] += 1;
                    self.program.pc += 1;
                }
                Ins::JmpCtrGe(i, v, t) => {
                    if self.program.ctr[i as usize] >= v {
                        self.program.pc = t;
                    } else {
                        self.program.pc += 1;
                    }
                }
            }
        }
    }

    /// Runs the fight to its end: gcsim's state machine.
    pub fn run(&mut self) {
        loop {
            // queue phase
            let Some((c, a, p)) = self.next_action() else {
                while !self.next_frame() {}
                return;
            };
            // ready check
            while !self.ready_check(c, a) {
                if self.next_frame() {
                    return;
                }
            }
            // execute
            if a == Action::Swap && self.player.active as usize == c {
                continue;
            }
            self.exec(c, a, p);
            // skip until the next action can be queued
            while !self.can_queue_next() {
                if self.next_frame() {
                    return;
                }
            }
        }
    }
}

fn perm(key: ModKey, affected: usize, extra: bool) -> Mod {
    Mod { key, kind: ModKind::Stat, affected, extra, hitlag: false, expiry_base: -1, extension: 0 }
}
