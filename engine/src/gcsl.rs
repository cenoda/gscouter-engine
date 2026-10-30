//! gcsim's config language: the lexer, the AST and the parser of
//! pkg/gcs/ast and pkg/gcs/parser, ported to keep its grammar. A config's
//! setup lines (options, target, energy, characters) become a Config; the
//! rest becomes the program the evaluator runs.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

use crate::data;
use crate::types::*;

// ---------------------------------------------------------------- tokens

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Tok {
    Error,
    Eof,
    TerminateLine,
    Assign,
    Comma,
    LeftParen,
    RightParen,
    LeftSquareParen,
    RightSquareParen,
    LeftBrace,
    RightBrace,
    Colon,
    Plus,
    Minus,
    Asterisk,
    ForwardSlash,
    LogicNot,
    LogicAnd,
    LogicOr,
    OpEqual,
    OpNotEqual,
    OpGreaterThan,
    OpGreaterThanOrEqual,
    OpLessThan,
    OpLessThanOrEqual,
    Dot,
    Field,
    Identifier,
    Number,
    Bool,
    String,
    // keywords
    Let,
    While,
    If,
    Else,
    Fn,
    Switch,
    Case,
    Default,
    Break,
    Continue,
    Fallthrough,
    Return,
    For,
    Options,
    Add,
    Char,
    Stats,
    Weapon,
    Set,
    Lvl,
    Refine,
    Cons,
    Talent,
    Count,
    Params,
    Label,
    Until,
    Active,
    Target,
    Resist,
    Energy,
    ParticleThreshold,
    ParticleDropCount,
    ParticleElement,
    Hurt,
    // keys
    StatKey,
    ElementKey,
    CharacterKey,
    ActionKey,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub typ: Tok,
    pub val: String,
    pub line: u32,
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Prec {
    Lowest = 1,
    LogicalOr,
    LogicalAnd,
    Equals,
    LessOrGreater,
    Sum,
    Product,
    Prefix,
    Call,
}

fn precedence(t: Tok) -> Prec {
    match t {
        Tok::LogicOr => Prec::LogicalOr,
        Tok::LogicAnd => Prec::LogicalAnd,
        Tok::OpEqual | Tok::OpNotEqual => Prec::Equals,
        Tok::OpLessThan | Tok::OpGreaterThan | Tok::OpLessThanOrEqual | Tok::OpGreaterThanOrEqual => Prec::LessOrGreater,
        Tok::Plus | Tok::Minus => Prec::Sum,
        Tok::ForwardSlash | Tok::Asterisk => Prec::Product,
        Tok::LeftParen => Prec::Call,
        _ => Prec::Lowest,
    }
}

fn keyword(w: &str) -> Option<Tok> {
    Some(match w {
        "let" => Tok::Let,
        "while" => Tok::While,
        "if" => Tok::If,
        "else" => Tok::Else,
        "fn" => Tok::Fn,
        "switch" => Tok::Switch,
        "case" => Tok::Case,
        "default" => Tok::Default,
        "break" => Tok::Break,
        "continue" => Tok::Continue,
        "fallthrough" => Tok::Fallthrough,
        "return" => Tok::Return,
        "for" => Tok::For,
        "options" => Tok::Options,
        "add" => Tok::Add,
        "char" => Tok::Char,
        "stats" => Tok::Stats,
        "weapon" => Tok::Weapon,
        "set" => Tok::Set,
        "lvl" => Tok::Lvl,
        "refine" => Tok::Refine,
        "cons" => Tok::Cons,
        "talent" => Tok::Talent,
        "count" => Tok::Count,
        "params" => Tok::Params,
        "label" => Tok::Label,
        "until" => Tok::Until,
        "active" => Tok::Active,
        "target" => Tok::Target,
        "particle_threshold" => Tok::ParticleThreshold,
        "particle_drop_count" => Tok::ParticleDropCount,
        "particle_element" => Tok::ParticleElement,
        "resist" => Tok::Resist,
        "energy" => Tok::Energy,
        "hurt" => Tok::Hurt,
        _ => return None,
    })
}

pub fn stat_key(s: &str) -> Option<usize> {
    if matches!(s, "base_hp" | "base_atk" | "base_def") {
        return None;
    }
    st::by_name(s)
}

pub fn element_key(s: &str) -> Option<Element> {
    Some(match s {
        "electro" => Element::Electro,
        "pyro" => Element::Pyro,
        "cryo" => Element::Cryo,
        "hydro" => Element::Hydro,
        "anemo" => Element::Anemo,
        "dendro" => Element::Dendro,
        "geo" => Element::Geo,
        "physical" => Element::Physical,
        "none" => Element::NoElement,
        _ => return None,
    })
}

pub fn action_key(s: &str) -> Option<Action> {
    Some(match s {
        "skill" => Action::Skill,
        "burst" => Action::Burst,
        "attack" => Action::Attack,
        "charge" => Action::Charge,
        "high_plunge" => Action::HighPlunge,
        "low_plunge" => Action::LowPlunge,
        "aim" => Action::Aim,
        "dash" => Action::Dash,
        "jump" => Action::Jump,
        "walk" => Action::Walk,
        "swap" => Action::Swap,
        _ => return None,
    })
}

fn names(table: &'static [(&'static str, u16)]) -> HashMap<&'static str, u16> {
    table.iter().copied().collect()
}

/// gcsim's shortcut.CharNameToKey.
pub fn char_key(s: &str) -> Option<u16> {
    static M: OnceLock<HashMap<&'static str, u16>> = OnceLock::new();
    M.get_or_init(|| names(data::CHAR_NAMES)).get(s).copied()
}
pub fn weapon_key(s: &str) -> Option<u16> {
    static M: OnceLock<HashMap<&'static str, u16>> = OnceLock::new();
    M.get_or_init(|| names(data::WEAPON_NAMES)).get(s).copied()
}
pub fn set_key(s: &str) -> Option<u16> {
    static M: OnceLock<HashMap<&'static str, u16>> = OnceLock::new();
    M.get_or_init(|| names(data::SET_NAMES)).get(s).copied()
}
fn key_name(table: &'static [(u16, &'static str)], k: u16) -> &'static str {
    table.iter().find(|x| x.0 == k).map(|x| x.1).unwrap_or("invalid")
}
pub fn char_name(k: u16) -> &'static str {
    key_name(data::CHAR_KEYS, k)
}
pub fn weapon_name(k: u16) -> &'static str {
    key_name(data::WEAPON_KEYS, k)
}
pub fn set_name(k: u16) -> &'static str {
    key_name(data::SET_KEYS, k)
}

// ---------------------------------------------------------------- lexer

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

fn is_alnum(c: char) -> bool {
    c == '_' || c == '-' || c.is_alphabetic() || c.is_numeric() || c == '%'
}

struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    start: usize,
    line: u32,
    start_line: u32,
    paren: i32,
    sq: i32,
    brace: i32,
    out: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn next(&mut self) -> Option<char> {
        let c = self.input[self.pos..].chars().next()?;
        self.pos += c.len_utf8();
        if c == '\n' {
            self.line += 1;
        }
        Some(c)
    }
    fn backup(&mut self, c: Option<char>) {
        if let Some(c) = c {
            self.pos -= c.len_utf8();
            if c == '\n' {
                self.line -= 1;
            }
        }
    }
    fn peek(&mut self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }
    fn emit(&mut self, t: Tok) {
        self.out.push(Token { typ: t, val: self.input[self.start..self.pos].to_string(), line: self.start_line });
        self.ignore();
    }
    fn ignore(&mut self) {
        self.start = self.pos;
        self.start_line = self.line;
    }
    fn error(&mut self, msg: String) -> bool {
        self.out.push(Token { typ: Tok::Error, val: msg, line: self.start_line });
        false
    }
    fn at_terminator(&mut self) -> bool {
        match self.peek() {
            None => true,
            Some(c) if is_space(c) => true,
            Some(c) => matches!(c, '.' | ',' | '|' | ':' | ')' | '(' | '+' | '=' | '>' | '<' | '&' | '!' | ';' | '[' | ']' | '{' | '}' | '/' | '*'),
        }
    }

    /// lexText; returns false when lexing stops.
    fn text(&mut self) -> bool {
        let Some(r) = self.next() else {
            self.emit(Tok::Eof);
            return false;
        };
        match r {
            ';' => self.emit(Tok::TerminateLine),
            ':' => self.emit(Tok::Colon),
            c if is_space(c) => self.ignore(),
            '#' => {
                self.ignore();
                self.comment();
            }
            '=' => {
                let n = self.next();
                if n == Some('=') {
                    self.emit(Tok::OpEqual);
                } else {
                    self.backup(n);
                    self.emit(Tok::Assign);
                }
            }
            ',' => self.emit(Tok::Comma),
            '*' => self.emit(Tok::Asterisk),
            '+' => self.emit(Tok::Plus),
            '/' => {
                let n = self.next();
                if n == Some('/') {
                    self.ignore();
                    self.comment();
                } else {
                    self.backup(n);
                    self.emit(Tok::ForwardSlash);
                }
            }
            '.' => {
                let next_digit = self.input[self.pos..].bytes().next().is_some_and(|b| b.is_ascii_digit());
                if !next_digit && self.pos < self.input.len() {
                    return self.field();
                }
                self.backup(Some(r));
                self.number();
            }
            '0'..='9' => {
                self.backup(Some(r));
                self.number();
            }
            '-' => self.emit(Tok::Minus),
            '>' => {
                let n = self.next();
                if n == Some('=') {
                    self.emit(Tok::OpGreaterThanOrEqual);
                } else {
                    self.backup(n);
                    self.emit(Tok::OpGreaterThan);
                }
            }
            '<' => match self.next() {
                Some('=') => self.emit(Tok::OpLessThanOrEqual),
                Some('>') => self.emit(Tok::OpNotEqual),
                n => {
                    self.backup(n);
                    self.emit(Tok::OpLessThan);
                }
            },
            '|' => {
                if self.next() == Some('|') {
                    self.emit(Tok::LogicOr);
                } else {
                    return self.error(format!("unrecognized character in action: {r:?}"));
                }
            }
            '!' => {
                let n = self.next();
                if n == Some('=') {
                    self.emit(Tok::OpNotEqual);
                } else {
                    self.backup(n);
                    self.emit(Tok::LogicNot);
                }
            }
            '"' => return self.quote(),
            '&' => {
                if self.next() == Some('&') {
                    self.emit(Tok::LogicAnd);
                } else {
                    return self.error(format!("unrecognized character in action: {r:?}"));
                }
            }
            '(' => {
                self.emit(Tok::LeftParen);
                self.paren += 1;
            }
            ')' => {
                self.emit(Tok::RightParen);
                self.paren -= 1;
                if self.paren < 0 {
                    return self.error("unexpected right paren".into());
                }
            }
            '[' => {
                self.emit(Tok::LeftSquareParen);
                self.sq += 1;
            }
            ']' => {
                self.emit(Tok::RightSquareParen);
                self.sq -= 1;
                if self.sq < 0 {
                    return self.error("unexpected right sq paren".into());
                }
            }
            '{' => {
                self.emit(Tok::LeftBrace);
                self.brace += 1;
            }
            '}' => {
                self.emit(Tok::RightBrace);
                self.brace -= 1;
                if self.brace < 0 {
                    return self.error("unexpected right brace".into());
                }
            }
            c if is_alnum(c) => {
                self.backup(Some(c));
                return self.identifier();
            }
            c => return self.error(format!("unrecognized character in action: {c:?}")),
        }
        true
    }

    fn comment(&mut self) {
        loop {
            match self.next() {
                None => break,
                Some('\n') => {
                    self.backup(Some('\n'));
                    break;
                }
                _ => {}
            }
        }
    }

    fn field(&mut self) -> bool {
        if self.at_terminator() {
            self.emit(Tok::Dot);
            return true;
        }
        let mut last = None;
        loop {
            let r = self.next();
            match r {
                Some(c) if is_alnum(c) => {}
                _ => {
                    self.backup(r);
                    last = r;
                    break;
                }
            }
        }
        if !self.at_terminator() {
            return self.error(format!("bad character {last:?}"));
        }
        self.emit(Tok::Field);
        true
    }

    fn quote(&mut self) -> bool {
        loop {
            match self.next() {
                Some('\\') => match self.next() {
                    Some(c) if c != '\n' => {}
                    _ => return self.error("unterminated quoted string".into()),
                },
                None | Some('\n') => return self.error("unterminated quoted string".into()),
                Some('"') => break,
                _ => {}
            }
        }
        self.emit(Tok::String);
        true
    }

    fn identifier(&mut self) -> bool {
        loop {
            let r = self.next();
            match r {
                Some(c) if is_alnum(c) => {}
                _ => {
                    self.backup(r);
                    let word = &self.input[self.start..self.pos];
                    if !self.at_terminator() {
                        return self.error(format!("bad character {r:?}"));
                    }
                    let t = if let Some(k) = keyword(word) {
                        k
                    } else if word.starts_with('.') {
                        Tok::Field
                    } else if word == "true" || word == "false" {
                        Tok::Bool
                    } else if stat_key(word).is_some() {
                        Tok::StatKey
                    } else if element_key(word).is_some() {
                        Tok::ElementKey
                    } else if char_key(word).is_some() {
                        Tok::CharacterKey
                    } else if action_key(word).is_some() {
                        Tok::ActionKey
                    } else {
                        Tok::Identifier
                    };
                    self.emit(t);
                    return true;
                }
            }
        }
    }

    fn number(&mut self) {
        let accept = |l: &mut Self, valid: &str| -> bool {
            match l.next() {
                Some(c) if valid.contains(c) => true,
                r => {
                    l.backup(r);
                    false
                }
            }
        };
        accept(self, "+-");
        while accept(self, "0123456789") {}
        if accept(self, ".") {
            while accept(self, "0123456789") {}
        }
        self.emit(Tok::Number);
    }
}

pub fn lex(input: &str) -> Vec<Token> {
    let mut l = Lexer { input, pos: 0, start: 0, line: 1, start_line: 1, paren: 0, sq: 0, brace: 0, out: Vec::new() };
    while l.text() {}
    l.out
}

// ---------------------------------------------------------------- AST

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Num {
    Int(i64),
    Float(f64),
}

impl Num {
    pub fn f(self) -> f64 {
        match self {
            Num::Int(i) => i as f64,
            Num::Float(f) => f,
        }
    }
    pub fn i(self) -> i64 {
        match self {
            Num::Int(i) => i,
            Num::Float(f) => f as i64,
        }
    }
    pub fn truthy(self) -> bool {
        match self {
            Num::Int(i) => i != 0,
            Num::Float(f) => f != 0.0,
        }
    }
    pub fn from_bool(b: bool) -> Num {
        Num::Int(b as i64)
    }
    pub fn inspect(self) -> String {
        match self {
            Num::Int(i) => i.to_string(),
            Num::Float(f) => format!("{f}"),
        }
    }
}

#[derive(Debug)]
pub enum Expr {
    Num(Num),
    Str(String),
    Func(Rc<FuncLit>),
    Ident(String),
    Field(Vec<String>),
    Call(Box<Expr>, Vec<Expr>, u32),
    Unary(Tok, Box<Expr>),
    Binary(Tok, Box<Expr>, Box<Expr>),
    Map(Vec<(String, Expr)>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ctrl {
    Break,
    Continue,
    Fallthrough,
}

#[derive(Debug)]
pub enum Stmt {
    Block(Block),
    Let(String, Expr),
    Assign(String, Expr),
    Return(Expr),
    Ctrl(Ctrl),
    If(Expr, Block, Option<Box<Stmt>>),
    Switch(Option<Expr>, Vec<(Expr, Block)>, Option<Block>),
    Fn(String, Rc<FuncLit>),
    While(Expr, Block),
    For(Option<Box<Stmt>>, Option<Expr>, Option<Box<Stmt>>, Block),
    Expr(Expr),
}

#[derive(Debug, Default)]
pub struct Block(pub Vec<Stmt>);

#[derive(Debug)]
pub struct FuncLit {
    pub args: Vec<String>,
    /// The result type is number (the default).
    pub returns_number: bool,
    pub body: Block,
}

// ---------------------------------------------------------------- config

#[derive(Clone, Debug, Default)]
pub struct WeaponProfile {
    pub key: u16,
    pub level: i32,
    pub max_level: i32,
    pub refine: i32,
    pub params: Vec<(String, i64)>,
}

#[derive(Clone, Debug)]
pub struct CharProfile {
    pub key: u16,
    pub level: i32,
    pub max_level: i32,
    pub cons: i32,
    pub talents: [i32; 3],
    pub params: Vec<(String, i64)>,
    pub weapon: WeaponProfile,
    pub sets: Vec<(u16, i32)>,
    pub stats: Stats,
}

#[derive(Clone, Debug, Default)]
pub struct TargetProfile {
    pub level: i32,
    /// Resistance by element, in Element order.
    pub resist: [f64; 11],
    pub hp: f64,
    pub pos: (f64, f64),
    pub radius: f64,
    pub typed: bool,
    pub particle_threshold: f64,
    pub particle_drop_count: f64,
}

#[derive(Clone, Debug, Default)]
pub struct EnergySettings {
    pub active: bool,
    pub once: bool,
    pub start: i32,
    pub end: i32,
    pub amount: i32,
}

#[derive(Debug)]
pub struct ParsedConfig {
    pub chars: Vec<CharProfile>,
    pub active: Option<u16>,
    pub targets: Vec<TargetProfile>,
    pub energy: EnergySettings,
    pub hurt: bool,
    pub iterations: i32,
    pub duration: f64,
    pub swap_delay: i32,
    pub hitlag: bool,
    pub defhalt: bool,
    pub ignore_burst_energy: bool,
    pub damage_mode: bool,
    pub program: Block,
}

// ---------------------------------------------------------------- parser

pub struct Parser {
    toks: Vec<Token>,
    pos: isize,
    cfg: ParsedConfig,
    current: u16,
}

type R<T> = Result<T, String>;

impl Parser {
    pub fn new(input: &str) -> Parser {
        Parser {
            toks: lex(input),
            pos: -1,
            cfg: ParsedConfig {
                chars: Vec::new(),
                active: None,
                targets: Vec::new(),
                energy: EnergySettings::default(),
                hurt: false,
                iterations: 1000,
                duration: 0.0,
                swap_delay: 1,
                hitlag: true,
                defhalt: true,
                ignore_burst_energy: false,
                damage_mode: false,
                program: Block::default(),
            },
            current: 0,
        }
    }

    fn next(&mut self) -> Token {
        self.pos += 1;
        let i = (self.pos as usize).min(self.toks.len() - 1);
        self.toks[i].clone()
    }
    fn backup(&mut self) {
        self.pos -= 1;
        if self.pos < -1 {
            self.pos = -1;
        }
    }
    fn peek(&mut self) -> Token {
        let n = self.next();
        self.backup();
        n
    }
    fn consume(&mut self, t: Tok) -> R<Token> {
        let n = self.next();
        if n.typ == Tok::Error {
            return Err(format!("ln{}: {}", n.line, n.val));
        }
        if n.typ != t {
            return Err(format!("ln{}: expecting {t:?}, got {:?} {:?}", n.line, n.typ, n.val));
        }
        Ok(n)
    }
    fn seq(&mut self, ts: &[Tok]) -> R<Token> {
        let mut last = None;
        for t in ts {
            last = Some(self.consume(*t)?);
        }
        Ok(last.unwrap())
    }
    fn int(t: &Token) -> R<i64> {
        t.val.parse::<i64>().map_err(|e| format!("ln{}: {e}", t.line))
    }
    fn float(t: &Token) -> R<f64> {
        t.val.parse::<f64>().map_err(|e| format!("ln{}: {e}", t.line))
    }

    pub fn parse(mut self) -> R<ParsedConfig> {
        loop {
            let n = self.peek();
            match n.typ {
                Tok::Error => return Err(format!("ln{}: {}", n.line, n.val)),
                Tok::CharacterKey => {
                    self.next();
                    if self.peek().typ != Tok::ActionKey {
                        let key = char_key(&n.val).unwrap();
                        if !self.cfg.chars.iter().any(|c| c.key == key) {
                            self.new_char(key);
                        }
                        self.current = key;
                        self.parse_character()?;
                        continue;
                    }
                    self.backup();
                    let s = self.parse_statement()?;
                    self.cfg.program.0.push(s);
                }
                Tok::Active => {
                    self.next();
                    let c = self.consume(Tok::CharacterKey).map_err(|_| format!("ln{}: setting active char: invalid char", n.line))?;
                    self.cfg.active = char_key(&c.val);
                    self.consume(Tok::TerminateLine)?;
                }
                Tok::Target => {
                    self.next();
                    self.parse_target()?;
                }
                Tok::Energy => {
                    self.next();
                    self.parse_energy()?;
                }
                Tok::Hurt => {
                    self.next();
                    self.cfg.hurt = true;
                    // skip to the end of the line; the engine refuses hurt
                    while !matches!(self.next().typ, Tok::TerminateLine | Tok::Eof) {}
                }
                Tok::Options => {
                    self.next();
                    self.parse_options()?;
                }
                Tok::Eof => break,
                Tok::ActionKey => return Err(format!("ln{}: unexpected line starts with an action: {}", n.line, n.val)),
                _ => {
                    let s = self.parse_statement()?;
                    self.cfg.program.0.push(s);
                }
            }
        }
        let c = &self.cfg;
        if c.chars.len() > 4 {
            return Err(format!("config contains a total of {} characters; cannot exceed 4", c.chars.len()));
        }
        match c.active {
            None => return Err("config does not contain active char".into()),
            Some(a) if !c.chars.iter().any(|x| x.key == a) => return Err(format!("active char {} not found in team", char_name(a))),
            _ => {}
        }
        if c.targets.is_empty() {
            return Err("config does not contain any targets".into());
        }
        for t in &mut self.cfg.targets {
            if t.radius == 0.0 {
                t.radius = 1.0;
            }
        }
        Ok(self.cfg)
    }

    // ------------------------------------------------------------ setup lines

    fn new_char(&mut self, key: u16) {
        self.cfg.chars.push(CharProfile {
            key,
            level: 0,
            max_level: 0,
            cons: 0,
            talents: [0; 3],
            params: Vec::new(),
            weapon: WeaponProfile::default(),
            sets: Vec::new(),
            stats: [0.0; N_STATS],
        });
    }

    fn cur(&mut self) -> &mut CharProfile {
        let k = self.current;
        self.cfg.chars.iter_mut().find(|c| c.key == k).unwrap()
    }

    fn parse_character(&mut self) -> R<()> {
        let n = self.next();
        match n.typ {
            Tok::Char => self.parse_char_details(),
            Tok::Add => {
                let n = self.next();
                match n.typ {
                    Tok::Weapon => self.parse_add_weapon(),
                    Tok::Set => self.parse_add_set(),
                    Tok::Stats => self.parse_add_stats(),
                    _ => Err(format!("ln{}: unexpected token after add: {}", n.line, n.val)),
                }
            }
            _ => Err(format!("ln{}: unexpected token after character: {}", n.line, n.val)),
        }
    }

    fn level(&mut self) -> R<(i32, i32)> {
        self.consume(Tok::Assign)?;
        let base = Self::int(&self.consume(Tok::Number)?)? as i32;
        self.consume(Tok::ForwardSlash)?;
        let x = self.consume(Tok::Number)?;
        let max = Self::int(&x)? as i32;
        if max < base {
            return Err(format!("ln{}: max level {max} cannot be less than base level {base}", x.line));
        }
        Ok((base, max))
    }

    fn int_map(&mut self) -> R<Vec<(String, i64)>> {
        if self.peek().typ != Tok::LeftSquareParen {
            return Ok(Vec::new());
        }
        let Expr::Map(fields) = self.parse_map()? else { unreachable!() };
        fields
            .into_iter()
            .map(|(k, v)| match v {
                Expr::Num(n) => Ok((k, n.i())),
                other => Err(format!("expected number in the map, got {other:?}")),
            })
            .collect()
    }

    fn plus_params(&mut self) -> R<Vec<(String, i64)>> {
        let n = self.next();
        if n.typ != Tok::Params {
            return Err(format!("ln{}: unexpected token after +: {}", n.line, n.val));
        }
        self.seq(&[Tok::Assign, Tok::LeftSquareParen])?;
        self.backup();
        self.int_map()
    }

    fn parse_char_details(&mut self) -> R<()> {
        loop {
            let n = self.next();
            match n.typ {
                Tok::Eof => return Err("unexpected end of line while parsing character".into()),
                Tok::Lvl => {
                    let (b, m) = self.level()?;
                    let c = self.cur();
                    c.level = b;
                    c.max_level = m;
                }
                Tok::Cons => {
                    let x = self.seq(&[Tok::Assign, Tok::Number])?;
                    self.cur().cons = Self::int(&x)? as i32;
                }
                Tok::Talent => {
                    let a = Self::int(&self.seq(&[Tok::Assign, Tok::Number])?)? as i32;
                    let s = Self::int(&self.seq(&[Tok::Comma, Tok::Number])?)? as i32;
                    let b = Self::int(&self.seq(&[Tok::Comma, Tok::Number])?)? as i32;
                    self.cur().talents = [a, s, b];
                }
                Tok::Plus => {
                    let p = self.plus_params()?;
                    self.cur().params = p;
                }
                Tok::TerminateLine => return Ok(()),
                Tok::Error => return Err(format!("ln{}: {}", n.line, n.val)),
                _ => return Err(format!("ln{}: unrecognized token parsing character: {}", n.line, n.val)),
            }
        }
    }

    fn quoted(t: &Token) -> String {
        t.val.trim_start_matches('"').trim_end_matches('"').to_string()
    }

    fn parse_add_set(&mut self) -> R<()> {
        let x = self.seq(&[Tok::Assign, Tok::String])?;
        let s = Self::quoted(&x);
        let key = set_key(&s).ok_or_else(|| format!("invalid set {s}"))?;
        let mut count = 0;
        loop {
            let n = self.next();
            match n.typ {
                Tok::Eof => return Err("unexpected end of line while parsing character add set".into()),
                Tok::Count => count = Self::int(&self.seq(&[Tok::Assign, Tok::Number])?)? as i32,
                Tok::Plus => {
                    self.plus_params()?;
                }
                Tok::TerminateLine => {
                    let c = self.cur();
                    match c.sets.iter_mut().find(|s| s.0 == key) {
                        Some(s) => s.1 = count,
                        None => c.sets.push((key, count)),
                    }
                    return Ok(());
                }
                _ => return Err(format!("ln{}: unrecognized token parsing add set: {}", n.line, n.val)),
            }
        }
    }

    fn parse_add_weapon(&mut self) -> R<()> {
        let x = self.seq(&[Tok::Assign, Tok::String])?;
        let s = Self::quoted(&x);
        let key = weapon_key(&s).ok_or_else(|| format!("invalid weapon {s}"))?;
        self.cur().weapon.key = key;
        let (mut lvl, mut refine) = (false, false);
        loop {
            let n = self.next();
            match n.typ {
                Tok::Eof => return Err("unexpected end of line while parsing character add weapon".into()),
                Tok::Lvl => {
                    let (b, m) = self.level()?;
                    let w = &mut self.cur().weapon;
                    w.level = b;
                    w.max_level = m;
                    lvl = true;
                }
                Tok::Refine => {
                    let r = Self::int(&self.seq(&[Tok::Assign, Tok::Number])?)? as i32;
                    self.cur().weapon.refine = r;
                    refine = true;
                }
                Tok::Plus => {
                    let p = self.plus_params()?;
                    self.cur().weapon.params = p;
                }
                Tok::TerminateLine => {
                    if !lvl {
                        return Err(format!("ln{}: weapon {s} missing lvl", n.line));
                    }
                    if !refine {
                        return Err(format!("ln{}: weapon {s} missing refine", n.line));
                    }
                    return Ok(());
                }
                _ => return Err(format!("ln{}: unrecognized token parsing add weapon: {}", n.line, n.val)),
            }
        }
    }

    fn parse_add_stats(&mut self) -> R<()> {
        let mut line = [0.0; N_STATS];
        loop {
            let n = self.next();
            match n.typ {
                Tok::Eof => return Err("unexpected end of line while parsing character add stats".into()),
                Tok::StatKey => {
                    self.consume(Tok::Assign)?;
                    let amt = self.float_const()?;
                    line[stat_key(&n.val).unwrap()] += amt;
                }
                Tok::Label => {
                    self.seq(&[Tok::Assign, Tok::Identifier])?;
                }
                Tok::TerminateLine => {
                    let c = self.cur();
                    for (i, v) in line.iter().enumerate() {
                        c.stats[i] += v;
                    }
                    return Ok(());
                }
                Tok::Identifier if n.val == "random" => return Err(format!("ln{}: random substats are not supported", n.line)),
                _ => return Err(format!("ln{}: unrecognized token parsing add stats: {}", n.line, n.val)),
            }
        }
    }

    fn float_const(&mut self) -> R<f64> {
        match self.parse_expr(Prec::Lowest)? {
            Expr::Num(n) => Ok(n.f()),
            other => Err(format!("expecting number, got {other:?}")),
        }
    }

    fn parse_options(&mut self) -> R<()> {
        loop {
            let n = self.next();
            match n.typ {
                Tok::Eof => return Err("unexpected end of line while parsing options".into()),
                Tok::TerminateLine => return Ok(()),
                Tok::Identifier => {
                    let bool_opt = |p: &mut Self| -> R<bool> { Ok(p.seq(&[Tok::Assign, Tok::Bool])?.val == "true") };
                    let int_opt = |p: &mut Self| -> R<i64> { Self::int(&p.seq(&[Tok::Assign, Tok::Number])?) };
                    match n.val.as_str() {
                        "debug" => {
                            bool_opt(self)?;
                        }
                        "defhalt" => self.cfg.defhalt = bool_opt(self)?,
                        "hitlag" => self.cfg.hitlag = bool_opt(self)?,
                        "iteration" => self.cfg.iterations = int_opt(self)? as i32,
                        "duration" => self.cfg.duration = Self::float(&self.seq(&[Tok::Assign, Tok::Number])?)?,
                        "workers" => {
                            int_opt(self)?;
                        }
                        "mode" => {
                            self.seq(&[Tok::Assign, Tok::Identifier])?;
                        }
                        "swap_delay" => self.cfg.swap_delay = int_opt(self)? as i32,
                        // only reported by gcsim, never used in a fight
                        "attack_delay" | "charge_delay" | "skill_delay" | "burst_delay" | "jump_delay" | "dash_delay" | "aim_delay" => {
                            int_opt(self)?;
                        }
                        "frame_defaults" => {
                            let v = self.seq(&[Tok::Assign, Tok::Identifier])?;
                            if v.val != "human" {
                                return Err(format!("ln{}: unrecognized option for frame_defaults specified: {}", v.line, v.val));
                            }
                            self.cfg.swap_delay = 8;
                        }
                        "ignore_burst_energy" => self.cfg.ignore_burst_energy = bool_opt(self)?,
                        _ => return Err(format!("ln{}: unrecognized option specified: {}", n.line, n.val)),
                    }
                }
                _ => return Err(format!("ln{}: unrecognized token parsing options: {}", n.line, n.val)),
            }
        }
    }

    fn parse_target(&mut self) -> R<()> {
        let mut r = TargetProfile::default();
        loop {
            let n = self.next();
            match n.typ {
                Tok::Eof => return Err("unexpected end of line while parsing target".into()),
                Tok::Identifier => match n.val.as_str() {
                    "pos" => {
                        self.consume(Tok::Assign)?;
                        let x = self.float_const()?;
                        self.consume(Tok::Comma)?;
                        let y = self.float_const()?;
                        r.pos = (x, y);
                    }
                    "radius" => r.radius = Self::float(&self.seq(&[Tok::Assign, Tok::Number])?)?,
                    "type" => {
                        self.seq(&[Tok::Assign, Tok::Identifier])?;
                        if self.peek().typ == Tok::LeftSquareParen {
                            self.parse_map()?;
                        }
                        r.typed = true;
                        self.cfg.damage_mode = true;
                    }
                    "freeze_resist" => {
                        self.consume(Tok::Assign)?;
                        self.float_const()?;
                    }
                    _ => return Err(format!("<target> bad token at line {}: {}", n.line, n.val)),
                },
                Tok::Lvl => r.level = Self::int(&self.seq(&[Tok::Assign, Tok::Number])?)? as i32,
                Tok::StatKey => {
                    if stat_key(&n.val) != Some(st::HP) {
                        return Err(format!("<target> bad token at line {}: {}", n.line, n.val));
                    }
                    r.hp = Self::float(&self.seq(&[Tok::Assign, Tok::Number])?)?;
                    self.cfg.damage_mode = true;
                }
                Tok::Resist => {
                    self.consume(Tok::Assign)?;
                    let amt = self.float_const()?;
                    for e in [Element::Electro, Element::Cryo, Element::Hydro, Element::Physical, Element::Pyro, Element::Geo, Element::Dendro, Element::Anemo] {
                        r.resist[e as usize] += amt;
                    }
                }
                Tok::ParticleThreshold => r.particle_threshold = Self::float(&self.seq(&[Tok::Assign, Tok::Number])?)?,
                Tok::ParticleDropCount => r.particle_drop_count = Self::float(&self.seq(&[Tok::Assign, Tok::Number])?)?,
                Tok::ParticleElement => {
                    self.seq(&[Tok::Assign, Tok::ElementKey])?;
                }
                Tok::ElementKey => {
                    let e = element_key(&n.val).unwrap();
                    self.consume(Tok::Assign)?;
                    r.resist[e as usize] += self.float_const()?;
                }
                Tok::TerminateLine => {
                    self.cfg.targets.push(r);
                    return Ok(());
                }
                _ => return Err(format!("<target> bad token at line {}: {}", n.line, n.val)),
            }
        }
    }

    fn parse_energy(&mut self) -> R<()> {
        let n = self.next();
        let once = match (n.typ, n.val.as_str()) {
            (Tok::Identifier, "once") => true,
            (Tok::Identifier, "every") => false,
            (Tok::TerminateLine, _) => return Ok(()),
            _ => return Err(format!("ln{}: unrecognized option specified: {}", n.line, n.val)),
        };
        let e = &mut self.cfg.energy;
        e.active = true;
        e.once = once;
        loop {
            let n = self.next();
            match n.typ {
                Tok::Eof => return Err("unexpected end of line while parsing energy event".into()),
                Tok::TerminateLine => return Ok(()),
                Tok::Identifier if n.val == "interval" => {
                    let s = Self::int(&self.seq(&[Tok::Assign, Tok::Number])?)? as i32;
                    self.cfg.energy.start = s;
                    if !once {
                        let e = Self::int(&self.seq(&[Tok::Comma, Tok::Number])?)? as i32;
                        self.cfg.energy.end = e;
                    }
                }
                Tok::Identifier if n.val == "amount" => {
                    self.cfg.energy.amount = Self::int(&self.seq(&[Tok::Assign, Tok::Number])?)? as i32;
                }
                _ => return Err(format!("ln{}: unrecognized token parsing energy event: {}", n.line, n.val)),
            }
        }
    }

    // ------------------------------------------------------------ statements

    fn parse_statement(&mut self) -> R<Stmt> {
        let n = self.peek();
        let mut has_semi = true;
        let stmt = match n.typ {
            Tok::Break | Tok::Continue | Tok::Fallthrough => {
                self.next();
                Stmt::Ctrl(match n.typ {
                    Tok::Break => Ctrl::Break,
                    Tok::Continue => Ctrl::Continue,
                    _ => Ctrl::Fallthrough,
                })
            }
            Tok::Let => self.parse_let()?,
            Tok::CharacterKey => self.parse_action()?,
            Tok::Return => {
                self.next();
                Stmt::Return(self.parse_expr(Prec::Lowest)?)
            }
            Tok::If => {
                has_semi = false;
                self.parse_if()?
            }
            Tok::Switch => {
                has_semi = false;
                self.parse_switch()?
            }
            Tok::Fn => {
                has_semi = false;
                self.next();
                let id = self.next();
                if id.typ != Tok::Identifier {
                    return Err(format!("ln{}: expecting identifier after fn, got {}", id.line, id.val));
                }
                Stmt::Fn(id.val, Rc::new(self.parse_fn()?))
            }
            Tok::While => {
                has_semi = false;
                self.next();
                let cond = self.parse_expr(Prec::Lowest)?;
                Stmt::While(cond, self.parse_block()?)
            }
            Tok::For => {
                has_semi = false;
                self.parse_for()?
            }
            Tok::LeftBrace => {
                has_semi = false;
                Stmt::Block(self.parse_block()?)
            }
            Tok::Identifier => {
                self.next();
                let assign = self.peek().typ == Tok::Assign;
                self.backup();
                if assign { self.parse_assign()? } else { Stmt::Expr(self.parse_expr(Prec::Lowest)?) }
            }
            Tok::Error => return Err(format!("ln{}: {}", n.line, n.val)),
            _ => Stmt::Expr(self.parse_expr(Prec::Lowest)?),
        };
        if has_semi {
            let t = self.next();
            if t.typ != Tok::TerminateLine {
                return Err(format!("ln{}: expecting ; at end of statement, got {}", t.line, t.val));
            }
        }
        Ok(stmt)
    }

    fn parse_assign(&mut self) -> R<Stmt> {
        let id = self.consume(Tok::Identifier)?;
        self.consume(Tok::Assign)?;
        Ok(Stmt::Assign(id.val, self.parse_expr(Prec::Lowest)?))
    }

    fn parse_let(&mut self) -> R<Stmt> {
        self.next();
        let id = self.consume(Tok::Identifier)?;
        if self.peek().typ != Tok::Assign {
            self.parse_typing()?;
        }
        self.consume(Tok::Assign)?;
        Ok(Stmt::Let(id.val, self.parse_expr(Prec::Lowest)?))
    }

    fn parse_if(&mut self) -> R<Stmt> {
        self.next();
        let cond = self.parse_expr(Prec::Lowest)?;
        let n = self.peek();
        if n.typ != Tok::LeftBrace {
            return Err(format!("ln{}: expecting {{ after if, got {}", n.line, n.val));
        }
        let then = self.parse_block()?;
        if self.peek().typ != Tok::Else {
            return Ok(Stmt::If(cond, then, None));
        }
        let e = self.next();
        let block = self.parse_statement_noterm()?;
        match block {
            Stmt::If(..) | Stmt::Block(_) => Ok(Stmt::If(cond, then, Some(Box::new(block)))),
            _ => Err(format!("ln{}: expecting either if or normal block after else", e.line)),
        }
    }

    /// parseStatement for the statement after else, which has no ;.
    fn parse_statement_noterm(&mut self) -> R<Stmt> {
        match self.peek().typ {
            Tok::If => self.parse_if(),
            Tok::LeftBrace => Ok(Stmt::Block(self.parse_block()?)),
            _ => self.parse_statement(),
        }
    }

    fn parse_switch(&mut self) -> R<Stmt> {
        self.next();
        let cond = if self.peek().typ != Tok::LeftBrace { Some(self.parse_expr(Prec::Lowest)?) } else { None };
        let n = self.next();
        if n.typ != Tok::LeftBrace {
            return Err(format!("ln{}: expecting {{ after switch, got {}", n.line, n.val));
        }
        let mut cases = Vec::new();
        let mut default = None;
        loop {
            let n = self.next();
            match n.typ {
                Tok::RightBrace => break,
                Tok::Case => {
                    let c = self.parse_expr(Prec::Lowest)?;
                    if self.peek().typ != Tok::Colon {
                        return Err(format!("ln{}: expecting : after case", n.line));
                    }
                    cases.push((c, self.parse_case_body()?));
                }
                Tok::Default => {
                    if self.peek().typ != Tok::Colon {
                        return Err(format!("ln{}: expecting : after default", n.line));
                    }
                    default = Some(self.parse_case_body()?);
                }
                _ => return Err(format!("ln{}: expecting case or default token, got {}", n.line, n.val)),
            }
        }
        Ok(Stmt::Switch(cond, cases, default))
    }

    fn parse_case_body(&mut self) -> R<Block> {
        self.next();
        let mut b = Block::default();
        loop {
            match self.peek().typ {
                Tok::Default | Tok::Case | Tok::RightBrace => return Ok(b),
                Tok::Eof => return Err("reached end of file without closing }".into()),
                _ => b.0.push(self.parse_statement()?),
            }
        }
    }

    fn exist_var_decl(&mut self) -> bool {
        match self.peek().typ {
            Tok::Let => true,
            Tok::Identifier => {
                self.next();
                let b = self.peek().typ == Tok::Assign;
                self.backup();
                b
            }
            _ => false,
        }
    }

    fn parse_for(&mut self) -> R<Stmt> {
        self.next();
        if self.peek().typ == Tok::LeftBrace {
            return Ok(Stmt::For(None, None, None, self.parse_block()?));
        }
        let mut init = None;
        if self.exist_var_decl() {
            init = Some(Box::new(if self.peek().typ == Tok::Let { self.parse_let()? } else { self.parse_assign()? }));
            let n = self.next();
            if n.typ != Tok::TerminateLine {
                return Err(format!("ln{}: expecting ; after for init", n.line));
            }
        }
        let cond = Some(self.parse_expr(Prec::Lowest)?);
        let mut post = None;
        if self.peek().typ == Tok::TerminateLine {
            self.next();
            if self.peek().typ != Tok::LeftBrace {
                post = Some(Box::new(self.parse_assign()?));
            }
        }
        let n = self.peek();
        if n.typ != Tok::LeftBrace {
            return Err(format!("ln{}: expecting {{ after for, got {}", n.line, n.val));
        }
        Ok(Stmt::For(init, cond, post, self.parse_block()?))
    }

    fn parse_block(&mut self) -> R<Block> {
        let n = self.next();
        if n.typ != Tok::LeftBrace {
            return Err(format!("ln{}: expecting {{, got {}", n.line, n.val));
        }
        let mut b = Block::default();
        loop {
            match self.peek().typ {
                Tok::RightBrace => {
                    self.next();
                    return Ok(b);
                }
                Tok::Eof => return Err("reached end of file without closing }".into()),
                _ => b.0.push(self.parse_statement()?),
            }
        }
    }

    /// A character's action list, compiled to execute_action calls.
    fn parse_action(&mut self) -> R<Stmt> {
        let ch = self.consume(Tok::CharacterKey)?;
        let key = char_key(&ch.val).unwrap();
        let n = self.peek();
        if n.typ != Tok::ActionKey {
            return Err(format!("ln{}: expecting actions for character {}, got {}", n.line, ch.val, n.val));
        }
        let mut b = Block::default();
        loop {
            let n = self.next();
            match n.typ {
                Tok::TerminateLine => {
                    self.backup();
                    break;
                }
                Tok::ActionKey => {
                    let a = action_key(&n.val).unwrap();
                    let params = if self.peek().typ == Tok::LeftSquareParen { self.parse_map()? } else { Expr::Map(Vec::new()) };
                    let Expr::Map(fields) = params else { unreachable!() };
                    let mut count = 1;
                    let c = self.next();
                    if c.typ == Tok::Colon {
                        let x = self.next();
                        if x.typ != Tok::Number {
                            return Err(format!("ln{}: expected a number after : but got {}", x.line, x.val));
                        }
                        count = Self::int(&x)?;
                    } else {
                        self.backup();
                    }
                    for _ in 0..count {
                        // the map is evaluated on each call, as gcsim's shared node is
                        let f: Vec<(String, Expr)> = fields.iter().map(|(k, v)| (k.clone(), clone_const(v))).collect();
                        b.0.push(Stmt::Expr(Expr::Call(
                            Box::new(Expr::Ident("execute_action".into())),
                            vec![Expr::Num(Num::Int(key as i64)), Expr::Num(Num::Int(a as i64)), Expr::Map(f)],
                            n.line,
                        )));
                    }
                    let c = self.next();
                    if c.typ != Tok::Comma {
                        self.backup();
                        break;
                    }
                }
                _ => return Err(format!("ln{}: expecting actions for character {}, got {}", n.line, ch.val, n.val)),
            }
        }
        Ok(Stmt::Block(b))
    }

    // ------------------------------------------------------------ functions and types

    fn parse_fn(&mut self) -> R<FuncLit> {
        let n = self.peek();
        if n.typ != Tok::LeftParen {
            return Err(format!("ln{}: expecting ( after identifier, got {}", n.line, n.val));
        }
        self.next();
        let mut args: Vec<String> = Vec::new();
        loop {
            let n = self.next();
            if n.typ == Tok::RightParen {
                break;
            }
            if n.typ != Tok::Identifier {
                return Err(format!("ln{}: expecting identifier in param list, got {}", n.line, n.val));
            }
            if args.contains(&n.val) {
                return Err(format!("ln{}: fn contains duplicated param name {}", n.line, n.val));
            }
            args.push(n.val);
            if matches!(self.peek().typ, Tok::Identifier | Tok::Fn) {
                self.parse_typing()?;
            }
            if self.peek().typ == Tok::Comma {
                self.next();
                let l = self.peek();
                if l.typ != Tok::Identifier {
                    return Err(format!("ln{}: expecting another identifier after comma in param list, got {}", l.line, l.val));
                }
            }
        }
        let mut returns_number = true;
        if self.peek().typ != Tok::LeftBrace {
            returns_number = self.parse_typing()?;
        }
        Ok(FuncLit { args, returns_number, body: self.parse_block()? })
    }

    /// Parses a type; returns whether it is number.
    fn parse_typing(&mut self) -> R<bool> {
        let n = self.next();
        match n.typ {
            Tok::Identifier => match n.val.as_str() {
                "number" => Ok(true),
                "string" | "map" => Ok(false),
                _ => Err(format!("ln{}: unexpected basic type parsing type info; got {}", n.line, n.val)),
            },
            Tok::Fn => {
                self.consume(Tok::LeftParen)?;
                if self.peek().typ == Tok::RightParen {
                    self.next();
                } else {
                    loop {
                        self.parse_typing()?;
                        let n = self.next();
                        match n.typ {
                            Tok::RightParen => break,
                            Tok::Comma => {}
                            _ => return Err(format!("ln{}: unexpected token parsing fn type: {}", n.line, n.val)),
                        }
                    }
                }
                if matches!(self.peek().typ, Tok::Identifier | Tok::Fn) {
                    self.parse_typing()?;
                }
                Ok(false)
            }
            _ => Err(format!("ln{}: error parsing type info, unexpected value, got {}", n.line, n.val)),
        }
    }

    // ------------------------------------------------------------ expressions

    fn parse_expr(&mut self, pre: Prec) -> R<Expr> {
        let t = self.peek();
        let mut left = match t.typ {
            Tok::Identifier => {
                self.next();
                Expr::Ident(t.val)
            }
            Tok::Field => {
                let mut fields = Vec::new();
                while self.peek().typ == Tok::Field {
                    fields.push(self.next().val.trim_matches('.').to_string());
                }
                Expr::Field(fields)
            }
            Tok::Number => {
                self.next();
                match t.val.parse::<i64>() {
                    Ok(i) => Expr::Num(Num::Int(i)),
                    Err(_) => Expr::Num(Num::Float(Self::float(&t)?)),
                }
            }
            Tok::Bool => {
                self.next();
                Expr::Num(Num::Int((t.val == "true") as i64))
            }
            Tok::String => {
                self.next();
                Expr::Str(t.val.trim_matches('"').to_string())
            }
            Tok::Fn => {
                self.next();
                Expr::Func(Rc::new(self.parse_fn()?))
            }
            Tok::LogicNot | Tok::Minus => {
                self.next();
                Expr::Unary(t.typ, Box::new(self.parse_expr(Prec::Prefix)?))
            }
            Tok::LeftParen => {
                self.next();
                let e = self.parse_expr(Prec::Lowest)?;
                let n = self.next();
                if n.typ != Tok::RightParen {
                    return Err(format!("ln{}: expecting ), got {}", n.line, n.val));
                }
                e
            }
            Tok::LeftSquareParen => self.parse_map()?,
            Tok::Error => return Err(format!("ln{}: {}", t.line, t.val)),
            _ => return Err(format!("ln{}: no prefix parse function for {}", t.line, t.val)),
        };
        loop {
            let n = self.peek();
            if n.typ == Tok::TerminateLine || pre >= precedence(n.typ) {
                break;
            }
            match n.typ {
                Tok::LeftParen => {
                    self.next();
                    let args = self.parse_call_args()?;
                    left = Expr::Call(Box::new(left), args, n.line);
                }
                Tok::LogicAnd
                | Tok::LogicOr
                | Tok::Plus
                | Tok::Minus
                | Tok::ForwardSlash
                | Tok::Asterisk
                | Tok::OpEqual
                | Tok::OpNotEqual
                | Tok::OpLessThan
                | Tok::OpLessThanOrEqual
                | Tok::OpGreaterThan
                | Tok::OpGreaterThanOrEqual => {
                    self.next();
                    let right = self.parse_expr(precedence(n.typ))?;
                    left = Expr::Binary(n.typ, Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        fold(left)
    }

    fn parse_call_args(&mut self) -> R<Vec<Expr>> {
        let mut args = Vec::new();
        if self.peek().typ == Tok::RightParen {
            self.next();
            return Ok(args);
        }
        args.push(self.parse_expr(Prec::Lowest)?);
        while self.peek().typ == Tok::Comma {
            self.next();
            args.push(self.parse_expr(Prec::Lowest)?);
        }
        let n = self.next();
        if n.typ != Tok::RightParen {
            return Err(format!("ln{}: expecting ) at end of function call, got {}", n.line, n.val));
        }
        Ok(args)
    }

    fn parse_map(&mut self) -> R<Expr> {
        self.next();
        let mut fields = Vec::new();
        if self.peek().typ == Tok::RightSquareParen {
            self.next();
            return Ok(Expr::Map(fields));
        }
        loop {
            let i = self.consume(Tok::Identifier)?;
            self.consume(Tok::Assign)?;
            let e = self.parse_expr(Prec::Lowest)?;
            fields.retain(|(k, _): &(String, Expr)| *k != i.val);
            fields.push((i.val, e));
            let n = self.next();
            match n.typ {
                Tok::RightSquareParen => return Ok(Expr::Map(fields)),
                Tok::Comma => {}
                _ => return Err(format!("ln{}: <map> bad token {}", n.line, n.val)),
            }
        }
    }
}

/// Copies an action param, which is a number after constant folding.
fn clone_const(e: &Expr) -> Expr {
    match e {
        Expr::Num(n) => Expr::Num(*n),
        Expr::Str(s) => Expr::Str(s.clone()),
        Expr::Ident(s) => Expr::Ident(s.clone()),
        Expr::Field(f) => Expr::Field(f.clone()),
        Expr::Unary(op, r) => Expr::Unary(*op, Box::new(clone_const(r))),
        Expr::Binary(op, l, r) => Expr::Binary(*op, Box::new(clone_const(l)), Box::new(clone_const(r))),
        Expr::Map(f) => Expr::Map(f.iter().map(|(k, v)| (k.clone(), clone_const(v))).collect()),
        Expr::Call(fun, args, line) => Expr::Call(Box::new(clone_const(fun)), args.iter().map(clone_const).collect(), *line),
        Expr::Func(f) => Expr::Func(f.clone()),
    }
}

// ---------------------------------------------------------------- constants

/// A number or string operand: gcsim's constant.Value.
#[derive(Clone, Debug)]
pub enum Const {
    Num(Num),
    Str(String),
}

fn arith(op: Tok, l: Num, r: Num) -> Num {
    match (l, r) {
        (Num::Int(a), Num::Int(b)) => Num::Int(match op {
            Tok::Plus => a.wrapping_add(b),
            Tok::Minus => a.wrapping_sub(b),
            Tok::Asterisk => a.wrapping_mul(b),
            _ => a / b,
        }),
        _ => {
            let (a, b) = (l.f(), r.f());
            Num::Float(match op {
                Tok::Plus => a + b,
                Tok::Minus => a - b,
                Tok::Asterisk => a * b,
                _ => a / b,
            })
        }
    }
}

pub fn unary_op(op: Tok, r: &Const) -> R<Const> {
    match (op, r) {
        (Tok::Plus, Const::Num(n)) => Ok(Const::Num(*n)),
        (Tok::LogicNot, _) => Ok(Const::Num(Num::from_bool(!const_truthy(r)))),
        (Tok::Minus, Const::Num(n)) => Ok(Const::Num(arith(Tok::Minus, Num::Int(0), *n))),
        _ => Err(format!("invalid unary operator {op:?}")),
    }
}

pub fn const_truthy(c: &Const) -> bool {
    match c {
        Const::Num(n) => n.truthy(),
        Const::Str(_) => true,
    }
}

pub fn binary_op(op: Tok, l: &Const, r: &Const) -> R<Const> {
    let (Const::Num(a), Const::Num(b)) = (l, r) else {
        return Err(format!("invalid binary operator {op:?} on {l:?} and {r:?}"));
    };
    let (a, b) = (*a, *b);
    Ok(Const::Num(match op {
        Tok::Plus | Tok::Minus | Tok::Asterisk => arith(op, a, b),
        Tok::ForwardSlash => {
            if b == Num::Int(0) {
                return Err("division by zero".into());
            }
            arith(op, a, b)
        }
        Tok::OpGreaterThan => Num::from_bool(a.f() > b.f()),
        Tok::OpGreaterThanOrEqual => Num::from_bool(a.f() >= b.f()),
        Tok::OpEqual => Num::from_bool(a.f() == b.f()),
        Tok::OpNotEqual => Num::from_bool(a.f() != b.f()),
        Tok::OpLessThan => Num::from_bool(a.f() < b.f()),
        Tok::OpLessThanOrEqual => Num::from_bool(a.f() <= b.f()),
        Tok::LogicAnd => Num::from_bool(a.truthy() && b.truthy()),
        Tok::LogicOr => Num::from_bool(a.truthy() || b.truthy()),
        _ => return Err(format!("invalid binary operator {op:?}")),
    }))
}

fn as_const(e: &Expr) -> Option<Const> {
    match e {
        Expr::Num(n) => Some(Const::Num(*n)),
        Expr::Str(s) => Some(Const::Str(s.clone())),
        _ => None,
    }
}

fn from_const(c: Const) -> Expr {
    match c {
        Const::Num(n) => Expr::Num(n),
        Const::Str(s) => Expr::Str(s),
    }
}

/// gcsim's foldConstants.
fn fold(e: Expr) -> R<Expr> {
    match e {
        Expr::Unary(op, r) => {
            let r = fold(*r)?;
            match as_const(&r) {
                Some(c) => Ok(from_const(unary_op(op, &c)?)),
                None => Ok(Expr::Unary(op, Box::new(r))),
            }
        }
        Expr::Binary(op, l, r) => {
            let (l, r) = (fold(*l)?, fold(*r)?);
            match (as_const(&l), as_const(&r)) {
                (Some(a), Some(b)) => Ok(from_const(binary_op(op, &a, &b)?)),
                _ => Ok(Expr::Binary(op, Box::new(l), Box::new(r))),
            }
        }
        e => Ok(e),
    }
}

pub fn parse(input: &str) -> R<ParsedConfig> {
    Parser::new(input).parse()
}
