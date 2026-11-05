//! The engine's data as Rust source: the types and values of the items of
//! a per-content tables module (`engine/src/<data>/chars/<name>.rs`) and
//! the rows of `engine/src/data.rs`, read from and written to text in the
//! exact form the engine's files use.

use std::fmt::Write;

/// The type of a table item.
#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    F64,
    I32,
    Str,
    Bool,
    /// `FrameTable`, `[i32; N_ACTIONS]`.
    Frames,
    /// `&[T]`.
    Slice(Box<Ty>),
}

pub const N_ACTIONS: usize = 15;

/// gcsim's `action.Action` names, by index in a frame table (the engine's
/// `types::Action`). Index 0 is the action's own length; 12 to 14 have no
/// action and keep the length.
pub const ACTIONS: [&str; 12] = ["length", "skill", "burst", "attack", "charge", "high_plunge", "low_plunge", "aim", "dash", "jump", "swap", "walk"];

impl Ty {
    pub fn parse(s: &str) -> Result<Ty, String> {
        let s = s.trim();
        Ok(match s {
            "f64" => Ty::F64,
            "i32" => Ty::I32,
            "&str" => Ty::Str,
            "bool" => Ty::Bool,
            "FrameTable" => Ty::Frames,
            _ => {
                let inner = s.strip_prefix("&[").and_then(|r| r.strip_suffix(']')).ok_or_else(|| format!("unsupported type `{s}`"))?;
                Ty::Slice(Box::new(Ty::parse(inner)?))
            }
        })
    }

    pub fn name(&self) -> String {
        match self {
            Ty::F64 => "f64".into(),
            Ty::I32 => "i32".into(),
            Ty::Str => "&str".into(),
            Ty::Bool => "bool".into(),
            Ty::Frames => "FrameTable".into(),
            Ty::Slice(t) => format!("&[{}]", t.name()),
        }
    }

    pub fn is_scalar(&self) -> bool {
        matches!(self, Ty::F64 | Ty::I32 | Ty::Str | Ty::Bool)
    }

    pub fn uses_frames(&self) -> bool {
        match self {
            Ty::Frames => true,
            Ty::Slice(t) => t.uses_frames(),
            _ => false,
        }
    }

    /// The type at a path of indices into this one.
    pub fn at(&self, depth: usize) -> Option<&Ty> {
        let mut t = self;
        for _ in 0..depth {
            t = match t {
                Ty::Slice(i) => i,
                Ty::Frames => &Ty::I32,
                _ => return None,
            };
        }
        Some(t)
    }
}

/// A Rust literal of the engine's data.
#[derive(Clone, Debug, PartialEq)]
pub enum RVal {
    F(f64),
    I(i64),
    S(String),
    B(bool),
    /// `&[...]`
    Slice(Vec<RVal>),
    /// `[...]`
    Arr(Vec<RVal>),
    /// `(...)`
    Tuple(Vec<RVal>),
    /// `Name { field: value, ... }`
    Struct(String, Vec<(String, RVal)>),
}

/// A float as the engine's generated files print it.
pub fn float(x: f64) -> String {
    format!("{x:?}")
}

impl RVal {
    pub fn write(&self, o: &mut String) {
        match self {
            RVal::F(x) => o.push_str(&float(*x)),
            RVal::I(i) => {
                let _ = write!(o, "{i}");
            }
            RVal::S(s) => {
                o.push('"');
                for c in s.chars() {
                    match c {
                        '"' => o.push_str("\\\""),
                        '\\' => o.push_str("\\\\"),
                        c => o.push(c),
                    }
                }
                o.push('"');
            }
            RVal::B(b) => {
                let _ = write!(o, "{b}");
            }
            RVal::Slice(v) | RVal::Arr(v) | RVal::Tuple(v) => {
                let (a, b) = match self {
                    RVal::Slice(_) => ("&[", "]"),
                    RVal::Arr(_) => ("[", "]"),
                    _ => ("(", ")"),
                };
                o.push_str(a);
                for (i, x) in v.iter().enumerate() {
                    if i > 0 {
                        o.push_str(", ");
                    }
                    x.write(o);
                }
                o.push_str(b);
            }
            RVal::Struct(name, fields) => {
                o.push_str(name);
                o.push_str(" { ");
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        o.push_str(", ");
                    }
                    o.push_str(k);
                    o.push_str(": ");
                    v.write(o);
                }
                o.push_str(" }");
            }
        }
    }

    pub fn text(&self) -> String {
        let mut o = String::new();
        self.write(&mut o);
        o
    }

    pub fn items(&self) -> Option<&[RVal]> {
        match self {
            RVal::Slice(v) | RVal::Arr(v) | RVal::Tuple(v) => Some(v),
            _ => None,
        }
    }

    pub fn num(&self) -> Option<f64> {
        match self {
            RVal::F(x) => Some(*x),
            RVal::I(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn field(&self, k: &str) -> Option<&RVal> {
        match self {
            RVal::Struct(_, f) => f.iter().find(|(n, _)| n == k).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Checks that a value has a type, as the compiler would.
    pub fn check(&self, ty: &Ty) -> Result<(), String> {
        match (ty, self) {
            (Ty::I32, RVal::I(i)) if i32::try_from(*i).is_err() => Err(format!("{i} is out of i32's range")),
            (Ty::F64, RVal::F(_)) | (Ty::I32, RVal::I(_)) | (Ty::Str, RVal::S(_)) | (Ty::Bool, RVal::B(_)) => Ok(()),
            (Ty::Frames, RVal::Arr(v)) if v.len() == N_ACTIONS => v.iter().try_for_each(|x| x.check(&Ty::I32)),
            (Ty::Slice(t), RVal::Slice(v)) => v.iter().try_for_each(|x| x.check(t)),
            _ => Err(format!("`{}` is not a {}", self.text(), ty.name())),
        }
    }
}

/// An item of a tables module.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub name: String,
    pub ty: Ty,
    pub val: RVal,
}

impl Item {
    /// `pub const` for scalars, `pub static` for the rest, as the engine's
    /// generated files have them.
    pub fn line(&self) -> String {
        let kw = if self.ty.is_scalar() { "const" } else { "static" };
        format!("pub {kw} {}: {} = {};", self.name, self.ty.name(), self.val.text())
    }
}

/// The body of a tables module: the `use` line if a frame table is used,
/// then the items in the order given (a record's `[items]` order), one per
/// line.
pub fn module_body(items: &[Item]) -> String {
    let mut o = String::new();
    if items.iter().any(|i| i.ty.uses_frames()) {
        o.push_str("use crate::types::FrameTable;\n\n");
    }
    for i in items {
        o.push_str(&i.line());
        o.push('\n');
    }
    o
}

/// The body of a module: everything after its leading comment lines and the
/// blank lines around them.
pub fn strip_header(src: &str) -> &str {
    let mut rest = src;
    loop {
        let line_end = rest.find('\n').map_or(rest.len(), |i| i + 1);
        let line = rest[..line_end].trim();
        if line.starts_with("//") || line.is_empty() {
            if line_end == 0 {
                return rest;
            }
            rest = &rest[line_end..];
        } else {
            return rest;
        }
    }
}

/// Reads the items of a tables module.
pub fn parse_module(src: &str) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    for (n, line) in src.lines().enumerate() {
        let Some(rest) = line.strip_prefix("pub const ").or_else(|| line.strip_prefix("pub static ")) else { continue };
        let colon = rest.find(':').ok_or_else(|| format!("line {}: no type", n + 1))?;
        let eq = rest.find(" = ").ok_or_else(|| format!("line {}: no value", n + 1))?;
        let name = rest[..colon].trim().to_string();
        let ty = Ty::parse(&rest[colon + 1..eq]).map_err(|e| format!("line {}: {e}", n + 1))?;
        let vtext = rest[eq + 3..].trim().strip_suffix(';').ok_or_else(|| format!("line {}: no `;`", n + 1))?;
        let val = parse_literal(vtext).map_err(|e| format!("line {}: {e}", n + 1))?;
        items.push(Item { name, ty, val });
    }
    Ok(items)
}

/// Reads one Rust literal of the engine's data.
pub fn parse_literal(s: &str) -> Result<RVal, String> {
    let mut p = Lit { s: s.as_bytes(), i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(format!("trailing text after a literal at {}", p.i));
    }
    Ok(v)
}

struct Lit<'a> {
    s: &'a [u8],
    i: usize,
}

impl Lit<'_> {
    fn ws(&mut self) {
        while self.s.get(self.i).is_some_and(|c| c.is_ascii_whitespace()) {
            self.i += 1;
        }
    }
    fn eat(&mut self, t: &str) -> bool {
        self.ws();
        if self.s[self.i..].starts_with(t.as_bytes()) {
            self.i += t.len();
            true
        } else {
            false
        }
    }
    fn list(&mut self, close: &str) -> Result<Vec<RVal>, String> {
        let mut v = Vec::new();
        loop {
            if self.eat(close) {
                return Ok(v);
            }
            v.push(self.value()?);
            if !self.eat(",") && !self.eat(close) {
                return Err(format!("expected `,` or `{close}` at {}", self.i));
            } else if self.s[self.i - 1] == close.as_bytes()[0] {
                return Ok(v);
            }
        }
    }
    fn value(&mut self) -> Result<RVal, String> {
        self.ws();
        if self.eat("&[") {
            return self.list("]").map(RVal::Slice);
        }
        if self.eat("[") {
            return self.list("]").map(RVal::Arr);
        }
        if self.eat("(") {
            return self.list(")").map(RVal::Tuple);
        }
        let c = *self.s.get(self.i).ok_or("unexpected end")?;
        if c == b'"' {
            self.i += 1;
            let mut o = String::new();
            loop {
                let c = *self.s.get(self.i).ok_or("unterminated string")?;
                self.i += 1;
                match c {
                    b'"' => return Ok(RVal::S(o)),
                    b'\\' => {
                        let e = *self.s.get(self.i).ok_or("unterminated escape")?;
                        self.i += 1;
                        o.push(match e {
                            b'"' => '"',
                            b'\\' => '\\',
                            b'n' => '\n',
                            _ => return Err("unsupported escape".into()),
                        });
                    }
                    _ => {
                        // multi-byte UTF-8 is copied byte by byte
                        let st = self.i - 1;
                        let len = match c {
                            0..=0x7f => 1,
                            0xc0..=0xdf => 2,
                            0xe0..=0xef => 3,
                            _ => 4,
                        };
                        self.i = st + len;
                        o.push_str(std::str::from_utf8(&self.s[st..self.i]).map_err(|e| e.to_string())?);
                    }
                }
            }
        }
        if c == b'-' || c.is_ascii_digit() {
            let st = self.i;
            self.i += 1;
            while self.s.get(self.i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'.' || (*c == b'-' && matches!(self.s[self.i - 1], b'e' | b'E'))) {
                self.i += 1;
            }
            let t = std::str::from_utf8(&self.s[st..self.i]).unwrap();
            return if t.contains(['.', 'e', 'E']) {
                t.parse().map(RVal::F).map_err(|_| format!("bad float `{t}`"))
            } else {
                t.parse().map(RVal::I).map_err(|_| format!("bad integer `{t}`"))
            };
        }
        // an identifier: true, false or a struct
        let st = self.i;
        while self.s.get(self.i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
            self.i += 1;
        }
        let id = std::str::from_utf8(&self.s[st..self.i]).unwrap().to_string();
        match id.as_str() {
            "true" => return Ok(RVal::B(true)),
            "false" => return Ok(RVal::B(false)),
            "" => return Err(format!("unexpected `{}` at {}", c as char, self.i)),
            _ => {}
        }
        if !self.eat("{") {
            return Err(format!("unsupported literal `{id}`"));
        }
        let mut fields = Vec::new();
        loop {
            if self.eat("}") {
                return Ok(RVal::Struct(id, fields));
            }
            self.ws();
            let st = self.i;
            while self.s.get(self.i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
                self.i += 1;
            }
            let k = std::str::from_utf8(&self.s[st..self.i]).unwrap().to_string();
            if k.is_empty() || !self.eat(":") {
                return Err(format!("expected a field at {}", self.i));
            }
            let v = self.value()?;
            fields.push((k, v));
            if !self.eat(",") {
                if self.eat("}") {
                    return Ok(RVal::Struct(id, fields));
                }
                return Err(format!("expected `,` or `}}` at {}", self.i));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_items() {
        let src = "use crate::types::FrameTable;\n\npub static A: &[&[f64]] = &[&[0.42054, 1.0], &[]];\npub const B: i32 = -3;\npub const C: &str = \"x-y\";\npub static D: FrameTable = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];\n";
        let items = parse_module(src).unwrap();
        assert_eq!(module_body(&items), src);
        for i in &items {
            i.val.check(&i.ty).unwrap();
        }
    }

    #[test]
    fn reads_a_struct_row() {
        let v = parse_literal("CharData { key: \"x\", base: [1.5, 2.0, 3.0], promo: &[(20, &[]), (40, &[(23, 1.25)])], cons: [-1, 5, 3] }").unwrap();
        assert_eq!(v.field("key"), Some(&RVal::S("x".into())));
        assert_eq!(v.text(), "CharData { key: \"x\", base: [1.5, 2.0, 3.0], promo: &[(20, &[]), (40, &[(23, 1.25)])], cons: [-1, 5, 3] }");
    }

    #[test]
    fn strips_headers() {
        assert_eq!(strip_header("// a\n\n//! b\n\nuse x;\n"), "use x;\n");
    }
}
