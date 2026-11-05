//! A content record: a character's values, each with the observations that
//! give it and their provenance. The format is `docs/content-format.md`.

use crate::toml::{Table, Value};

pub const FORMAT: i64 = 1;
/// Talent levels a table holds (`engine/src/profile.rs` reaches 1 to 15).
pub const TALENT_LEVELS: usize = 15;
/// The ascension breakpoints the official pages show, as (level, phase).
pub const BREAKPOINTS: [(u32, usize); 14] = [(1, 0), (20, 0), (20, 1), (40, 1), (40, 2), (50, 2), (50, 3), (60, 3), (60, 4), (70, 4), (70, 5), (80, 5), (80, 6), (90, 6)];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Evidence {
    /// An exact value from an official source (HoYoWiki's tables, the
    /// in-game description).
    Official,
    /// A value an official source shows at display precision; it carries
    /// `shown`.
    OfficialRounded,
    /// Computed from other values: `formula`, `inputs`.
    Derived,
    /// A public test with its evidence (video, sheet).
    Measured,
    /// A community statement without linked evidence.
    Stated,
    /// An estimate: `basis`, and `analogy` when it is taken from another
    /// character.
    Estimated,
    /// Kept from the gcsim-generated data under the legacy exception
    /// (`docs/independence.md` §4, "Existing data").
    LegacyGcsim,
    /// A name the engine chose (a mod key): not a game value.
    Assigned,
}

impl Evidence {
    pub const ALL: [(&'static str, Evidence); 8] = [
        ("official", Evidence::Official),
        ("official_rounded", Evidence::OfficialRounded),
        ("derived", Evidence::Derived),
        ("measured", Evidence::Measured),
        ("stated", Evidence::Stated),
        ("estimated", Evidence::Estimated),
        ("legacy_gcsim", Evidence::LegacyGcsim),
        ("assigned", Evidence::Assigned),
    ];

    pub fn parse(s: &str) -> Option<Evidence> {
        Evidence::ALL.iter().find(|(n, _)| *n == s).map(|(_, e)| *e)
    }

    pub fn name(self) -> &'static str {
        Evidence::ALL.iter().find(|(_, e)| *e == self).unwrap().0
    }

    /// Whether it observes the game (or states it officially), as opposed to
    /// a statement, an estimate or legacy data.
    pub fn is_independent_evidence(self) -> bool {
        matches!(self, Evidence::Official | Evidence::OfficialRounded | Evidence::Derived | Evidence::Measured)
    }
}

/// Which observation the converter takes for each value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Select {
    /// Legacy values first: the engine's data as it is, for content that
    /// existed before independence.
    Legacy,
    /// Independent evidence first; legacy only where nothing else covers a
    /// value, and reported.
    Independent,
}

impl Select {
    pub fn parse(s: &str) -> Option<Select> {
        match s {
            "legacy" => Some(Select::Legacy),
            "independent" => Some(Select::Independent),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Select::Legacy => "legacy",
            Select::Independent => "independent",
        }
    }

    /// The rank of an evidence kind; lower is taken first.
    pub fn rank(self, e: Evidence) -> usize {
        use Evidence::*;
        let order: &[Evidence] = match self {
            Select::Legacy => &[LegacyGcsim, Assigned, Official, Measured, Derived, OfficialRounded, Stated, Estimated],
            Select::Independent => &[Official, Measured, Derived, OfficialRounded, Stated, Estimated, Assigned, LegacyGcsim],
        };
        order.iter().position(|x| *x == e).unwrap()
    }
}

/// What a unit's values are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UnitKind {
    /// A number in [min, max]; `int` when it must be whole.
    Num { min: f64, max: f64, int: bool },
    /// A string from a closed set, or any string when the set is empty.
    Name(&'static [&'static str]),
    /// A string checked against a catalog (ICD tags and groups).
    CatalogName,
    /// Ascension stat bonuses (`char.promo`).
    Promo,
    Bool,
    /// `unclassified`: any value; for drafts `gsc import` could not classify,
    /// with legacy observations only.
    Any,
}

pub const ELEMENTS: [&str; 11] = ["none", "pyro", "hydro", "dendro", "electro", "cryo", "anemo", "geo", "physical", "frozen", "quicken"];
pub const WEAPON_CLASSES: [&str; 5] = ["sword", "claymore", "polearm", "bow", "catalyst"];
pub const BODIES: [&str; 5] = ["boy", "girl", "male", "lady", "loli"];
pub const ZONES: [&str; 10] = ["unknown", "mondstadt", "liyue", "inazuma", "sumeru", "fontaine", "natlan", "nodkrai", "snezhnaya", "nodkraizibai"];
pub const STRIKES: [&str; 5] = ["default", "slash", "blunt", "pierce", "spear"];

/// The engine's stat indices (`engine/src/types.rs` `st`, gcsim's
/// attributes.Stat), by the names records use for them.
pub const STATS: [(&str, usize); 24] = [
    ("def%", 1),
    ("def", 2),
    ("hp", 3),
    ("hp%", 4),
    ("atk", 5),
    ("atk%", 6),
    ("er", 7),
    ("em", 8),
    ("cr", 9),
    ("cd", 10),
    ("heal", 11),
    ("pyro%", 12),
    ("hydro%", 13),
    ("cryo%", 14),
    ("electro%", 15),
    ("anemo%", 16),
    ("geo%", 17),
    ("dendro%", 18),
    ("phys%", 19),
    ("atkspd", 20),
    ("dmg%", 21),
    ("base_hp", 23),
    ("base_atk", 24),
    ("base_def", 25),
];

pub fn stat_index(name: &str) -> Option<usize> {
    STATS.iter().find(|(n, _)| *n == name).map(|(_, i)| *i)
}

pub fn stat_name(i: usize) -> Option<&'static str> {
    STATS.iter().find(|(_, x)| *x == i).map(|(n, _)| *n)
}

pub const UNITS: [(&str, UnitKind, &str); 29] = {
    use UnitKind::*;
    [
        ("atk_frac", Num { min: 0.0, max: 100.0, int: false }, "a multiplier of ATK (1.0 = 100%)"),
        ("hp_frac", Num { min: 0.0, max: 100.0, int: false }, "a multiplier of max HP"),
        ("def_frac", Num { min: 0.0, max: 100.0, int: false }, "a multiplier of DEF"),
        ("em_frac", Num { min: 0.0, max: 100.0, int: false }, "a multiplier of Elemental Mastery"),
        ("frac", Num { min: -10.0, max: 10.0, int: false }, "a fraction (0.1 = 10%): a buff, a resistance change"),
        ("frames", Num { min: 0.0, max: 100_000.0, int: true }, "frames at 60 fps"),
        ("s", Num { min: 0.0, max: 10_000.0, int: false }, "seconds"),
        ("points", Num { min: 0.0, max: 1_000_000.0, int: false }, "stat points (base HP, ATK, DEF, EM)"),
        ("energy", Num { min: 0.0, max: 200.0, int: false }, "energy"),
        ("stamina", Num { min: 0.0, max: 200.0, int: false }, "stamina"),
        ("durability", Num { min: 0.0, max: 1000.0, int: false }, "element durability in gcsim units (25 = 1U)"),
        ("count", Num { min: 0.0, max: 10_000.0, int: true }, "a count (hits, particles, stacks)"),
        ("particles", Num { min: 0.0, max: 100.0, int: false }, "elemental particles"),
        ("m", Num { min: 0.0, max: 100.0, int: false }, "a length or radius in game metres"),
        ("deg", Num { min: 0.0, max: 360.0, int: false }, "an angle in degrees"),
        ("poise", Num { min: 0.0, max: 10_000.0, int: false }, "poise damage"),
        ("hitlag_s", Num { min: 0.0, max: 1.0, int: false }, "hitlag halt time in seconds"),
        ("factor", Num { min: 0.0, max: 1.0, int: false }, "a time scale factor (hitlag)"),
        ("rarity", Num { min: 1.0, max: 5.0, int: true }, "stars"),
        ("curve", Num { min: 1.0, max: 100_000.0, int: true }, "a growth curve id of the engine's CURVES"),
        ("constellation", Num { min: -1.0, max: 6.0, int: true }, "a constellation, -1 for none"),
        ("level", Num { min: 1.0, max: 100.0, int: true }, "a character or talent level"),
        ("id", Name(&[]), "an engine identifier (a mod or status key)"),
        ("text", Name(&[]), "a description or condition, as the source words it"),
        ("element", Name(&ELEMENTS), "an element"),
        ("weapon_class", Name(&WEAPON_CLASSES), "a weapon class"),
        ("body", Name(&BODIES), "a body type"),
        ("zone", Name(&ZONES), "a region"),
        ("strike", Name(&STRIKES), "a strike type"),
    ]
};

/// Units outside the table above, which need a catalog or a structure.
pub fn unit_kind(u: &str) -> Option<UnitKind> {
    match u {
        "icd_tag" | "icd_group" => Some(UnitKind::CatalogName),
        "stat_bonus" => Some(UnitKind::Promo),
        "bool" => Some(UnitKind::Bool),
        "unclassified" => Some(UnitKind::Any),
        _ => UNITS.iter().find(|(n, _, _)| *n == u).map(|(_, k, _)| *k),
    }
}

#[derive(Clone, Debug)]
pub struct Source {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub url: Option<String>,
    pub video: Option<String>,
    pub observed: Option<String>,
    pub game_version: Option<String>,
    pub line: usize,
}

/// Where a source stands. `project` is a document of this repository (the
/// standard estimates of `docs/independence.md`); it and `legacy_gcsim` are
/// given by a repository path, the others by a URL.
pub const SOURCE_KINDS: [&str; 8] = ["official_page", "official_text", "video", "sheet", "article", "community", "project", "legacy_gcsim"];

/// A number an observation gives, and how far the true value may be from
/// it (half the last digit shown; 0 when exact).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Num {
    pub v: f64,
    pub tol: f64,
}

#[derive(Clone, Debug)]
pub struct Obs {
    pub evidence: Evidence,
    pub source: Option<String>,
    /// Where in the source: a section, a table, a video timestamp.
    pub at: Option<String>,
    pub observed: Option<String>,
    pub game_version: Option<String>,
    /// The values, in the value's unit.
    pub data: Option<Value>,
    /// The values as the source shows them (`official_rounded`).
    pub shown: Option<Value>,
    /// The talent level of the first element (`by = "talent_level"`).
    pub first: usize,
    pub formula: Option<String>,
    pub inputs: Vec<String>,
    pub basis: Option<String>,
    pub analogy: Option<String>,
    pub tolerance: Option<f64>,
    /// Why it differs from another observation beyond display rounding.
    pub differs: Option<String>,
    pub note: Option<String>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct Sensitivity {
    pub alternative: String,
    pub team: f64,
    pub char: f64,
    pub basis: String,
}

#[derive(Clone, Debug)]
pub struct Val {
    pub key: String,
    pub unit: String,
    pub layer: i64,
    pub target: Option<String>,
    /// Where the engine's logic holds it, when no table does.
    pub engine: Option<String>,
    /// A value kept only to check others (base stats at breakpoints).
    pub check: bool,
    pub by: Option<String>,
    /// For `by = "breakpoint"`: the stat it checks.
    pub stat: Option<String>,
    pub field: Option<String>,
    pub unresolved: Option<String>,
    pub obs: Vec<Obs>,
    pub sensitivity: Vec<Sensitivity>,
    pub note: Option<String>,
    pub line: usize,
}

pub const FIELDS: [&str; 17] = [
    "frames", "hitmark", "icd_tag", "icd_group", "durability", "hitlag", "strike", "poise", "hitbox", "particles", "snapshot", "geometry", "count", "timing", "energy", "cooldown", "duration",
];

#[derive(Clone, Debug)]
pub struct Record {
    pub path: String,
    pub format: i64,
    pub kind: String,
    pub key: String,
    pub name: String,
    pub module: String,
    pub select: Select,
    pub sources: Vec<Source>,
    /// Each item's name, type and, for a slice, its length or the lengths
    /// of its rows.
    pub items: Vec<(String, String, Option<Value>)>,
    pub values: Vec<Val>,
}

impl Record {
    pub fn source(&self, id: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.id == id)
    }
    pub fn value(&self, key: &str) -> Option<&Val> {
        self.values.iter().find(|v| v.key == key)
    }
}

fn s(t: &Table, k: &str) -> Result<Option<String>, String> {
    match t.get(k) {
        None => Ok(None),
        Some(Value::Str(v)) => Ok(Some(v.clone())),
        Some(v) => Err(format!("line {}: `{k}` must be a string, not a {}", t.line_of(k), v.type_name())),
    }
}

fn req(t: &Table, k: &str, ctx: &str) -> Result<String, String> {
    s(t, k)?.ok_or_else(|| format!("{ctx}: `{k}` is required"))
}

fn known(t: &Table, allowed: &[&str], ctx: &str) -> Result<(), String> {
    for k in t.keys() {
        if !allowed.contains(&k) {
            return Err(format!("line {}: {ctx}: unknown field `{k}`", t.line_of(k)));
        }
    }
    Ok(())
}

/// Reads a record; structure only. `validate` checks the rest.
pub fn parse(path: &str, src: &str) -> Result<Record, Vec<String>> {
    let t = crate::toml::parse(src).map_err(|e| vec![format!("{path}: {e}")])?;
    from_table(path, &t).map_err(|e| vec![format!("{path}: {e}")])
}

fn from_table(path: &str, t: &Table) -> Result<Record, String> {
    known(t, &["format", "kind", "key", "name", "module", "select", "source", "items", "value"], "record")?;
    let format = t.get("format").and_then(Value::as_int).ok_or("`format` is required (an integer)")?;
    if format != FORMAT {
        return Err(format!("format {format} is not {FORMAT}"));
    }
    let select_s = req(t, "select", "record")?;
    let select = Select::parse(&select_s).ok_or_else(|| format!("select `{select_s}` is not legacy or independent"))?;
    let mut r = Record {
        path: path.to_string(),
        format,
        kind: req(t, "kind", "record")?,
        key: req(t, "key", "record")?,
        name: req(t, "name", "record")?,
        module: req(t, "module", "record")?,
        select,
        sources: Vec::new(),
        items: Vec::new(),
        values: Vec::new(),
    };
    if let Some(v) = t.get("source") {
        let st = v.as_table().ok_or("`source` must be tables: [source.<id>]")?;
        for (id, sv) in &st.entries {
            let line = st.line_of(id);
            let x = sv.as_table().ok_or_else(|| format!("line {line}: source `{id}` must be a table"))?;
            let ctx = format!("source `{id}`");
            known(x, &["kind", "title", "url", "video", "observed", "game_version", "note"], &ctx)?;
            r.sources.push(Source {
                id: id.clone(),
                kind: req(x, "kind", &ctx)?,
                title: req(x, "title", &ctx)?,
                url: s(x, "url")?,
                video: s(x, "video")?,
                observed: s(x, "observed")?,
                game_version: s(x, "game_version")?,
                line,
            });
        }
    }
    if let Some(v) = t.get("items") {
        let it = v.as_table().ok_or("`items` must be a table")?;
        for (name, decl) in &it.entries {
            let bad = || format!("line {}: item `{name}` is \"<type>\" or {{ type = \"<type>\", len = <n or [n, ...]> }}", it.line_of(name));
            let (ty, len) = match decl {
                Value::Str(t) => (t.clone(), None),
                Value::Table(t) => {
                    known(t, &["type", "len"], &format!("item `{name}`"))?;
                    (t.get("type").and_then(Value::as_str).ok_or_else(bad)?.to_string(), t.get("len").cloned())
                }
                _ => return Err(bad()),
            };
            r.items.push((name.clone(), ty, len));
        }
    }
    for vv in t.get("value").and_then(Value::as_array).unwrap_or(&[]) {
        let x = vv.as_table().ok_or("`value` must be [[value]] tables")?;
        let line = x.lines.first().copied().unwrap_or(0);
        let key = req(x, "key", &format!("line {line}: value"))?;
        let ctx = format!("value `{key}` (line {line})");
        known(x, &["key", "unit", "layer", "target", "engine", "check", "by", "stat", "field", "unresolved", "obs", "sensitivity", "note"], &ctx)?;
        let mut v = Val {
            unit: req(x, "unit", &ctx)?,
            layer: x.get("layer").and_then(Value::as_int).ok_or_else(|| format!("{ctx}: `layer` is required (0, 1 or 2)"))?,
            target: s(x, "target")?,
            engine: s(x, "engine")?,
            check: matches!(x.get("check"), Some(Value::Bool(true))),
            by: s(x, "by")?,
            stat: s(x, "stat")?,
            field: s(x, "field")?,
            unresolved: s(x, "unresolved")?,
            obs: Vec::new(),
            sensitivity: Vec::new(),
            note: s(x, "note")?,
            key,
            line,
        };
        for ov in x.get("obs").and_then(Value::as_array).unwrap_or(&[]) {
            let o = ov.as_table().ok_or_else(|| format!("{ctx}: `obs` must be [[value.obs]] tables"))?;
            let oline = o.lines.first().copied().unwrap_or(0);
            let octx = format!("value `{}`, observation at line {oline}", v.key);
            known(o, &["evidence", "source", "at", "observed", "game_version", "data", "shown", "first", "formula", "inputs", "basis", "analogy", "tolerance", "differs", "note"], &octx)?;
            let ev = req(o, "evidence", &octx)?;
            let evidence = Evidence::parse(&ev).ok_or_else(|| format!("{octx}: evidence `{ev}` is not one of {}", Evidence::ALL.map(|(n, _)| n).join(", ")))?;
            let inputs = match o.get("inputs") {
                None => Vec::new(),
                Some(Value::Array(a)) => a.iter().map(|i| i.as_str().map(String::from).ok_or_else(|| format!("{octx}: `inputs` must be strings"))).collect::<Result<_, _>>()?,
                Some(_) => return Err(format!("{octx}: `inputs` must be an array of strings")),
            };
            v.obs.push(Obs {
                evidence,
                source: s(o, "source")?,
                at: s(o, "at")?,
                observed: s(o, "observed")?,
                game_version: s(o, "game_version")?,
                data: o.get("data").cloned(),
                shown: o.get("shown").cloned(),
                first: o.get("first").and_then(Value::as_int).unwrap_or(1).max(1) as usize,
                formula: s(o, "formula")?,
                inputs,
                basis: s(o, "basis")?,
                analogy: s(o, "analogy")?,
                tolerance: o.get("tolerance").and_then(Value::as_f64),
                differs: s(o, "differs")?,
                note: s(o, "note")?,
                line: oline,
            });
        }
        for sv in x.get("sensitivity").and_then(Value::as_array).unwrap_or(&[]) {
            let o = sv.as_table().ok_or_else(|| format!("{ctx}: `sensitivity` must be [[value.sensitivity]] tables"))?;
            let sctx = format!("value `{}`, sensitivity", v.key);
            known(o, &["alternative", "team", "char", "basis"], &sctx)?;
            v.sensitivity.push(Sensitivity {
                alternative: req(o, "alternative", &sctx)?,
                team: o.get("team").and_then(Value::as_f64).ok_or_else(|| format!("{sctx}: `team` (percent) is required"))?,
                char: o.get("char").and_then(Value::as_f64).ok_or_else(|| format!("{sctx}: `char` (percent) is required"))?,
                basis: req(o, "basis", &sctx)?,
            });
        }
        r.values.push(v);
    }
    Ok(r)
}

/// A decimal string with its point moved `shift` places left, exactly.
fn shift_left(t: &str, shift: usize) -> String {
    let (neg, t) = t.strip_prefix('-').map_or((false, t), |r| (true, r));
    let (int, frac) = t.split_once('.').unwrap_or((t, ""));
    let mut digits = format!("{int}{frac}");
    let mut point = int.len() as isize - shift as isize;
    while point <= 0 {
        digits.insert(0, '0');
        point += 1;
    }
    let (a, b) = digits.split_at(point as usize);
    let a = a.trim_start_matches('0');
    format!("{}{}.{}", if neg { "-" } else { "" }, if a.is_empty() { "0" } else { a }, if b.is_empty() { "0" } else { b })
}

/// Reads a shown value: `"42.1%"`, `"12.0s"`, `"1U"`, `912` or `"912"`.
pub fn parse_shown(v: &Value, unit: &str) -> Result<Num, String> {
    let text = match v {
        Value::Str(s) => s.trim().to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => crate::toml::float(*f),
        _ => return Err(format!("a shown value must be a string or number, not a {}", v.type_name())),
    };
    let (num, scale_shift, mult) = if let Some(n) = text.strip_suffix('%') {
        (n, 2, 1.0)
    } else if let Some(n) = text.strip_suffix('s') {
        if unit != "s" {
            return Err(format!("`{text}` is in seconds, the unit is {unit}"));
        }
        (n, 0, 1.0)
    } else if let Some(n) = text.strip_suffix('U') {
        if unit != "durability" {
            return Err(format!("`{text}` is in gauge units, the unit is {unit}"));
        }
        (n, 0, 25.0)
    } else {
        (text.as_str(), 0, 1.0)
    };
    if num.is_empty() || !num.trim_start_matches('-').bytes().all(|c| c.is_ascii_digit() || c == b'.') || num.matches('.').count() > 1 {
        return Err(format!("`{text}` is not a shown number"));
    }
    if scale_shift > 0 && !matches!(unit, "atk_frac" | "hp_frac" | "def_frac" | "em_frac" | "frac") {
        return Err(format!("`{text}` is a percentage, the unit is {unit}"));
    }
    let decimals = num.split_once('.').map_or(0, |(_, f)| f.len()) + scale_shift;
    let exact = shift_left(num, scale_shift);
    let v: f64 = exact.parse().map_err(|_| format!("`{text}` is not a number"))?;
    let tol = 0.5 * 10f64.powi(-(decimals as i32)) * mult;
    Ok(Num { v: v * mult, tol })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_shown_values() {
        let n = parse_shown(&Value::Str("42.1%".into()), "atk_frac").unwrap();
        assert_eq!(n.v, 0.421);
        assert!((n.tol - 0.0005).abs() < 1e-15);
        assert_eq!(parse_shown(&Value::Str("103%".into()), "atk_frac").unwrap().v, 1.03);
        assert_eq!(parse_shown(&Value::Str("5%".into()), "frac").unwrap().v, 0.05);
        assert_eq!(parse_shown(&Value::Int(912), "points").unwrap(), Num { v: 912.0, tol: 0.5 });
        assert_eq!(parse_shown(&Value::Str("12.0s".into()), "s").unwrap().v, 12.0);
        assert_eq!(parse_shown(&Value::Str("1U".into()), "durability").unwrap().v, 25.0);
        assert!(parse_shown(&Value::Str("42%".into()), "frames").is_err());
        assert!(parse_shown(&Value::Str("abc".into()), "points").is_err());
    }
}
