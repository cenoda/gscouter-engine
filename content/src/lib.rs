//! Content records with provenance, their validator, and their converter
//! to the engine's data (`docs/content-format.md`). Step 1 of
//! `docs/independence.md` §5: content data can be added, validated and
//! converted without gcsim's source or tools.

pub mod catalog;
pub mod convert;
pub mod import;
pub mod record;
pub mod rustdata;
pub mod toml;
pub mod validate;

/// Reads, validates and converts a record file.
pub fn load(path: &std::path::Path, cat: &catalog::Catalog) -> Result<record::Record, Vec<String>> {
    let src = catalog::read(path).map_err(|e| vec![e])?;
    let rel = path.canonicalize().ok().and_then(|p| p.strip_prefix(&cat.root).ok().map(|p| p.display().to_string())).unwrap_or_else(|| path.display().to_string());
    let r = record::parse(&rel, &src)?;
    let errs = validate::validate(&r, cat);
    if errs.is_empty() { Ok(r) } else { Err(errs) }
}
