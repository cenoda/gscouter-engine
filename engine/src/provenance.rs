//! Where each value of the content data comes from (docs/independence.md
//! §4, docs/porting.md "Content data").
//!
//! Every record of `data.rs` and `gamedata` is `legacy_gcsim` unless a row
//! of `RECORDS` says otherwise: a value replaced by an official
//! observation, a derivation, a measurement or an estimate adds a row here
//! naming its content key and field, with its source. Nothing in a fight
//! reads this table; the hot structs carry no provenance field. `gse
//! --catalog` reports it, and the tests below check it.

use crate::data;

/// The kind of evidence behind a value (docs/independence.md §4, "Source
/// feasibility and provenance").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Evidence {
    /// Generated from gcsim eb21884 by reference/gendata, kept under the
    /// owner's legacy-data exception of 2026-11-04.
    LegacyGcsim,
    /// Read from an official source (HoYoWiki, the game's own showcase).
    Official,
    /// Computed from other values by a recorded formula.
    Derived,
    /// Measured in the game, with its evidence (a video, a test).
    Measured,
    /// An estimate, to be replaced when a measurement appears.
    Estimated,
}

impl Evidence {
    pub fn name(self) -> &'static str {
        match self {
            Evidence::LegacyGcsim => "legacy_gcsim",
            Evidence::Official => "official",
            Evidence::Derived => "derived",
            Evidence::Measured => "measured",
            Evidence::Estimated => "estimated",
        }
    }
}

/// What a content key names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Character,
    Weapon,
    Set,
    /// Tables no character, weapon or set owns: `gamedata::combat`,
    /// `gamedata::crystallize`, `gamedata::icd`, `data::CURVES`.
    Shared,
}

/// The keys of `Kind::Shared`.
pub static SHARED_KEYS: &[&str] = &["combat", "crystallize", "icd", "curves"];

/// One value whose provenance is not `legacy_gcsim`.
pub struct Record {
    pub kind: Kind,
    /// The content key (`data::CHAR_KEYS`, `WEAPON_KEYS`, `SET_KEYS`, or
    /// `SHARED_KEYS`).
    pub key: &'static str,
    /// The value: a Rust path below `engine/src`, with an index or field
    /// where the item holds several values, e.g.
    /// `gamedata::chars::xiangling::SKILL[0]` or `data::CHARS.base`.
    pub field: &'static str,
    pub evidence: Evidence,
    /// The game version the value was observed in, e.g. "6.1".
    pub version: &'static str,
    /// A URL, or a video with its timestamp; for a derived value, its
    /// formula and inputs.
    pub source: &'static str,
    /// The date of the observation, YYYY-MM-DD.
    pub observed: &'static str,
    pub note: &'static str,
}

/// The values replaced since the legacy data. Empty: every value is still
/// `legacy_gcsim`.
pub static RECORDS: &[Record] = &[];

/// The provenance of one field of one content key: its record, or
/// `legacy_gcsim` when it has none.
pub fn of(kind: Kind, key: &str, field: &str) -> Evidence {
    RECORDS.iter().find(|r| r.kind == kind && r.key == key && r.field == field).map_or(Evidence::LegacyGcsim, |r| r.evidence)
}

/// The records of one content key.
pub fn records(kind: Kind, key: &str) -> impl Iterator<Item = &'static Record> + '_ {
    RECORDS.iter().filter(move |r| r.kind == kind && r.key == key)
}

/// Every content key of a kind.
pub fn keys(kind: Kind) -> Vec<&'static str> {
    match kind {
        Kind::Character => data::CHAR_KEYS.iter().map(|k| k.1).collect(),
        Kind::Weapon => data::WEAPON_KEYS.iter().map(|k| k.1).collect(),
        Kind::Set => data::SET_KEYS.iter().map(|k| k.1).collect(),
        Kind::Shared => SHARED_KEYS.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [Kind; 4] = [Kind::Character, Kind::Weapon, Kind::Set, Kind::Shared];

    #[test]
    fn every_content_key_has_a_provenance() {
        let mut n = 0;
        for kind in KINDS {
            for key in keys(kind) {
                // The default counts: a key without records is legacy_gcsim.
                let e = of(kind, key, "");
                assert!(records(kind, key).count() > 0 || e == Evidence::LegacyGcsim, "{kind:?} {key}");
                n += 1;
            }
        }
        assert_eq!(n, data::CHAR_KEYS.len() + data::WEAPON_KEYS.len() + data::SET_KEYS.len() + SHARED_KEYS.len());
    }

    #[test]
    fn records_name_known_keys_and_carry_their_source() {
        for (i, r) in RECORDS.iter().enumerate() {
            let what = format!("provenance::RECORDS[{i}] ({:?} {} {})", r.kind, r.key, r.field);
            assert!(keys(r.kind).contains(&r.key), "{what}: unknown key");
            assert!(r.field.starts_with("data::") || r.field.starts_with("gamedata::"), "{what}: field is a path below engine/src");
            assert_ne!(r.evidence, Evidence::LegacyGcsim, "{what}: legacy_gcsim is the default, not a record");
            assert!(!r.source.is_empty() && !r.version.is_empty(), "{what}: needs its source and game version");
            let d = r.observed.as_bytes();
            assert!(d.len() == 10 && d[4] == b'-' && d[7] == b'-', "{what}: observed is YYYY-MM-DD");
            assert!(RECORDS[..i].iter().all(|o| (o.kind, o.key, o.field) != (r.kind, r.key, r.field)), "{what}: recorded twice");
        }
    }

    #[test]
    fn shared_keys_name_tables_that_exist() {
        // combat, crystallize and icd are modules of gamedata; curves is data::CURVES.
        assert!(!crate::gamedata::combat::REACTION_LVL_BASE.is_empty());
        assert!(!crate::gamedata::crystallize::SHIELD_BASE_HP.is_empty());
        assert_eq!(crate::gamedata::icd::TAG_NONE.0, 0);
        assert!(!data::CURVES.is_empty());
    }
}
