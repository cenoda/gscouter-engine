//! From a record to the engine's data: the observation each value takes,
//! the items of its tables module, and its row of `data.rs`.

use crate::record::{self, Evidence, Num, Obs, Record, Select, Val, TALENT_LEVELS};
use crate::rustdata::{self, Item, RVal, Ty, ACTIONS, N_ACTIONS};
use crate::toml::Value;

/// The observation a value (or one talent level of it) takes.
#[derive(Clone, Debug)]
pub struct Choice {
    /// The talent level, for `by = "talent_level"`.
    pub level: Option<usize>,
    pub obs: usize,
    pub evidence: Evidence,
    /// How far the true value may be from the one taken.
    pub tol: f64,
}

#[derive(Clone, Debug)]
pub struct Resolved {
    pub value: Value,
    pub choices: Vec<Choice>,
}

/// An observation's values in the value's unit, and each leaf's tolerance
/// in order.
pub fn payload(v: &Val, o: &Obs) -> Result<(Value, Vec<f64>), String> {
    let mut tols = Vec::new();
    let val = match (&o.data, &o.shown) {
        (Some(d), None) => {
            leaves(d, &mut |_| tols.push(o.tolerance.unwrap_or(0.0)));
            d.clone()
        }
        (None, Some(s)) => map_leaves(s, &mut |x| {
            let n: Num = record::parse_shown(x, &v.unit)?;
            tols.push(n.tol);
            Ok(if is_int_unit(&v.unit) && n.v.fract() == 0.0 { Value::Int(n.v as i64) } else { Value::Float(n.v) })
        })?,
        (Some(_), Some(_)) => return Err("an observation has `data` or `shown`, not both".into()),
        (None, None) => return Err("an observation needs `data` or `shown`".into()),
    };
    Ok((val, tols))
}

fn is_int_unit(u: &str) -> bool {
    matches!(record::unit_kind(u), Some(record::UnitKind::Num { int: true, .. }))
}

pub fn leaves(v: &Value, f: &mut dyn FnMut(&Value)) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| leaves(x, f)),
        Value::Table(t) => t.entries.iter().for_each(|(_, x)| leaves(x, f)),
        x => f(x),
    }
}

fn map_leaves(v: &Value, f: &mut dyn FnMut(&Value) -> Result<Value, String>) -> Result<Value, String> {
    Ok(match v {
        Value::Array(a) => Value::Array(a.iter().map(|x| map_leaves(x, f)).collect::<Result<_, _>>()?),
        x => f(x)?,
    })
}

/// The value a record gives under a selection.
pub fn resolve(v: &Val, sel: Select) -> Result<Resolved, String> {
    let ctx = |e: String| format!("value `{}`: {e}", v.key);
    let mut payloads = Vec::new();
    for o in &v.obs {
        payloads.push(payload(v, o).map_err(ctx)?);
    }
    let best = |cands: &mut dyn Iterator<Item = usize>| cands.min_by_key(|&i| (sel.rank(v.obs[i].evidence), i));
    if v.by.as_deref() == Some("talent_level") {
        let mut out = Vec::new();
        let mut choices = Vec::new();
        for lv in 1..=TALENT_LEVELS {
            let covering = (0..v.obs.len()).filter(|&i| {
                let len = payloads[i].0.as_array().map_or(0, |a| a.len());
                lv >= v.obs[i].first && lv < v.obs[i].first + len
            });
            let Some(i) = best(&mut covering.collect::<Vec<_>>().into_iter()) else { break };
            let k = lv - v.obs[i].first;
            out.push(payloads[i].0.as_array().unwrap()[k].clone());
            choices.push(Choice { level: Some(lv), obs: i, evidence: v.obs[i].evidence, tol: payloads[i].1[k] });
        }
        return Ok(Resolved { value: Value::Array(out), choices });
    }
    let i = best(&mut (0..v.obs.len())).ok_or_else(|| ctx("it has no observation".into()))?;
    let tol = payloads[i].1.iter().copied().fold(0.0, f64::max);
    Ok(Resolved { value: payloads[i].0.clone(), choices: vec![Choice { level: None, obs: i, evidence: v.obs[i].evidence, tol }] })
}

/// A record value as an engine literal of a type.
pub fn to_rval(v: &Value, ty: &Ty) -> Result<RVal, String> {
    Ok(match ty {
        Ty::F64 => RVal::F(v.as_f64().ok_or_else(|| format!("`{v}` is not a number"))?),
        Ty::I32 => match v {
            Value::Int(i) => RVal::I(*i),
            Value::Float(f) if f.fract() == 0.0 => RVal::I(*f as i64),
            _ => return Err(format!("`{v}` is not an integer")),
        },
        Ty::Str => RVal::S(v.as_str().ok_or_else(|| format!("`{v}` is not a string"))?.to_string()),
        Ty::Bool => match v {
            Value::Bool(b) => RVal::B(*b),
            _ => return Err(format!("`{v}` is not a boolean")),
        },
        Ty::Frames => RVal::Arr(frame_table(v)?.into_iter().map(|x| RVal::I(x as i64)).collect()),
        Ty::Slice(t) => RVal::Slice(v.as_array().ok_or_else(|| format!("`{v}` is not an array"))?.iter().map(|x| to_rval(x, t)).collect::<Result<_, _>>()?),
    })
}

/// A frame table from `{ length = 20, skill = 12, ... }`: every action
/// cancels at `length` unless named.
pub fn frame_table(v: &Value) -> Result<[i32; N_ACTIONS], String> {
    let t = v.as_table().ok_or_else(|| format!("a frame table is `{{ length = n, <action> = n, ... }}`, not `{v}`"))?;
    let num = |x: &Value| x.as_int().and_then(|i| i32::try_from(i).ok()).ok_or_else(|| format!("frames must be integers, not `{x}`"));
    let len = num(t.get("length").ok_or("a frame table needs `length`")?)?;
    let mut f = [len; N_ACTIONS];
    for (k, x) in &t.entries {
        let i = ACTIONS.iter().position(|a| a == k).ok_or_else(|| format!("`{k}` is not an action ({})", ACTIONS.join(", ")))?;
        f[i] = num(x)?;
    }
    Ok(f)
}

/// A target: an item and indices into it, or a `char.` field.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Item(String, Vec<usize>),
    Char(String),
}

pub const CHAR_FIELDS: [&str; 13] = ["base", "curves", "em", "promo", "element", "weapon_class", "body", "zone", "rarity", "energy_max", "normal_hit_num", "cons", "anim_delay"];

pub fn parse_target(s: &str) -> Result<Target, String> {
    if let Some(f) = s.strip_prefix("char.") {
        return if CHAR_FIELDS.contains(&f) { Ok(Target::Char(f.into())) } else { Err(format!("`char.{f}` is not a field of CharData ({})", CHAR_FIELDS.join(", "))) };
    }
    let (name, mut rest) = s.split_once('[').map_or((s, ""), |(n, r)| (n, r));
    if name.is_empty() || !name.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_') {
        return Err(format!("`{s}` is not ITEM, ITEM[i][j]… or char.<field>"));
    }
    let mut idx = Vec::new();
    while !rest.is_empty() {
        let (n, r) = rest.split_once(']').ok_or_else(|| format!("`{s}`: unclosed `[`"))?;
        idx.push(n.parse().map_err(|_| format!("`{s}`: `{n}` is not an index"))?);
        rest = r.strip_prefix('[').unwrap_or(r);
        if !r.is_empty() && !r.starts_with('[') {
            return Err(format!("`{s}`: text after an index"));
        }
    }
    Ok(Target::Item(name.into(), idx))
}

enum Tree {
    Hole,
    Set(RVal, String),
    List(Vec<Tree>),
}

impl Tree {
    fn set(&mut self, path: &[usize], v: RVal, key: &str) -> Result<(), String> {
        if path.is_empty() {
            return match self {
                Tree::Hole => {
                    *self = Tree::Set(v, key.into());
                    Ok(())
                }
                Tree::Set(_, k) => Err(format!("set by both `{k}` and `{key}`")),
                Tree::List(_) => Err(format!("`{key}` sets what other values set parts of")),
            };
        }
        if let Tree::Hole = self {
            *self = Tree::List(Vec::new());
        }
        match self {
            Tree::List(l) => {
                while l.len() <= path[0] {
                    l.push(Tree::Hole);
                }
                l[path[0]].set(&path[1..], v, key)
            }
            Tree::Set(_, k) => Err(format!("`{k}` sets all of what `{key}` sets part of")),
            Tree::Hole => unreachable!(),
        }
    }

    fn finish(self, ty: &Ty, at: &str) -> Result<RVal, String> {
        match self {
            Tree::Hole => Err(format!("{at} has no value")),
            Tree::Set(v, k) => v.check(ty).map(|_| v).map_err(|e| format!("{at} (from `{k}`): {e}")),
            Tree::List(l) => {
                let (inner, frames) = match ty {
                    Ty::Slice(t) => (t.as_ref(), false),
                    Ty::Frames => (&Ty::I32, true),
                    _ => return Err(format!("{at} is a {}, not a list", ty.name())),
                };
                if frames && l.len() != N_ACTIONS {
                    return Err(format!("{at}: a frame table has {N_ACTIONS} entries"));
                }
                let v = l.into_iter().enumerate().map(|(i, t)| t.finish(inner, &format!("{at}[{i}]"))).collect::<Result<Vec<_>, _>>()?;
                Ok(if frames { RVal::Arr(v) } else { RVal::Slice(v) })
            }
        }
    }
}

pub struct Output {
    pub items: Vec<Item>,
    pub row: Option<RVal>,
    pub resolved: Vec<(usize, Resolved)>,
}

/// Converts a record under a selection.
pub fn convert(r: &Record, sel: Select) -> Result<Output, Vec<String>> {
    let mut errs = Vec::new();
    let mut types = Vec::new();
    for (name, ty, _) in &r.items {
        match Ty::parse(ty) {
            Ok(t) => types.push((name.clone(), t)),
            Err(e) => errs.push(format!("item `{name}`: {e}")),
        }
    }
    let mut trees: Vec<Tree> = types.iter().map(|_| Tree::Hole).collect();
    let mut row: Vec<(String, RVal)> = Vec::new();
    let mut resolved = Vec::new();
    for (vi, v) in r.values.iter().enumerate() {
        let res = match resolve(v, sel) {
            Ok(x) => x,
            Err(e) => {
                errs.push(e);
                continue;
            }
        };
        if v.target.is_some() && v.by.as_deref() == Some("talent_level") && res.choices.len() < TALENT_LEVELS {
            errs.push(format!("value `{}`: levels 1 to {} only under selection {}; the engine's tables hold {TALENT_LEVELS} (derive or estimate the rest)", v.key, res.choices.len(), sel.name()));
        }
        if let Some(t) = &v.target {
            let r2 = parse_target(t).and_then(|t| match t {
                Target::Item(name, path) => {
                    let i = types.iter().position(|(n, _)| *n == name).ok_or_else(|| format!("`{name}` is not in [items]"))?;
                    let ty = types[i].1.at(path.len()).ok_or_else(|| format!("`{name}{path:?}` indexes into a {}", types[i].1.name()))?.clone();
                    let rv = to_rval(&res.value, &ty)?;
                    trees[i].set(&path, rv, &v.key)
                }
                Target::Char(f) => {
                    if row.iter().any(|(k, _)| *k == f) {
                        return Err(format!("char.{f} is set twice"));
                    }
                    row.push((f.clone(), char_field(&f, &res.value)?));
                    Ok(())
                }
            });
            if let Err(e) = r2 {
                errs.push(format!("value `{}`: {e}", v.key));
            }
        }
        resolved.push((vi, res));
    }
    let mut items = Vec::new();
    for ((name, ty), tree) in types.into_iter().zip(trees) {
        match tree.finish(&ty, &name) {
            Ok(val) => {
                let len = r.items.iter().find(|(n, _, _)| *n == name).and_then(|(_, _, l)| l.as_ref());
                if let Some(e) = len.and_then(|l| check_len(&name, &val, l)) {
                    errs.push(e);
                }
                items.push(Item { name, ty, val })
            }
            Err(e) => errs.push(e),
        }
    }
    let row = if r.kind == "character" {
        let mut fields = vec![("key".to_string(), RVal::S(r.key.clone()))];
        for f in CHAR_FIELDS {
            match row.iter().find(|(k, _)| k == f) {
                Some((_, v)) => fields.push((f.to_string(), v.clone())),
                None => errs.push(format!("no value targets char.{f}")),
            }
        }
        fields.push(("module".into(), RVal::S(r.module.clone())));
        Some(RVal::Struct("CharData".into(), fields))
    } else {
        None
    };
    if errs.is_empty() { Ok(Output { items, row, resolved }) } else { Err(errs) }
}

/// Checks an item against its declared length, or the lengths of its rows.
fn check_len(name: &str, v: &RVal, len: &Value) -> Option<String> {
    let rows = v.items()?;
    let want: Vec<Option<i64>> = match len {
        Value::Int(n) => vec![None; *n as usize],
        Value::Array(a) => a.iter().map(Value::as_int).collect(),
        _ => return None,
    };
    if rows.len() != want.len() {
        return Some(format!("{name} has {} rows, and its `len` says {}", rows.len(), want.len()));
    }
    for (i, (row, w)) in rows.iter().zip(want).enumerate() {
        if let (Some(w), Some(r)) = (w, row.items())
            && r.len() as i64 != w {
                return Some(format!("{name}[{i}] has {} entries, and its `len` says {w}", r.len()));
            }
    }
    None
}

fn name_index(v: &Value, names: &[&str], what: &str) -> Result<RVal, String> {
    let s = v.as_str().ok_or_else(|| format!("{what} is a name, not `{v}`"))?;
    names.iter().position(|n| *n == s).map(|i| RVal::I(i as i64)).ok_or_else(|| format!("`{s}` is not a {what} ({})", names.join(", ")))
}

fn named_ints(v: &Value, names: &[&str], what: &str) -> Result<RVal, String> {
    let t = v.as_table().ok_or_else(|| format!("{what} is `{{ {} }}`", names.iter().map(|n| format!("{n} = …")).collect::<Vec<_>>().join(", ")))?;
    for k in t.keys() {
        if !names.contains(&k) {
            return Err(format!("`{k}` is not one of {}", names.join(", ")));
        }
    }
    names.iter().map(|n| t.get(n).and_then(Value::as_int).map(RVal::I).ok_or_else(|| format!("{what} needs `{n}`"))).collect::<Result<_, _>>().map(RVal::Arr)
}

pub const CONS_NAMES: [&str; 3] = ["normal", "skill", "burst"];
/// gcsim's AnimationDelayKey, in order.
pub const ANIM_DELAY_NAMES: [&str; 3] = ["none", "yelan_n0", "xingqiu_n0"];

/// A `char.` field as its CharData literal.
pub fn char_field(f: &str, v: &Value) -> Result<RVal, String> {
    let nums = |n: usize, int: bool| -> Result<RVal, String> {
        let a = v.as_array().filter(|a| a.len() == n).ok_or_else(|| format!("char.{f} is {n} numbers"))?;
        a.iter().map(|x| if int { x.as_int().map(RVal::I) } else { x.as_f64().map(RVal::F) }.ok_or_else(|| format!("char.{f}: `{x}`"))).collect::<Result<_, _>>().map(RVal::Arr)
    };
    Ok(match f {
        "base" => nums(3, false)?,
        "curves" => nums(3, true)?,
        "em" | "energy_max" => RVal::F(v.as_f64().ok_or_else(|| format!("char.{f} is a number"))?),
        "rarity" | "normal_hit_num" => RVal::I(v.as_int().ok_or_else(|| format!("char.{f} is an integer"))?),
        "element" => name_index(v, &record::ELEMENTS, "element")?,
        "weapon_class" => name_index(v, &record::WEAPON_CLASSES, "weapon class")?,
        "body" => name_index(v, &record::BODIES, "body type")?,
        "zone" => name_index(v, &record::ZONES, "zone")?,
        "cons" => named_ints(v, &CONS_NAMES, "char.cons")?,
        "anim_delay" => named_ints(v, &ANIM_DELAY_NAMES, "char.anim_delay")?,
        "promo" => {
            let a = v.as_array().ok_or("char.promo is an array of { level = …, <stat> = … }")?;
            let mut out = Vec::new();
            for p in a {
                let t = p.as_table().ok_or("char.promo is an array of { level = …, <stat> = … }")?;
                let lv = t.get("level").and_then(Value::as_int).ok_or("each char.promo entry needs `level`")?;
                let mut stats = Vec::new();
                for (k, x) in &t.entries {
                    if k == "level" {
                        continue;
                    }
                    let i = record::stat_index(k).ok_or_else(|| format!("`{k}` is not a stat"))?;
                    stats.push(RVal::Tuple(vec![RVal::I(i as i64), RVal::F(x.as_f64().ok_or_else(|| format!("char.promo {k}: `{x}`"))?)]));
                }
                out.push(RVal::Tuple(vec![RVal::I(lv), RVal::Slice(stats)]));
            }
            RVal::Slice(out)
        }
        _ => return Err(format!("char.{f} is not a field")),
    })
}

/// The generated tables module.
pub fn module_text(r: &Record, sel: Select, items: &[Item]) -> String {
    format!(
        "// Generated by gsc convert (content/) from {} (selection: {}); do not edit.\n\n//! {}'s tables, converted from the content record.\n\n{}",
        r.path,
        sel.name(),
        r.name,
        rustdata::module_body(items)
    )
}

/// The generated row of `data.rs`'s CHARS, as it is indented there.
pub fn row_text(row: &RVal) -> String {
    format!("    {},", row.text())
}
