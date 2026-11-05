//! A strict subset of TOML, enough for content records and readable by any
//! TOML parser (Python's `tomllib` reads every record the same way).
//!
//! Supported: comments; `key = value` with bare or quoted keys; `[table]`
//! and `[[array.of.tables]]` headers with dotted names; basic strings with
//! the escapes `\" \\ \n \t \uXXXX`, literal strings; integers; floats
//! (decimal, with an exponent); booleans; arrays over several lines with
//! trailing commas and comments; inline tables. Not supported (an error):
//! dotted keys in assignments, multi-line strings, dates, hex/octal/binary
//! numbers, `inf` and `nan`, underscores in numbers.
//!
//! Tables keep the order their keys were written in.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<Value>),
    Table(Table),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Table {
    pub entries: Vec<(String, Value)>,
    /// The line each key was set on, for messages.
    pub lines: Vec<usize>,
}

impl Table {
    pub fn get(&self, k: &str) -> Option<&Value> {
        self.entries.iter().find(|(n, _)| n == k).map(|(_, v)| v)
    }
    pub fn line_of(&self, k: &str) -> usize {
        self.entries.iter().position(|(n, _)| n == k).map_or(0, |i| self.lines[i])
    }
    fn get_mut(&mut self, k: &str) -> Option<&mut Value> {
        self.entries.iter_mut().find(|(n, _)| n == k).map(|(_, v)| v)
    }
    fn insert(&mut self, k: String, v: Value, line: usize) -> Result<(), String> {
        if self.get(&k).is_some() {
            return Err(format!("line {line}: key `{k}` set twice"));
        }
        self.entries.push((k, v));
        self.lines.push(line);
        Ok(())
    }
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        if let Value::Str(s) = self { Some(s) } else { None }
    }
    pub fn as_table(&self) -> Option<&Table> {
        if let Value::Table(t) = self { Some(t) } else { None }
    }
    pub fn as_array(&self) -> Option<&[Value]> {
        if let Value::Array(a) = self { Some(a) } else { None }
    }
    pub fn as_int(&self) -> Option<i64> {
        if let Value::Int(i) = self { Some(*i) } else { None }
    }
    /// A number, integer or float.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Str(_) => "string",
            Value::Int(_) => "integer",
            Value::Float(_) => "float",
            Value::Bool(_) => "boolean",
            Value::Array(_) => "array",
            Value::Table(_) => "table",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Str(s) => write!(f, "{}", quote(s)),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(x) => write!(f, "{}", float(*x)),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Array(a) => {
                write!(f, "[")?;
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::Table(t) => {
                write!(f, "{{ ")?;
                for (i, (k, v)) in t.entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{} = {v}", key(k))?;
                }
                write!(f, " }}")
            }
        }
    }
}

/// A float as TOML (always with a point or an exponent, shortest round trip).
pub fn float(x: f64) -> String {
    format!("{x:?}")
}

pub fn quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04X}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

pub fn key(k: &str) -> String {
    if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') { k.to_string() } else { quote(k) }
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
    line: usize,
}

pub fn parse(src: &str) -> Result<Table, String> {
    let mut p = P { s: src.as_bytes(), i: 0, line: 1 };
    let mut root = Table::default();
    // the path of the current table: names, and for arrays of tables, the
    // last element
    let mut path: Vec<String> = Vec::new();
    loop {
        p.skip_ws_comments_newlines();
        if p.i >= p.s.len() {
            break;
        }
        let line = p.line;
        if p.peek() == Some(b'[') {
            let array = p.s.get(p.i + 1) == Some(&b'[');
            p.i += if array { 2 } else { 1 };
            let names = p.header_names()?;
            p.expect(b']')?;
            if array {
                p.expect(b']')?;
            }
            p.end_of_line()?;
            open_table(&mut root, &names, array, line)?;
            path = names;
            continue;
        }
        let k = p.key()?;
        p.skip_spaces();
        p.expect(b'=')?;
        p.skip_spaces();
        let v = p.value()?;
        p.end_of_line()?;
        let t = current(&mut root, &path).ok_or_else(|| format!("line {line}: no table"))?;
        t.insert(k, v, line)?;
    }
    Ok(root)
}

fn open_table(root: &mut Table, names: &[String], array: bool, line: usize) -> Result<(), String> {
    let mut t = root;
    for (n, name) in names.iter().enumerate() {
        let last = n + 1 == names.len();
        if t.get(name).is_none() {
            let v = if last && array { Value::Array(Vec::new()) } else { Value::Table(Table::default()) };
            t.insert(name.clone(), v, line)?;
        } else if last && !array {
            return Err(format!("line {line}: table `{}` defined twice", names.join(".")));
        }
        let v = t.get_mut(name).unwrap();
        t = match v {
            Value::Table(t) => t,
            Value::Array(a) => {
                if last && array {
                    a.push(Value::Table(Table::default()));
                }
                match a.last_mut() {
                    Some(Value::Table(t)) => t,
                    _ => return Err(format!("line {line}: `{name}` is not an array of tables")),
                }
            }
            _ => return Err(format!("line {line}: `{name}` is not a table")),
        };
    }
    Ok(())
}

fn current<'a>(root: &'a mut Table, path: &[String]) -> Option<&'a mut Table> {
    let mut t = root;
    for name in path {
        t = match t.get_mut(name)? {
            Value::Table(t) => t,
            Value::Array(a) => match a.last_mut()? {
                Value::Table(t) => t,
                _ => return None,
            },
            _ => return None,
        };
    }
    Some(t)
}

impl P<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn err<T>(&self, m: &str) -> Result<T, String> {
        Err(format!("line {}: {m}", self.line))
    }
    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            self.err(&format!("expected `{}`", c as char))
        }
    }
    fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.i += 1;
        }
    }
    fn skip_comment(&mut self) {
        if self.peek() == Some(b'#') {
            while !matches!(self.peek(), None | Some(b'\n')) {
                self.i += 1;
            }
        }
    }
    fn skip_ws_comments_newlines(&mut self) {
        loop {
            self.skip_spaces();
            self.skip_comment();
            match self.peek() {
                Some(b'\n') => {
                    self.i += 1;
                    self.line += 1;
                }
                Some(b'\r') if self.s.get(self.i + 1) == Some(&b'\n') => self.i += 1,
                _ => break,
            }
        }
    }
    fn end_of_line(&mut self) -> Result<(), String> {
        self.skip_spaces();
        self.skip_comment();
        if self.peek() == Some(b'\r') {
            self.i += 1;
        }
        match self.peek() {
            None => Ok(()),
            Some(b'\n') => {
                self.i += 1;
                self.line += 1;
                Ok(())
            }
            _ => self.err("expected the end of the line"),
        }
    }
    fn header_names(&mut self) -> Result<Vec<String>, String> {
        let mut v = Vec::new();
        loop {
            self.skip_spaces();
            v.push(self.key()?);
            self.skip_spaces();
            if self.peek() == Some(b'.') {
                self.i += 1;
            } else {
                return Ok(v);
            }
        }
    }
    fn key(&mut self) -> Result<String, String> {
        match self.peek() {
            Some(b'"') => self.basic_string(),
            Some(b'\'') => self.literal_string(),
            _ => {
                let st = self.i;
                while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == b'_' || c == b'-') {
                    self.i += 1;
                }
                if st == self.i {
                    return self.err("expected a key");
                }
                Ok(String::from_utf8(self.s[st..self.i].to_vec()).unwrap())
            }
        }
    }
    fn basic_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        if self.s[self.i..].starts_with(b"\"\"") {
            return self.err("multi-line strings are not supported");
        }
        let mut o = Vec::new();
        loop {
            match self.peek() {
                None | Some(b'\n') => return self.err("unterminated string"),
                Some(b'"') => {
                    self.i += 1;
                    break;
                }
                Some(b'\\') => {
                    self.i += 1;
                    let c = self.peek().ok_or("line end in an escape")?;
                    self.i += 1;
                    match c {
                        b'"' => o.push(b'"'),
                        b'\\' => o.push(b'\\'),
                        b'n' => o.push(b'\n'),
                        b't' => o.push(b'\t'),
                        b'u' => {
                            let h = std::str::from_utf8(self.s.get(self.i..self.i + 4).ok_or("short \\u escape")?).map_err(|e| e.to_string())?;
                            let cp = u32::from_str_radix(h, 16).map_err(|_| format!("line {}: bad \\u escape", self.line))?;
                            let ch = char::from_u32(cp).ok_or("bad code point")?;
                            let mut b = [0; 4];
                            o.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                            self.i += 4;
                        }
                        _ => return self.err("unsupported escape"),
                    }
                }
                Some(c) => {
                    o.push(c);
                    self.i += 1;
                }
            }
        }
        String::from_utf8(o).map_err(|e| e.to_string())
    }
    fn literal_string(&mut self) -> Result<String, String> {
        self.expect(b'\'')?;
        let st = self.i;
        while !matches!(self.peek(), None | Some(b'\'' | b'\n')) {
            self.i += 1;
        }
        if self.peek() != Some(b'\'') {
            return self.err("unterminated string");
        }
        let s = String::from_utf8(self.s[st..self.i].to_vec()).map_err(|e| e.to_string())?;
        self.i += 1;
        Ok(s)
    }
    fn value(&mut self) -> Result<Value, String> {
        match self.peek() {
            Some(b'"') => self.basic_string().map(Value::Str),
            Some(b'\'') => self.literal_string().map(Value::Str),
            Some(b'[') => self.array(),
            Some(b'{') => self.inline_table(),
            Some(b't') if self.s[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(Value::Bool(true))
            }
            Some(b'f') if self.s[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(Value::Bool(false))
            }
            Some(c) if c == b'-' || c == b'+' || c.is_ascii_digit() => self.number(),
            _ => self.err("expected a value"),
        }
    }
    fn number(&mut self) -> Result<Value, String> {
        let st = self.i;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.' | b'_')) {
            self.i += 1;
        }
        let t = std::str::from_utf8(&self.s[st..self.i]).unwrap();
        let digits = t.trim_start_matches(['+', '-']);
        let ok_chars = digits.bytes().all(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-'));
        if !ok_chars || digits.is_empty() || !digits.as_bytes()[0].is_ascii_digit() {
            return self.err(&format!("unsupported number `{t}`"));
        }
        let int_part = digits.split(['.', 'e', 'E']).next().unwrap();
        if int_part.len() > 1 && int_part.starts_with('0') {
            return self.err(&format!("leading zero in `{t}`"));
        }
        if digits.contains(['.', 'e', 'E']) {
            if digits.contains('.') {
                let frac = digits.split('.').nth(1).unwrap();
                if !frac.as_bytes().first().is_some_and(|c| c.is_ascii_digit()) {
                    return self.err(&format!("a point needs digits on both sides in `{t}`"));
                }
            }
            t.parse::<f64>().map(Value::Float).map_err(|_| format!("line {}: bad float `{t}`", self.line))
        } else {
            t.parse::<i64>().map(Value::Int).map_err(|_| format!("line {}: bad integer `{t}`", self.line))
        }
    }
    fn array(&mut self) -> Result<Value, String> {
        self.expect(b'[')?;
        let mut v = Vec::new();
        loop {
            self.skip_ws_comments_newlines();
            if self.peek() == Some(b']') {
                self.i += 1;
                return Ok(Value::Array(v));
            }
            v.push(self.value()?);
            self.skip_ws_comments_newlines();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {}
                _ => return self.err("expected `,` or `]` in an array"),
            }
        }
    }
    fn inline_table(&mut self) -> Result<Value, String> {
        self.expect(b'{')?;
        let mut t = Table::default();
        self.skip_spaces();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Value::Table(t));
        }
        loop {
            self.skip_spaces();
            let line = self.line;
            let k = self.key()?;
            self.skip_spaces();
            self.expect(b'=')?;
            self.skip_spaces();
            let v = self.value()?;
            t.insert(k, v, line)?;
            self.skip_spaces();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Table(t));
                }
                _ => return self.err("expected `,` or `}` in an inline table"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_subset() {
        let t = parse(
            r#"
# comment
format = 1
name = "a \"b\"" # trailing
lit = 'c:\d'
f = [0.42054, -1.5e-3, 2.0,
     3, # a comment in an array
]
it = { a = 1, "b c" = true }
[source.page]
url = "https://x"
[[value]]
key = "a"
[[value.obs]]
evidence = "official"
[[value.obs]]
evidence = "measured"
[[value]]
key = "b"
"#,
        )
        .unwrap();
        assert_eq!(t.get("format"), Some(&Value::Int(1)));
        assert_eq!(t.get("name").unwrap().as_str(), Some("a \"b\""));
        assert_eq!(t.get("lit").unwrap().as_str(), Some("c:\\d"));
        let f = t.get("f").unwrap().as_array().unwrap();
        assert_eq!(f[0], Value::Float(0.42054));
        assert_eq!(f[1], Value::Float(-0.0015));
        assert_eq!(f[3], Value::Int(3));
        let it = t.get("it").unwrap().as_table().unwrap();
        assert_eq!(it.get("b c"), Some(&Value::Bool(true)));
        let src = t.get("source").unwrap().as_table().unwrap().get("page").unwrap().as_table().unwrap();
        assert_eq!(src.get("url").unwrap().as_str(), Some("https://x"));
        let vals = t.get("value").unwrap().as_array().unwrap();
        assert_eq!(vals.len(), 2);
        let obs = vals[0].as_table().unwrap().get("obs").unwrap().as_array().unwrap();
        assert_eq!(obs.len(), 2);
        assert_eq!(obs[1].as_table().unwrap().get("evidence").unwrap().as_str(), Some("measured"));
    }

    #[test]
    fn rejects_outside_the_subset() {
        for bad in ["a = 0x10", "a = 1_000", "a = inf", "a = \"\"\"x\"\"\"", "a.b = 1", "a = 1\na = 2", "a = 01", "a = .5", "a = 1."] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn floats_round_trip() {
        for x in [0.42054, 1.0, 0.0, 1e-7, 2.064174, 911.791] {
            assert_eq!(parse(&format!("a = {}", float(x))).unwrap().get("a"), Some(&Value::Float(x)));
        }
    }
}
