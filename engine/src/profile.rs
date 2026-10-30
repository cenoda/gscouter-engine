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

fn char_setup(p: &CharProfile) -> Result<CharSetup, String> {
    let name = char_name(p.key);
    let kind = match name {
        "hutao" => CharKind::Hutao,
        "xingqiu" => CharKind::Xingqiu,
        "zhongli" => CharKind::Zhongli,
        "yelan" => CharKind::Yelan,
        _ => return Err(format!("character {name} is not implemented")),
    };
    let wname = weapon_name(p.weapon.key);
    let weapon = match wname {
        "staffofhoma" => Weapon::Homa,
        "sacrificialsword" => Weapon::SacrificialSword,
        "aquasimulacra" => Weapon::AquaSimulacra,
        _ => return Err(format!("{name}: weapon {wname} is not implemented")),
    };
    let mut sets = [0; N_SETS];
    for (k, count) in &p.sets {
        let s = set_name(*k);
        match s {
            "shimenawasreminiscence" => sets[Set::Shimenawa as usize] = *count,
            "emblemofseveredfate" => sets[Set::Emblem as usize] = *count,
            "tenacityofthemillelith" => sets[Set::Tenacity as usize] = *count,
            // a single piece of a set does nothing
            _ if *count < 2 => {}
            _ => return Err(format!("{name}: set {s} is not implemented")),
        }
    }
    if p.talents.iter().any(|t| !(1..=10).contains(t)) {
        return Err(format!("{name}: invalid talent levels {:?}", p.talents));
    }
    let (mut start_hp, mut start_hp_ratio, mut start_energy) = (-1.0, -1.0, -1.0);
    for (k, v) in &p.params {
        match (k.as_str(), kind) {
            ("start_hp", _) => start_hp = *v as f64,
            ("start_hp%", _) => start_hp_ratio = *v as f64,
            ("start_energy", _) => start_energy = *v as f64,
            ("breakthrough", CharKind::Yelan) => {}
            _ => return Err(format!("{name}: param {k} is not implemented")),
        }
    }
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
    })
}

pub fn config(p: &ParsedConfig) -> Result<Config, String> {
    if p.targets.len() != 1 {
        return Err(format!("the engine fights one target; the config has {}", p.targets.len()));
    }
    let t = &p.targets[0];
    if t.typed {
        return Err("target types are not implemented".into());
    }
    if t.particle_threshold > 0.0 {
        return Err("target particle drops are not implemented".into());
    }
    if p.hurt {
        return Err("hurt is not implemented".into());
    }
    let chars = p.chars.iter().map(char_setup).collect::<Result<Vec<_>, _>>()?;
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
