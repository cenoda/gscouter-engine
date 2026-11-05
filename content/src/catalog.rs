//! What a record may refer to, read from the engine's source: character
//! keys and their data rows (`data.rs`), base-stat growth curves, ICD tags
//! and groups, and the existing tables modules.
//!
//! The data module is `engine/src/gcsimdata` until it is renamed; both its
//! old name and `gamedata` are looked for, so a rename needs no change here.

use crate::rustdata::{self, Item, RVal};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Catalog {
    /// The repository's root.
    pub root: PathBuf,
    /// `engine/src`.
    pub src: PathBuf,
    /// The data module's directory (`engine/src/gcsimdata` or its new name).
    pub data_dir: PathBuf,
    /// Every name and alias, to its character id.
    pub char_names: BTreeMap<String, u16>,
    /// Each character id's canonical key.
    pub char_keys: BTreeMap<u16, String>,
    /// The rows of `CHARS`, by key, as text and as values.
    pub char_rows: BTreeMap<String, (String, RVal)>,
    /// Growth curves by id, levels 1 up.
    pub curves: BTreeMap<u32, Vec<f64>>,
    /// ICD tag and group names, lowercased without `TAG_` / `GROUP_`.
    pub icd_tags: Vec<String>,
    pub icd_groups: Vec<String>,
}

/// The data module directories, newest name first.
pub const DATA_DIRS: [&str; 2] = ["gamedata", "gcsimdata"];

impl Catalog {
    /// Finds the repository from a directory inside it.
    pub fn find(from: &Path) -> Result<Catalog, String> {
        let mut d = from.canonicalize().map_err(|e| format!("{}: {e}", from.display()))?;
        loop {
            if d.join("engine/src/data.rs").is_file() {
                return Catalog::load(&d);
            }
            if !d.pop() {
                return Err(format!("no engine/src/data.rs above {}", from.display()));
            }
        }
    }

    pub fn load(root: &Path) -> Result<Catalog, String> {
        let src = root.join("engine/src");
        let data_dir = DATA_DIRS.iter().map(|d| src.join(d)).find(|d| d.join("icd.rs").is_file()).ok_or("no engine/src/gamedata or gcsimdata")?;
        let data = read(&src.join("data.rs"))?;
        let mut c = Catalog {
            root: root.to_path_buf(),
            src: src.clone(),
            data_dir: data_dir.clone(),
            char_names: BTreeMap::new(),
            char_keys: BTreeMap::new(),
            char_rows: BTreeMap::new(),
            curves: BTreeMap::new(),
            icd_tags: Vec::new(),
            icd_groups: Vec::new(),
        };
        for line in statics(&data, &["CHAR_NAMES", "CHAR_KEYS", "CURVES"]).lines() {
            let name = line["pub static ".len()..].split(':').next().unwrap();
            let eq = line.find(" = ").ok_or("no value")?;
            let val = rustdata::parse_literal(line[eq + 3..].trim().trim_end_matches(';'))?;
            let rows = val.items().ok_or("not a slice")?;
            for r in rows {
                let t = r.items().ok_or("not a tuple")?;
                match name {
                    "CHAR_NAMES" => {
                        if let (RVal::S(n), RVal::I(id)) = (&t[0], &t[1]) {
                            c.char_names.insert(n.clone(), *id as u16);
                        }
                    }
                    "CHAR_KEYS" => {
                        if let (RVal::I(id), RVal::S(n)) = (&t[0], &t[1]) {
                            c.char_keys.insert(*id as u16, n.clone());
                        }
                    }
                    _ => {
                        if let (RVal::I(id), Some(v)) = (&t[0], t[1].items()) {
                            c.curves.insert(*id as u32, v.iter().filter_map(RVal::num).collect());
                        }
                    }
                }
            }
        }
        for line in data.lines() {
            let l = line.trim();
            if l.starts_with("CharData {") {
                let text = l.strip_suffix(',').unwrap_or(l);
                let v = rustdata::parse_literal(text)?;
                if let Some(RVal::S(k)) = v.field("key") {
                    c.char_rows.insert(k.clone(), (text.to_string(), v.clone()));
                }
            }
        }
        let icd = read(&data_dir.join("icd.rs"))?;
        for line in icd.lines() {
            if let Some(r) = line.strip_prefix("pub const TAG_") {
                c.icd_tags.push(r.split(':').next().unwrap().to_lowercase());
            } else if let Some(r) = line.strip_prefix("pub const GROUP_") {
                c.icd_groups.push(r.split(':').next().unwrap().to_lowercase());
            }
        }
        if c.char_names.is_empty() || c.curves.is_empty() || c.icd_tags.is_empty() {
            return Err("the engine's catalogs could not be read".into());
        }
        Ok(c)
    }

    /// The canonical key of a name or alias.
    pub fn char_key(&self, name: &str) -> Option<&str> {
        self.char_names.get(name).and_then(|id| self.char_keys.get(id)).map(String::as_str)
    }

    /// The path of a character's tables module.
    pub fn char_module_path(&self, module: &str) -> PathBuf {
        self.data_dir.join("chars").join(format!("{module}.rs"))
    }

    pub fn char_module(&self, module: &str) -> Result<Vec<Item>, String> {
        rustdata::parse_module(&read(&self.char_module_path(module))?)
    }

    /// The repository path of a file, from the root, with `gcsimdata` read
    /// as the data module's current name.
    pub fn repo_file(&self, rel: &str) -> PathBuf {
        let p = self.root.join(rel);
        if p.exists() {
            return p;
        }
        for d in DATA_DIRS {
            let alt = rel.replace("engine/src/gcsimdata/", &format!("engine/src/{d}/"));
            if self.root.join(&alt).exists() {
                return self.root.join(alt);
            }
        }
        p
    }
}

pub fn read(p: &Path) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))
}

/// The lines of the named `pub static`s of a file, each joined to one line.
fn statics(src: &str, names: &[&str]) -> String {
    let mut o = String::new();
    let mut inside = false;
    for line in src.lines() {
        if let Some(r) = line.strip_prefix("pub static ") {
            inside = names.iter().any(|n| r.starts_with(&format!("{n}:")));
            if inside {
                o.push_str(line);
                if line.ends_with(';') {
                    o.push('\n');
                    inside = false;
                }
                continue;
            }
        }
        if inside {
            o.push(' ');
            o.push_str(line.trim());
            if line.trim() == "];" {
                o.push('\n');
                inside = false;
            }
        }
    }
    o
}
