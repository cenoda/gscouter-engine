//! A draft record from a character's legacy data: every table item and its
//! `data.rs` row as `legacy_gcsim` observations. Units and layers are
//! guessed from the item names and checked against the values; what cannot
//! be guessed is `unit = "unclassified"`, which takes legacy observations
//! only. The guesses are a draft's: review them.

use crate::catalog::Catalog;
use crate::convert::{ANIM_DELAY_NAMES, CONS_NAMES};
use crate::record::{self, BODIES, ELEMENTS, TALENT_LEVELS, WEAPON_CLASSES, ZONES};
use crate::rustdata::{RVal, Ty, ACTIONS, N_ACTIONS};
use crate::toml::{float, quote};
use std::fmt::Write;

/// The unit, layer and field an item's name suggests.
fn guess(name: &str, ty: &Ty, talent: bool) -> (&'static str, i64, Option<&'static str>) {
    let has = |s: &str| name.contains(s);
    let leaf = {
        let mut t = ty;
        while let Ty::Slice(i) = t {
            t = i;
        }
        t.clone()
    };
    if leaf == Ty::Str {
        return ("id", 0, None);
    }
    if talent {
        return ("atk_frac", 1, None);
    }
    if leaf == Ty::Frames || has("FRAMES") {
        return ("frames", 2, Some("frames"));
    }
    if has("HITMARK") {
        return ("frames", 2, Some("hitmark"));
    }
    if has("HITLAG") {
        return ("hitlag_s", 2, Some("hitlag"));
    }
    if has("HITBOX") || has("RADIUS") {
        return ("m", 2, Some("hitbox"));
    }
    if has("POISE") {
        return ("poise", 2, Some("poise"));
    }
    if has("DURABILITY") {
        return ("durability", 2, Some("durability"));
    }
    if has("WINDOW") || has("DURATION") || has("DELAY") {
        return ("frames", 2, Some("timing"));
    }
    if has("HIT_NUM") || has("COUNT") {
        return ("count", 2, Some("count"));
    }
    ("unclassified", 1, None)
}

/// Whether every number of a literal fits a unit; a guess that does not is
/// left unclassified.
fn fits(v: &RVal, unit: &str) -> bool {
    match (v, record::unit_kind(unit)) {
        (_, Some(record::UnitKind::Name(_))) => matches!(v, RVal::S(_)) || v.items().is_some_and(|a| a.iter().all(|x| fits(x, unit))),
        (RVal::F(x), Some(record::UnitKind::Num { min, max, int })) => !int && *x >= min && *x <= max,
        (RVal::I(i), Some(record::UnitKind::Num { min, max, .. })) => (*i as f64) >= min && (*i as f64) <= max,
        (RVal::Slice(a) | RVal::Arr(a), _) => a.iter().all(|x| fits(x, unit)),
        _ => false,
    }
}

/// TOML for a literal of the record's data.
fn toml_of(v: &RVal, ty: &Ty) -> String {
    match (ty, v) {
        (Ty::Frames, RVal::Arr(a)) => {
            let f: Vec<i64> = a.iter().map(|x| if let RVal::I(i) = x { *i } else { 0 }).collect();
            let mut o = format!("{{ length = {}", f[0]);
            for (i, x) in f.iter().enumerate().skip(1) {
                if *x != f[0] {
                    let name = ACTIONS.get(i).copied().unwrap_or("?");
                    let _ = write!(o, ", {name} = {x}");
                }
            }
            o.push_str(" }");
            o
        }
        (Ty::Slice(t), RVal::Slice(a)) => format!("[{}]", a.iter().map(|x| toml_of(x, t)).collect::<Vec<_>>().join(", ")),
        (_, RVal::F(x)) => float(*x),
        (_, RVal::I(i)) => i.to_string(),
        (_, RVal::S(s)) => quote(s),
        (_, RVal::B(b)) => b.to_string(),
        _ => format!("\"? {}\"", v.text()),
    }
}

fn is_talent_row(v: &RVal) -> bool {
    matches!(v, RVal::Slice(a) if a.len() == TALENT_LEVELS && a.iter().all(|x| matches!(x, RVal::F(_))))
}

struct W {
    o: String,
}

impl W {
    #[allow(clippy::too_many_arguments)]
    fn value(&mut self, key: &str, unit: &str, layer: i64, target: &str, field: Option<&str>, by: Option<&str>, source: &str, data: &str) {
        let o = &mut self.o;
        let _ = writeln!(o, "\n[[value]]\nkey = {}\nunit = {}\nlayer = {layer}\ntarget = {}", quote(key), quote(unit), quote(target));
        if let Some(f) = field {
            let _ = writeln!(o, "field = {}", quote(f));
        }
        if let Some(b) = by {
            let _ = writeln!(o, "by = {}", quote(b));
        }
        let _ = writeln!(o, "\n  [[value.obs]]\n  evidence = \"legacy_gcsim\"\n  source = {}\n  data = {data}", quote(source));
    }
}

/// A draft record for a character.
pub fn import(cat: &Catalog, key: &str) -> Result<String, String> {
    let key = cat.char_key(key).ok_or_else(|| format!("`{key}` is not a character"))?.to_string();
    let (_, row) = cat.char_rows.get(&key).ok_or_else(|| format!("{key} has no row in data.rs"))?;
    let module = match row.field("module") {
        Some(RVal::S(m)) => m.clone(),
        _ => return Err("no module".into()),
    };
    // the travelers' rows name modules that have no tables of their own
    let has_module = cat.char_module_path(&module).is_file();
    let items = if has_module { cat.char_module(&module)? } else { Vec::new() };
    let rel = |p: &std::path::Path| p.strip_prefix(&cat.root).unwrap_or(p).display().to_string();
    let mut w = W { o: String::new() };
    let o = &mut w.o;
    let _ = writeln!(o, "# {key}: content record (docs/content-format.md), imported from the legacy data by `gsc import`.");
    let _ = writeln!(o, "format = 1\nkind = \"character\"\nkey = {}\nname = {}\nmodule = {}\nselect = \"legacy\"\n", quote(&key), quote(&key), quote(&module));
    if has_module {
        let _ = writeln!(o, "[source.legacy_tables]\nkind = \"legacy_gcsim\"\ntitle = \"The engine's tables module, generated by reference/gendata from cenoda/gcsim@eb21884\"\nurl = {}\ngame_version = \"gcsim eb21884 (cenoda/gcsim, reference/go.mod)\"\nobserved = \"2026-11-05\"\n", quote(&rel(&cat.char_module_path(&module))));
    }
    let _ = writeln!(o, "[source.legacy_chardata]\nkind = \"legacy_gcsim\"\ntitle = \"The engine's CHARS row, generated by reference/gendata from cenoda/gcsim@eb21884\"\nurl = \"engine/src/data.rs\"\ngame_version = \"gcsim eb21884 (cenoda/gcsim, reference/go.mod)\"\nobserved = \"2026-11-05\"\n");
    let _ = writeln!(o, "[items]");
    for it in &items {
        // slices of slices give their rows' lengths, so a missing row is an error
        let len = match &it.val {
            RVal::Slice(rows) if rows.iter().all(|r| matches!(r, RVal::Slice(_))) && matches!(&it.ty, Ty::Slice(t) if matches!(**t, Ty::Slice(_))) => {
                Some(format!("[{}]", rows.iter().map(|r| r.items().map_or(0, |x| x.len()).to_string()).collect::<Vec<_>>().join(", ")))
            }
            RVal::Slice(rows) => Some(rows.len().to_string()),
            _ => None,
        };
        match len {
            Some(l) => {
                let _ = writeln!(o, "{} = {{ type = {}, len = {l} }}", it.name, quote(&it.ty.name()));
            }
            None => {
                let _ = writeln!(o, "{} = {}", it.name, quote(&it.ty.name()));
            }
        }
    }
    for it in &items {
        // talent tables: one value per row of levels
        let mut rows = Vec::new();
        fn walk(v: &RVal, ty: &Ty, path: &mut Vec<usize>, rows: &mut Vec<(Vec<usize>, RVal)>) -> bool {
            if is_talent_row(v) {
                rows.push((path.clone(), v.clone()));
                return true;
            }
            match (ty, v) {
                (Ty::Slice(t), RVal::Slice(a)) if !a.is_empty() => {
                    for (i, x) in a.iter().enumerate() {
                        path.push(i);
                        let ok = walk(x, t, path, rows);
                        path.pop();
                        if !ok {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            }
        }
        let talent = walk(&it.val, &it.ty, &mut Vec::new(), &mut rows);
        let (unit, layer, field) = match guess(&it.name, &it.ty, talent) {
            (u, l, f) if fits(&it.val, u) => (u, l, f),
            _ => ("unclassified", 1, None),
        };
        if talent {
            for (path, v) in rows {
                let idx: String = path.iter().map(|i| format!("[{i}]")).collect();
                let k = format!("{}{idx}", it.name.to_lowercase());
                w.value(&k, unit, layer, &format!("{}{idx}", it.name), field, Some("talent_level"), "legacy_tables", &toml_of(&v, &Ty::Slice(Box::new(Ty::F64))));
            }
        } else {
            w.value(&it.name.to_lowercase(), unit, layer, &it.name, field, None, "legacy_tables", &toml_of(&it.val, &it.ty));
        }
    }
    // the data.rs row
    let f = |k: &str| row.field(k).cloned().ok_or_else(|| format!("no `{k}` in the row"));
    let ints = |v: &RVal| -> Vec<i64> { v.items().unwrap_or(&[]).iter().map(|x| x.num().unwrap_or(0.0) as i64).collect() };
    let named = |names: &[&str], v: &RVal| -> String { format!("{{ {} }}", names.iter().zip(ints(v)).map(|(n, x)| format!("{n} = {x}")).collect::<Vec<_>>().join(", ")) };
    let pick = |names: &[&str], v: &RVal| -> String { quote(names.get(v.num().unwrap_or(-1.0) as usize).copied().unwrap_or("?")) };
    let base = f("base")?;
    let promo = f("promo")?;
    let mut promo_t = Vec::new();
    for p in promo.items().unwrap_or(&[]) {
        let t = p.items().ok_or("promo")?;
        let mut s = format!("{{ level = {}", t[0].num().unwrap_or(0.0) as i64);
        for st in t[1].items().unwrap_or(&[]) {
            let st = st.items().ok_or("promo stat")?;
            let name = record::stat_name(st[0].num().unwrap_or(0.0) as usize).unwrap_or("?");
            let _ = write!(s, ", {} = {}", crate::toml::key(name), float(st[1].num().unwrap_or(0.0)));
        }
        s.push_str(" }");
        promo_t.push(s);
    }
    let base_s = format!("[{}]", base.items().unwrap_or(&[]).iter().map(|x| float(x.num().unwrap_or(0.0))).collect::<Vec<_>>().join(", "));
    let rows: Vec<(&str, &str, i64, Option<&str>, String)> = vec![
        ("char.base", "points", 1, None, base_s),
        ("char.curves", "curve", 1, None, format!("{:?}", ints(&f("curves")?))),
        ("char.em", "points", 1, None, float(f("em")?.num().unwrap_or(0.0))),
        ("char.promo", "stat_bonus", 1, None, format!("[\n    {},\n  ]", promo_t.join(",\n    "))),
        ("char.element", "element", 1, None, pick(&ELEMENTS, &f("element")?)),
        ("char.weapon_class", "weapon_class", 1, None, pick(&WEAPON_CLASSES, &f("weapon_class")?)),
        ("char.body", "body", 1, None, pick(&BODIES, &f("body")?)),
        ("char.zone", "zone", 1, None, pick(&ZONES, &f("zone")?)),
        ("char.rarity", "rarity", 1, None, (f("rarity")?.num().unwrap_or(0.0) as i64).to_string()),
        ("char.energy_max", "energy", 1, None, float(f("energy_max")?.num().unwrap_or(0.0))),
        ("char.normal_hit_num", "count", 1, None, (f("normal_hit_num")?.num().unwrap_or(0.0) as i64).to_string()),
        ("char.cons", "constellation", 1, None, named(&CONS_NAMES, &f("cons")?)),
        ("char.anim_delay", "frames", 2, Some("frames"), named(&ANIM_DELAY_NAMES, &f("anim_delay")?)),
    ];
    for (t, unit, layer, field, data) in rows {
        w.value(t, unit, layer, t, field, None, "legacy_chardata", &data);
    }
    let _ = N_ACTIONS;
    Ok(w.o)
}
