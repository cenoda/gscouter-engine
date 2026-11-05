//! The key catalog: every character, weapon and artifact set the engine
//! knows, with the names gcsl accepts for it (gcsim's `pkg/shortcut` at
//! eb21884, `data::CHAR_NAMES`, `WEAPON_NAMES`, `SET_NAMES`) and what
//! Gscouter reads from gcsim's catalog. `data.rs` is the single source of
//! these names; a new name is added there only.
//!
//! `json()` is the versioned export (`gse --catalog`, the Wasm module's
//! `gse_catalog`, `api::catalog`):
//!
//! ```text
//! {"catalog_version": 1, "provenance_default": "legacy_gcsim",
//!  "characters": [{"key", "aliases", "implemented", "rarity", "element",
//!                  "weapon_class", "burst_energy", "provenance"}],
//!  "weapons":    [{"key", "aliases", "implemented", "rarity", "weapon_class", "provenance"}],
//!  "sets":       [{"key", "aliases", "implemented", "provenance"}]}
//! ```
//!
//! Entries are sorted by key. `aliases` are the other names that resolve
//! to the key (the key itself always does), sorted. `implemented` says the
//! engine has the content's logic (a module in `chars`, `weapons` or
//! `sets`); a key without it is refused by `gse --check`. A character
//! without base data has no `rarity`, `element`, `weapon_class` or
//! `burst_energy`. `burst_energy` is the energy cost (`EnergyMax`), 0 for
//! a character whose burst costs no energy. `provenance` is
//! `"legacy_gcsim"`, or the records of `provenance::RECORDS` for the key.
//! A change that removes or renames a field raises `catalog_version`;
//! adding a field does not.

use std::fmt::Write as _;

use crate::data;
use crate::provenance::{self, Kind};
use crate::sim::{CharKind, SetKind, WeaponKind};

pub const CATALOG_VERSION: u32 = 1;

/// gcsim's attributes.Element names, by its value.
const ELEMENTS: [&str; 11] = ["", "pyro", "hydro", "dendro", "electro", "cryo", "anemo", "geo", "physical", "frozen", "quicken"];
/// gcsim's info.WeaponClass names, by its value.
const WEAPON_CLASSES: [&str; 5] = ["sword", "claymore", "polearm", "bow", "catalyst"];

/// The keys of a name table and the other names of each, sorted by key.
fn entries(names: &'static [(&'static str, u16)], keys: &'static [(u16, &'static str)]) -> Vec<(&'static str, Vec<&'static str>)> {
    let mut v: Vec<_> = keys
        .iter()
        .map(|&(id, key)| {
            let mut aliases: Vec<_> = names.iter().filter(|&&(n, i)| i == id && n != key).map(|n| n.0).collect();
            aliases.sort_unstable();
            (key, aliases)
        })
        .collect();
    v.sort_unstable_by_key(|e| e.0);
    v
}

fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn head(out: &mut String, key: &str, aliases: &[&str], implemented: bool) {
    out.push_str("{\"key\": ");
    string(out, key);
    out.push_str(", \"aliases\": [");
    for (i, a) in aliases.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        string(out, a);
    }
    let _ = write!(out, "], \"implemented\": {implemented}");
}

fn tail(out: &mut String, kind: Kind, key: &str) {
    out.push_str(", \"provenance\": ");
    let mut recs = provenance::records(kind, key).peekable();
    if recs.peek().is_none() {
        string(out, provenance::Evidence::LegacyGcsim.name());
    } else {
        out.push('[');
        for (i, r) in recs.enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str("{\"field\": ");
            string(out, r.field);
            out.push_str(", \"evidence\": ");
            string(out, r.evidence.name());
            out.push_str(", \"version\": ");
            string(out, r.version);
            out.push_str(", \"source\": ");
            string(out, r.source);
            out.push_str(", \"observed\": ");
            string(out, r.observed);
            out.push('}');
        }
        out.push(']');
    }
    out.push('}');
}

fn num(x: f64) -> String {
    if x.fract() == 0.0 { format!("{}", x as i64) } else { format!("{x}") }
}

/// The catalog as JSON (see the module's documentation).
pub fn json() -> String {
    let mut out = String::with_capacity(64 * 1024);
    let _ = write!(out, "{{\"catalog_version\": {CATALOG_VERSION}, \"provenance_default\": \"legacy_gcsim\",\n\"characters\": [");
    for (i, (key, aliases)) in entries(data::CHAR_NAMES, data::CHAR_KEYS).iter().enumerate() {
        out.push_str(if i > 0 { ",\n  " } else { "\n  " });
        head(&mut out, key, aliases, CharKind::from_name(key).is_some());
        if let Some(d) = data::CHARS.iter().find(|d| d.key == *key) {
            let _ = write!(
                out,
                ", \"rarity\": {}, \"element\": \"{}\", \"weapon_class\": \"{}\", \"burst_energy\": {}",
                d.rarity,
                ELEMENTS[d.element as usize],
                WEAPON_CLASSES[d.weapon_class as usize],
                num(d.energy_max)
            );
        }
        tail(&mut out, Kind::Character, key);
    }
    out.push_str("\n],\n\"weapons\": [");
    for (i, (key, aliases)) in entries(data::WEAPON_NAMES, data::WEAPON_KEYS).iter().enumerate() {
        out.push_str(if i > 0 { ",\n  " } else { "\n  " });
        head(&mut out, key, aliases, WeaponKind::from_name(key).is_some());
        if let Some(d) = data::WEAPONS.iter().find(|d| d.key == *key) {
            let _ = write!(out, ", \"rarity\": {}, \"weapon_class\": \"{}\"", d.rarity, WEAPON_CLASSES[d.class as usize]);
        }
        tail(&mut out, Kind::Weapon, key);
    }
    out.push_str("\n],\n\"sets\": [");
    for (i, (key, aliases)) in entries(data::SET_NAMES, data::SET_KEYS).iter().enumerate() {
        out.push_str(if i > 0 { ",\n  " } else { "\n  " });
        head(&mut out, key, aliases, SetKind::from_name(key).is_some());
        tail(&mut out, Kind::Set, key);
    }
    out.push_str("\n]}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcsl;

    /// Every name resolves through gcsl to the key the catalog lists it under.
    #[test]
    fn names_resolve_to_their_keys() {
        for (key, aliases) in entries(data::CHAR_NAMES, data::CHAR_KEYS) {
            for n in aliases.iter().chain([&key]) {
                assert_eq!(gcsl::char_name(gcsl::char_key(n).unwrap()), key, "{n}");
            }
        }
        for (key, aliases) in entries(data::WEAPON_NAMES, data::WEAPON_KEYS) {
            for n in aliases.iter().chain([&key]) {
                assert_eq!(gcsl::weapon_name(gcsl::weapon_key(n).unwrap()), key, "{n}");
            }
        }
        for (key, aliases) in entries(data::SET_NAMES, data::SET_KEYS) {
            for n in aliases.iter().chain([&key]) {
                assert_eq!(gcsl::set_name(gcsl::set_key(n).unwrap()), key, "{n}");
            }
        }
    }

    /// The key tables are one source: no key twice, every name lowercase
    /// and resolving to one listed key, every data record under a key.
    #[test]
    fn key_tables_are_consistent() {
        fn check(what: &str, names: &[(&str, u16)], keys: &[(u16, &str)]) {
            for (i, k) in keys.iter().enumerate() {
                assert!(keys[..i].iter().all(|o| o.0 != k.0 && o.1 != k.1), "{what}: {} twice", k.1);
                assert!(names.iter().any(|n| n.0 == k.1 && n.1 == k.0), "{what}: key {} is not a name of itself", k.1);
            }
            for (i, n) in names.iter().enumerate() {
                assert_eq!(n.0, n.0.to_lowercase(), "{what}: {} is not lowercase", n.0);
                assert!(names[..i].iter().all(|o| o.0 != n.0), "{what}: name {} twice", n.0);
                assert!(keys.iter().any(|k| k.0 == n.1), "{what}: name {} has no key", n.0);
            }
        }
        check("characters", data::CHAR_NAMES, data::CHAR_KEYS);
        check("weapons", data::WEAPON_NAMES, data::WEAPON_KEYS);
        check("sets", data::SET_NAMES, data::SET_KEYS);
        for d in data::CHARS {
            assert!(data::CHAR_KEYS.iter().any(|k| k.1 == d.key), "data::CHARS {} has no key", d.key);
        }
        for d in data::WEAPONS {
            assert!(data::WEAPON_KEYS.iter().any(|k| k.1 == d.key), "data::WEAPONS {} has no key", d.key);
        }
        for s in data::SETS {
            assert!(data::SET_KEYS.iter().any(|k| k.1 == s.0), "data::SETS {} has no key", s.0);
        }
    }

    #[test]
    fn json_lists_every_key() {
        let j = json();
        assert!(j.starts_with("{\"catalog_version\": 1,"));
        for k in data::CHAR_KEYS.iter().map(|k| k.1).chain(data::WEAPON_KEYS.iter().map(|k| k.1)).chain(data::SET_KEYS.iter().map(|k| k.1)) {
            assert!(j.contains(&format!("{{\"key\": \"{k}\"")), "{k}");
        }
        assert!(j.contains("{\"key\": \"hutao\", \"aliases\": [\"ht\", \"tao\"], \"implemented\": true, \"rarity\": 5, \"element\": \"pyro\", \"weapon_class\": \"polearm\", \"burst_energy\": 60, \"provenance\": \"legacy_gcsim\"}"), "{}", &j[..400]);
    }
}
