//! Builds the engine's fight from a parsed config, as gcsim's AddChar and
//! UpdateBaseStats do: base stats from the catalog's curves and ascension
//! data, plus the config's added stats.

use crate::data;
use crate::gcsl::{char_name, set_name, weapon_name, CharProfile, ParsedConfig};
use crate::sim::*;
use crate::types::*;

fn curve(id: u32) -> &'static [f64] {
    data::CURVES.iter().find(|c| c.0 == id).map(|c| c.1).expect("curve")
}

/// gcsim's AvatarBaseStat and AvatarAsc.
fn char_base(p: &CharProfile, d: &data::CharData) -> (Stats, i32) {
    let mut res = [0.0; N_STATS];
    let lvl = (p.level - 1).clamp(0, 99) as usize;
    res[st::BASEHP] = d.base[0] * curve(d.curves[0])[lvl];
    res[st::BASEATK] = d.base[1] * curve(d.curves[1])[lvl];
    res[st::BASEDEF] = d.base[2] * curve(d.curves[2])[lvl];
    res[st::EM] = d.em;
    res[st::ER] += 1.0;
    res[st::CD] += 0.5;
    res[st::CR] += 0.05;
    let mut asc = 0;
    let mut ind = None;
    for (i, (max, _)) in d.promo.iter().enumerate() {
        if p.max_level >= *max as i32 {
            ind = Some(i);
            asc = i as i32;
        }
    }
    if let Some(i) = ind {
        for (s, v) in d.promo[i].1 {
            res[*s] += v;
        }
    }
    (res, asc)
}

/// gcsim's WeaponBaseStat.
fn weapon_base(p: &CharProfile, d: &data::WeaponData) -> Stats {
    let mut res = [0.0; N_STATS];
    let lvl = (p.weapon.level - 1).clamp(0, 89) as usize;
    for (s, init, c) in d.props {
        res[*s] = init * curve(*c)[lvl];
    }
    let mut ind = None;
    for (i, (max, _)) in d.promo.iter().enumerate() {
        if p.weapon.max_level >= *max as i32 {
            ind = Some(i);
        }
    }
    if let Some(i) = ind {
        for (s, v) in d.promo[i].1 {
            res[*s] += v;
        }
    }
    res
}

/// The characters gcsim makes Hexerei (IsHexerei), unless their param
/// hexerei=0.
const HEXEREI: &[&str] = &["durin", "varka", "sucrose", "mona", "razor", "fischl", "prune", "lohen", "nicole"];

/// The engine's character for a gcsim key name, if it implements it.
pub fn char_kind(name: &str) -> Option<CharKind> {
    CharKind::from_name(name)
}

fn char_setup(p: &CharProfile) -> Result<CharSetup, Vec<String>> {
    let mut errs = Vec::new();
    let name = char_name(p.key);
    let kind = char_kind(name);
    if kind.is_none() {
        errs.push(format!("unsupported: character {name}"));
    }
    let wname = weapon_name(p.weapon.key);
    let weapon = WeaponKind::from_name(wname);
    if weapon.is_none() {
        errs.push(format!("unsupported: weapon {wname}"));
    }
    let mut sets = Vec::new();
    for (k, count) in &p.sets {
        let s = set_name(*k);
        // a single piece of a set does nothing
        if *count < 2 {
            continue;
        }
        match SetKind::from_name(s) {
            Some(kind) => sets.push((kind, *count)),
            None => errs.push(format!("unsupported: set {s}")),
        }
    }
    if p.talents.iter().any(|t| !(1..=10).contains(t)) {
        errs.push(format!("{name}: invalid talent levels {:?}", p.talents));
    }
    let (mut start_hp, mut start_hp_ratio, mut start_energy) = (-1.0, -1.0, -1.0);
    for (k, v) in &p.params {
        match (k.as_str(), kind) {
            ("start_hp", _) => start_hp = *v as f64,
            ("start_hp%", _) => start_hp_ratio = *v as f64,
            ("start_energy", _) => start_energy = *v as f64,
            ("breakthrough", Some(CharKind::Yelan)) => {}
            ("oz_travel" | "hexerei", Some(CharKind::Fischl)) => {}
            // the character itself is unsupported
            (_, None) => {}
            _ => errs.push(format!("unsupported: param {name} {k}")),
        }
    }
    let (Some(kind), Some(weapon), true) = (kind, weapon, errs.is_empty()) else {
        return Err(errs);
    };
    let cd = data::CHARS.iter().find(|c| c.key == name).unwrap();
    let wd = data::WEAPONS.iter().find(|w| w.key == wname).unwrap();
    let mut base = p.stats;
    let (cb, asc) = char_base(p, cd);
    for i in 0..N_STATS {
        base[i] += cb[i];
    }
    let wb = weapon_base(p, wd);
    for i in 0..N_STATS {
        base[i] += wb[i];
    }
    Ok(CharSetup {
        kind,
        key: p.key,
        weapon_key: p.weapon.key,
        set_counts: p.sets.clone(),
        base,
        level: p.level,
        cons: p.cons,
        ascension: asc,
        talents: p.talents,
        weapon,
        refine: p.weapon.refine,
        sets,
        start_hp,
        start_hp_ratio,
        start_energy,
        params: p.params.clone(),
        weapon_params: p.weapon.params.clone(),
    })
}

/// The fight a parsed config describes, or every reason the engine cannot
/// run it, each unsupported feature as "unsupported: <what> <name>".
pub fn config(p: &ParsedConfig) -> Result<Config, Vec<String>> {
    let mut errs = Vec::new();
    if p.targets.len() != 1 {
        errs.push(format!("unsupported: targets {}", p.targets.len()));
    }
    for t in &p.targets {
        if t.typed {
            errs.push("unsupported: target type".into());
        }
        if t.particle_threshold > 0.0 {
            errs.push("unsupported: target particle drops".into());
        }
    }
    if p.hurt {
        errs.push("unsupported: hurt".into());
    }
    if p.chars.len() >= 4 {
        let mut count = [0; 11];
        for c in &p.chars {
            if let Some(d) = data::CHARS.iter().find(|d| d.key == char_name(c.key)) {
                count[d.element as usize] += 1;
            }
        }
        for (e, name) in [(3, "dendro"), (5, "cryo"), (6, "anemo"), (7, "geo")] {
            if count[e] >= 2 {
                errs.push(format!("unsupported: resonance {name}"));
            }
        }
    }
    // gcsim's Hexerei bonuses need two Hexerei characters; the engine
    // implements none of them
    let hexerei = p.chars.iter().filter(|c| HEXEREI.contains(&char_name(c.key)) && !c.params.iter().any(|(k, v)| k == "hexerei" && *v == 0)).count();
    if hexerei >= 2 {
        errs.push("unsupported: hexerei".into());
    }
    let mut chars = Vec::new();
    for c in &p.chars {
        match char_setup(c) {
            Ok(c) => chars.push(c),
            Err(e) => errs.extend(e),
        }
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    let t = &p.targets[0];
    let active = p.chars.iter().position(|c| Some(c.key) == p.active).unwrap();
    let e = &p.energy;
    Ok(Config {
        chars,
        active,
        duration: if p.duration == 0.0 { 90.0 } else { p.duration },
        enemy_level: t.level,
        resist: t.resist,
        energy: if e.active { Some((e.start, e.end, e.amount)) } else { None },
        energy_once: e.once,
        swap_delay: p.swap_delay,
        hitlag: p.hitlag,
        defhalt: p.defhalt,
        ignore_burst_energy: p.ignore_burst_energy,
        enemy_hp: t.hp,
    })
}
