//! Checks a record: structure, units and ranges, references (sources,
//! values, characters, ICD tags and groups, curves), the provenance of every
//! observation, agreement between observations at display precision, the
//! base stats at the official breakpoints, and that it converts under both
//! selections.

use crate::catalog::Catalog;
use crate::convert::{self, Target};
use crate::record::{self, Evidence, Record, Select, UnitKind, Val, BREAKPOINTS, TALENT_LEVELS};
use crate::toml::Value;
use std::collections::HashSet;

pub fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10 && b[4] == b'-' && b[7] == b'-' && s.bytes().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) && (1..=12).contains(&s[5..7].parse::<u32>().unwrap_or(0)) && (1..=31).contains(&s[8..].parse::<u32>().unwrap_or(0))
}

fn is_url(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

/// A video timestamp: `m:ss`, `h:mm:ss`, or a range of them with `-`.
fn is_timestamp(s: &str) -> bool {
    s.split('-').all(|p| {
        let parts: Vec<&str> = p.trim().split(':').collect();
        (2..=3).contains(&parts.len()) && parts.iter().all(|x| !x.is_empty() && x.bytes().all(|c| c.is_ascii_digit()))
    })
}

/// Every problem with a record; empty when it is valid.
pub fn validate(r: &Record, cat: &Catalog) -> Vec<String> {
    let mut e = Vec::new();
    let p = &r.path;
    if r.kind != "character" {
        e.push(format!("{p}: kind `{}`: only `character` records are converted yet (weapons and sets: docs/content-format.md §6)", r.kind));
    }
    match cat.char_key(&r.key) {
        Some(k) if k == r.key => {}
        Some(k) => e.push(format!("{p}: key `{}` is an alias; the record's key is the canonical `{k}`", r.key)),
        None => e.push(format!("{p}: key `{}` is not in the engine's key table (data.rs CHAR_NAMES); add the name there first", r.key)),
    }
    if r.module.is_empty() || !r.module.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_') {
        e.push(format!("{p}: module `{}` must be lowercase letters, digits and `_`", r.module));
    }
    for s in &r.sources {
        let c = format!("{p}: source `{}` (line {})", s.id, s.line);
        if !record::SOURCE_KINDS.contains(&s.kind.as_str()) {
            e.push(format!("{c}: kind `{}` is not one of {}", s.kind, record::SOURCE_KINDS.join(", ")));
        }
        match (&s.url, &s.video) {
            (None, None) => e.push(format!("{c}: needs `url` or `video`")),
            (Some(u), _) if matches!(s.kind.as_str(), "legacy_gcsim" | "project") => {
                if is_url(u) || !cat.repo_file(u).exists() {
                    e.push(format!("{c}: a {} source is a file of this repository, and `{u}` is not one", s.kind));
                }
            }
            (Some(u), _) if !is_url(u) => e.push(format!("{c}: url `{u}` is not http(s)")),
            (_, Some(v)) if !is_url(v) => e.push(format!("{c}: video `{v}` is not http(s)")),
            _ => {}
        }
        if s.kind == "video" && s.video.is_none() {
            e.push(format!("{c}: a video source needs `video`"));
        }
        if let Some(d) = &s.observed
            && !is_date(d) {
                e.push(format!("{c}: observed `{d}` is not YYYY-MM-DD"));
            }
    }
    let mut names = HashSet::new();
    for (n, ty, len) in &r.items {
        if !names.insert(n) {
            e.push(format!("{p}: item `{n}` declared twice"));
        }
        if !n.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_') {
            e.push(format!("{p}: item `{n}` must be UPPER_SNAKE_CASE"));
        }
        if let Err(x) = crate::rustdata::Ty::parse(ty) {
            e.push(format!("{p}: item `{n}`: {x}"));
        }
        let len_ok = match len {
            None => true,
            Some(Value::Int(i)) => *i >= 0,
            Some(Value::Array(a)) => a.iter().all(|x| x.as_int().is_some_and(|i| i >= 0)),
            _ => false,
        };
        if !len_ok || (len.is_some() && !ty.starts_with("&[")) {
            e.push(format!("{p}: item `{n}`: `len` is a slice's length, or the lengths of its rows"));
        }
    }
    let mut keys = HashSet::new();
    for v in &r.values {
        if !keys.insert(v.key.as_str()) {
            e.push(format!("{p}: value `{}` (line {}) is defined twice", v.key, v.line));
        }
        value(r, v, cat, &mut e);
    }
    // converts under both selections
    for sel in [Select::Legacy, Select::Independent] {
        match convert::convert(r, sel) {
            Ok(out) => {
                if let Some(row) = &out.row {
                    breakpoints(r, row, sel, cat, &mut e);
                }
            }
            Err(errs) => e.extend(errs.into_iter().map(|x| format!("{p}: selection {}: {x}", sel.name()))),
        }
    }
    e.sort();
    e.dedup();
    e
}

fn value(r: &Record, v: &Val, cat: &Catalog, e: &mut Vec<String>) {
    let c = format!("{}: value `{}` (line {})", r.path, v.key, v.line);
    if v.key.is_empty() || !v.key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.[]-".contains(&b)) {
        e.push(format!("{c}: a key is lowercase letters, digits and `_ . [ ] -`"));
    }
    let Some(kind) = record::unit_kind(&v.unit) else {
        e.push(format!("{c}: unit `{}` is not a unit (docs/content-format.md §3)", v.unit));
        return;
    };
    if !(0..=2).contains(&v.layer) {
        e.push(format!("{c}: layer {} is not 0, 1 or 2", v.layer));
    }
    if (v.layer == 0) != (v.unit == "id") {
        e.push(format!("{c}: layer 0 is for engine identifiers (unit id), and they are layer 0"));
    }
    match &v.field {
        Some(f) if !record::FIELDS.contains(&f.as_str()) => e.push(format!("{c}: field `{f}` is not one of {}", record::FIELDS.join(", "))),
        None if v.layer == 2 => e.push(format!("{c}: a layer 2 value names its `field` ({})", record::FIELDS.join(", "))),
        _ => {}
    }
    match v.by.as_deref() {
        None | Some("talent_level") => {}
        Some("breakpoint") => {
            if !v.check || !matches!(v.stat.as_deref(), Some("base_hp" | "base_atk" | "base_def" | "em")) {
                e.push(format!("{c}: a breakpoint value is `check = true` with `stat` base_hp, base_atk, base_def or em"));
            }
        }
        Some(b) => e.push(format!("{c}: by `{b}` is not talent_level or breakpoint")),
    }
    if v.target.is_none() && v.engine.is_none() && !v.check {
        e.push(format!("{c}: give its `target` (an item or char.<field>), or `engine` (where the engine's logic holds it), or `check = true`"));
    }
    if let Some(t) = &v.target {
        match convert::parse_target(t) {
            Ok(Target::Item(n, _)) if !r.items.iter().any(|(i, _, _)| *i == n) => e.push(format!("{c}: target item `{n}` is not in [items]")),
            Err(x) => e.push(format!("{c}: {x}")),
            _ => {}
        }
    }
    if v.obs.is_empty() {
        e.push(format!("{c}: it has no observation"));
    }
    if kind == UnitKind::Any && v.obs.iter().any(|o| o.evidence != Evidence::LegacyGcsim) {
        e.push(format!("{c}: give it a unit before other evidence than legacy_gcsim (it is unclassified)"));
    }
    for s in &v.sensitivity {
        if s.alternative.is_empty() || s.basis.is_empty() {
            e.push(format!("{c}: a sensitivity names its alternative and basis"));
        }
    }
    let mut payloads = Vec::new();
    for (oi, o) in v.obs.iter().enumerate() {
        let oc = format!("{c}, observation {} (line {})", oi + 1, o.line);
        let src = o.source.as_ref().and_then(|s| r.source(s));
        match (&o.source, src) {
            (None, _) if o.evidence == Evidence::Assigned => {}
            (None, _) => e.push(format!("{oc}: `source` is required")),
            (Some(s), None) => e.push(format!("{oc}: source `{s}` is not in [source]")),
            _ => {}
        }
        if let Some(src) = src {
            let legacy_src = src.kind == "legacy_gcsim";
            if (o.evidence == Evidence::LegacyGcsim) != legacy_src {
                e.push(format!("{oc}: legacy_gcsim evidence comes from a legacy_gcsim source, and only it does"));
            }
            if matches!(o.evidence, Evidence::Official | Evidence::OfficialRounded) && !matches!(src.kind.as_str(), "official_page" | "official_text") {
                e.push(format!("{oc}: {} evidence needs an official source, not `{}`", o.evidence.name(), src.kind));
            }
            if o.evidence == Evidence::Measured && !matches!(src.kind.as_str(), "video" | "sheet" | "article" | "community") {
                e.push(format!("{oc}: measured evidence needs a video, sheet, article or community source"));
            }
            if src.kind == "video" && !o.at.as_deref().is_some_and(is_timestamp) {
                e.push(format!("{oc}: a video source needs `at` with its timestamp (m:ss)"));
            }
            match o.observed.as_ref().or(src.observed.as_ref()) {
                None => e.push(format!("{oc}: no observation date (on it or its source)")),
                Some(d) if !is_date(d) => e.push(format!("{oc}: observed `{d}` is not YYYY-MM-DD")),
                _ => {}
            }
            if o.game_version.as_ref().or(src.game_version.as_ref()).is_none_or(|g| g.is_empty()) {
                e.push(format!("{oc}: no game version (on it or its source)"));
            }
        }
        match o.evidence {
            Evidence::Derived => {
                if o.formula.as_deref().is_none_or(str::is_empty) || o.inputs.is_empty() {
                    e.push(format!("{oc}: derived evidence gives `formula` and `inputs`"));
                }
                for i in &o.inputs {
                    let k = i.split('@').next().unwrap();
                    if r.value(k).is_none() {
                        e.push(format!("{oc}: input `{i}` is not a value of this record"));
                    }
                }
            }
            Evidence::Estimated => {
                if o.basis.as_deref().is_none_or(str::is_empty) {
                    e.push(format!("{oc}: estimated evidence gives its `basis`"));
                }
            }
            Evidence::OfficialRounded if o.shown.is_none() => e.push(format!("{oc}: official_rounded evidence gives `shown`, the values as the source shows them")),
            Evidence::Assigned if v.unit != "id" => e.push(format!("{oc}: assigned evidence is for engine identifiers (unit id)")),
            _ => {}
        }
        if o.analogy.is_some() && o.evidence != Evidence::Estimated {
            e.push(format!("{oc}: `analogy` belongs to estimated evidence"));
        }
        if let Some(a) = &o.analogy
            && cat.char_key(a).is_none() {
                e.push(format!("{oc}: analogy `{a}` is not a character"));
            }
        if o.shown.is_some() && !matches!(o.evidence, Evidence::Official | Evidence::OfficialRounded) {
            e.push(format!("{oc}: `shown` is for official values; give others as `data`"));
        }
        match convert::payload(v, o) {
            Ok((val, tols)) => {
                leaves_ok(&val, &v.unit, kind, cat, &oc, e);
                if v.unit == "frames" && matches!(val, Value::Table(_)) && !v.target.as_deref().is_some_and(|t| t.starts_with("char."))
                    && let Err(x) = convert::frame_table(&val) {
                        e.push(format!("{oc}: {x}"));
                    }
                match v.by.as_deref() {
                    Some("talent_level") => match val.as_array() {
                        Some(a) if a.iter().all(|x| !matches!(x, Value::Array(_) | Value::Table(_))) => {
                            if o.first + a.len() - 1 > TALENT_LEVELS {
                                e.push(format!("{oc}: levels {} to {} pass {TALENT_LEVELS}", o.first, o.first + a.len() - 1));
                            }
                        }
                        _ => e.push(format!("{oc}: by talent_level, its values are one array")),
                    },
                    Some("breakpoint") if val.as_array().map(|a| a.len()) != Some(BREAKPOINTS.len()) => {
                        e.push(format!("{oc}: by breakpoint, its values are {} numbers (Lv.1, 20, 20+, 40, 40+, … 90)", BREAKPOINTS.len()))
                    }
                    _ => {}
                }
                payloads.push(Some((val, tols)));
            }
            Err(x) => {
                e.push(format!("{oc}: {x}"));
                payloads.push(None);
            }
        }
    }
    // observations agree at display precision
    for (ri, ro) in v.obs.iter().enumerate() {
        let Some((rv, rt)) = &payloads[ri] else { continue };
        if ro.shown.is_none() {
            continue;
        }
        for (oi, o) in v.obs.iter().enumerate() {
            let Some((ov, _)) = &payloads[oi] else { continue };
            if oi == ri || o.shown.is_some() || ro.differs.is_some() || o.differs.is_some() {
                continue;
            }
            let (rs, os) = (nums(rv), nums(ov));
            let offset = if v.by.as_deref() == Some("talent_level") { ro.first as isize - o.first as isize } else { 0 };
            for (k, (x, tol)) in rs.iter().zip(rt).enumerate() {
                let j = k as isize + offset;
                if j < 0 || j as usize >= os.len() {
                    continue;
                }
                let y = os[j as usize];
                if (y - x).abs() > tol * (1.0 + 1e-9) + 1e-12 {
                    let at = if v.by.as_deref() == Some("talent_level") { format!(" at Lv.{}", ro.first + k) } else { String::new() };
                    e.push(format!(
                        "{}: value `{}`{at}: {} gives {y}, outside {} shown as {x} (±{tol}); explain it with `differs` or correct it",
                        r.path,
                        v.key,
                        o.evidence.name(),
                        ro.evidence.name()
                    ));
                }
            }
        }
    }
}

fn nums(v: &Value) -> Vec<f64> {
    let mut o = Vec::new();
    convert::leaves(v, &mut |x| o.extend(x.as_f64()));
    o
}

fn leaves_ok(v: &Value, unit: &str, kind: UnitKind, cat: &Catalog, c: &str, e: &mut Vec<String>) {
    if kind == UnitKind::Promo {
        let ok = v.as_array().is_some_and(|a| {
            a.iter().all(|p| p.as_table().is_some_and(|t| t.get("level").and_then(Value::as_int).is_some() && t.entries.iter().all(|(k, x)| k == "level" || (record::stat_index(k).is_some() && x.as_f64().is_some()))))
        });
        if !ok {
            e.push(format!("{c}: stat bonuses are [{{ level = 20 }}, {{ level = 40, base_hp = …, <stat> = … }}, …] with stats from {}", record::STATS.map(|(n, _)| n).join(", ")));
        }
        return;
    }
    let mut bad = Vec::new();
    convert::leaves(v, &mut |x| {
        let ok = match (kind, x) {
            (UnitKind::Num { min, max, .. }, Value::Int(i)) => (*i as f64) >= min && (*i as f64) <= max,
            (UnitKind::Num { min, max, int: false }, Value::Float(f)) => *f >= min && *f <= max,
            (UnitKind::Name(set), Value::Str(s)) => set.is_empty() || set.contains(&s.as_str()),
            (UnitKind::CatalogName, Value::Str(s)) => if unit == "icd_tag" { cat.icd_tags.contains(s) } else { cat.icd_groups.contains(s) },
            (UnitKind::Bool, Value::Bool(_)) | (UnitKind::Any, _) => true,
            _ => false,
        };
        let ok = ok && (unit != "curve" || x.as_int().is_some_and(|i| cat.curves.contains_key(&(i as u32))));
        if !ok {
            bad.push(x.to_string());
        }
    });
    if !bad.is_empty() {
        let what = match kind {
            UnitKind::Num { min, max, int } => format!("{}{min} to {max}", if int { "integers " } else { "" }),
            UnitKind::Name(set) if !set.is_empty() => format!("one of {}", set.join(", ")),
            UnitKind::CatalogName => format!("a name of the engine's {} (icd.rs, lowercase, without the prefix)", if unit == "icd_tag" { "ICD tags" } else { "ICD groups" }),
            UnitKind::Bool => "true or false".into(),
            _ => "strings".into(),
        };
        let what = if unit == "curve" { format!("{what}, and a curve of the engine's CURVES") } else { what };
        e.push(format!("{c}: unit {unit} takes {what}, not {}", bad.join(", ")));
    }
}

/// The base stats a row gives at a level and ascension phase.
pub fn base_stat(row: &crate::rustdata::RVal, stat: &str, level: u32, phase: usize, cat: &Catalog) -> Option<f64> {
    let promo = row.field("promo")?.items()?.get(phase)?.items()?.get(1)?.items()?.to_vec();
    let idx = record::stat_index(stat)?;
    let bonus: f64 = promo.iter().filter_map(|t| t.items()).filter(|t| t[0].num() == Some(idx as f64)).filter_map(|t| t[1].num()).sum();
    if stat == "em" {
        return Some(row.field("em")?.num()? + bonus);
    }
    let i = ["base_hp", "base_atk", "base_def"].iter().position(|s| *s == stat)?;
    let base = row.field("base")?.items()?[i].num()?;
    let curve = row.field("curves")?.items()?[i].num()? as u32;
    Some(base * cat.curves.get(&curve)?.get(level as usize - 1)? + bonus)
}

fn breakpoints(r: &Record, row: &crate::rustdata::RVal, sel: Select, cat: &Catalog, e: &mut Vec<String>) {
    for v in r.values.iter().filter(|v| v.by.as_deref() == Some("breakpoint")) {
        let Some(stat) = v.stat.as_deref() else { continue };
        for o in v.obs.iter().filter(|o| o.shown.is_some()) {
            let Ok((val, tols)) = convert::payload(v, o) else { continue };
            for (k, (x, tol)) in nums(&val).iter().zip(&tols).enumerate() {
                let Some(&(lv, ph)) = BREAKPOINTS.get(k) else { continue };
                if let Some(y) = base_stat(row, stat, lv, ph, cat)
                    && (y - x).abs() > tol * (1.0 + 1e-9) + 1e-9 && o.differs.is_none() {
                        e.push(format!("{}: value `{}`: under selection {}, {stat} at Lv.{lv} (phase {ph}) is {y:.4}, outside the shown {x} (±{tol})", r.path, v.key, sel.name()));
                    }
            }
        }
    }
}

/// How far a record's values stand from independent evidence, under the
/// independent selection: `docs/independence.md` §7.2–7.3.
#[derive(Debug, Default)]
pub struct Status {
    /// `independent`, `provisional` or `high_uncertainty`.
    pub status: &'static str,
    /// Values the engine uses that only legacy data gives.
    pub legacy_fallback: Vec<String>,
    /// Values taken from a statement or an estimate.
    pub estimated: Vec<String>,
    pub unresolved: Vec<String>,
    /// Values whose plausible alternative moves DPS by more than 5%, without
    /// independent evidence.
    pub high: Vec<String>,
}

pub const SENSITIVITY_LIMIT: f64 = 5.0;

pub fn status(r: &Record) -> Result<Status, Vec<String>> {
    let out = convert::convert(r, Select::Independent)?;
    let mut s = Status::default();
    for (vi, res) in &out.resolved {
        let v = &r.values[*vi];
        if v.check {
            continue;
        }
        let worst = res.choices.iter().map(|c| c.evidence).max_by_key(|e| Select::Independent.rank(*e)).unwrap_or(Evidence::Official);
        let levels: Vec<String> = res.choices.iter().filter(|c| c.evidence == Evidence::LegacyGcsim).filter_map(|c| c.level).map(|l| l.to_string()).collect();
        match worst {
            Evidence::LegacyGcsim => s.legacy_fallback.push(if levels.is_empty() || levels.len() == res.choices.len() { v.key.clone() } else { format!("{} (Lv.{})", v.key, levels.join(", ")) }),
            Evidence::Stated | Evidence::Estimated => s.estimated.push(format!("{} ({})", v.key, worst.name())),
            _ => {}
        }
        if let Some(u) = &v.unresolved {
            s.unresolved.push(format!("{}: {u}", v.key));
        }
        if !worst.is_independent_evidence() && worst != Evidence::Assigned {
            for x in &v.sensitivity {
                if x.team.abs() > SENSITIVITY_LIMIT || x.char.abs() > SENSITIVITY_LIMIT {
                    s.high.push(format!("{}: {} (team {:+}%, character {:+}%; {})", v.key, x.alternative, x.team, x.char, x.basis));
                }
            }
        }
    }
    s.status = if !s.high.is_empty() {
        "high_uncertainty"
    } else if !(s.legacy_fallback.is_empty() && s.estimated.is_empty() && s.unresolved.is_empty()) {
        "provisional"
    } else {
        "independent"
    };
    Ok(s)
}
