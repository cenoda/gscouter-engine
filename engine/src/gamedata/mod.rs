// Maintained in this repository: edit by hand (docs/porting.md, "Content data").
// Provenance: legacy_gcsim. Generated from gcsim eb21884 by reference/gendata
// and kept under the owner's legacy-data exception of 2026-11-04
// (docs/independence.md §4). A value replaced by an official observation,
// a derivation, a measurement or an estimate records its new provenance in
// engine/src/provenance.rs.

//! The content data of gcsim's packages, maintained here: one module per
//! character, weapon and artifact package (talent multipliers, frames,
//! hitmarks, hitlag, mod keys), the ICD tables and the shared combat
//! tables. Module names follow gcsim's package paths.

#![allow(dead_code, clippy::all)]

pub mod icd;
pub mod combat;
pub mod crystallize;
pub mod artifacts;
pub mod chars;
pub mod weapons;
