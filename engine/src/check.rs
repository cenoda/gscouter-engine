//! Lists what a config needs that the engine does not implement, without
//! running it: characters, weapons, sets, params, targets, and the actions
//! and built-ins its program calls.

use crate::gcsl::{char_name, Block, CharProfile, Expr, Num, ParsedConfig, Stmt};
use crate::profile;
use crate::sim::{unsupported_action, CharKind, Params};
use crate::types::Action;

/// Every unsupported feature of the config, each named once, in the order
/// found.
pub fn unsupported(p: &ParsedConfig) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |s: String| {
        if !out.contains(&s) {
            out.push(s);
        }
    };
    let kinds: Vec<(u16, Option<CharKind>)> = match profile::config(p) {
        Ok(cfg) => cfg.chars.iter().map(|c| (c.key, Some(c.kind))).collect(),
        Err(errs) => {
            for e in errs {
                add(e);
            }
            p.chars.iter().map(|c| (c.key, profile::char_kind(char_name(c.key)))).collect()
        }
    };
    let mut w = Walker { kinds: &kinds, chars: &p.chars, found: Vec::new(), fns: Vec::new() };
    w.block(&p.program);
    for s in w.found {
        add(s);
    }
    out
}

struct Walker<'a> {
    kinds: &'a [(u16, Option<CharKind>)],
    chars: &'a [CharProfile],
    found: Vec<String>,
    /// The functions declared so far, by name, for set_on_tick's.
    fns: Vec<(&'a str, &'a Block)>,
}

impl<'a> Walker<'a> {
    fn block(&mut self, b: &'a Block) {
        for s in &b.0 {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &'a Stmt) {
        match s {
            Stmt::Block(b) => self.block(b),
            Stmt::Let(_, e) | Stmt::Assign(_, e) | Stmt::Return(e) | Stmt::Expr(e) => self.expr(e),
            Stmt::Ctrl(_) => {}
            Stmt::If(c, then, other) => {
                self.expr(c);
                self.block(then);
                if let Some(o) = other {
                    self.stmt(o);
                }
            }
            Stmt::Switch(c, cases, default) => {
                if let Some(c) = c {
                    self.expr(c);
                }
                for (e, b) in cases {
                    self.expr(e);
                    self.block(b);
                }
                if let Some(d) = default {
                    self.block(d);
                }
            }
            Stmt::Fn(name, lit) => {
                self.fns.push((name, &lit.body));
                self.block(&lit.body)
            }
            Stmt::While(c, b) => {
                self.expr(c);
                self.block(b);
            }
            Stmt::For(init, cond, post, b) => {
                if let Some(i) = init {
                    self.stmt(i);
                }
                if let Some(c) = cond {
                    self.expr(c);
                }
                if let Some(p) = post {
                    self.stmt(p);
                }
                self.block(b);
            }
        }
    }

    fn expr(&mut self, e: &'a Expr) {
        match e {
            Expr::Num(_) | Expr::Str(_) | Expr::Ident(_) | Expr::Field(_) => {}
            Expr::Func(lit) => self.block(&lit.body),
            Expr::Unary(_, r) => self.expr(r),
            Expr::Binary(_, l, r) => {
                self.expr(l);
                self.expr(r);
            }
            Expr::Map(fields) => {
                for (_, v) in fields {
                    self.expr(v);
                }
            }
            Expr::Call(fun, args, _) => {
                if let Expr::Ident(name) = fun.as_ref() {
                    if crate::eval::unsupported_builtin(name) {
                        self.found.push(format!("unsupported: builtin {name}"));
                    }
                    if name == "execute_action" {
                        self.action(args);
                    }
                    // the engine runs a callback that only reads (eval.rs)
                    if name == "set_on_tick" {
                        let body = match args.first() {
                            Some(Expr::Func(lit)) => Some(&lit.body),
                            Some(Expr::Ident(f)) => self.fns.iter().rev().find(|x| x.0 == f).map(|x| x.1),
                            _ => None,
                        };
                        match body.map(crate::eval::on_tick_effect) {
                            Some(None) => {}
                            Some(Some(why)) => self.found.push(format!("unsupported: builtin set_on_tick with {why}")),
                            None => self.found.push("unsupported: builtin set_on_tick with a function it cannot find".into()),
                        }
                    }
                    // moving the player away from the origin needs every
                    // member's module checked for it
                    let origin = args.len() == 2 && args.iter().all(|a| matches!(a, Expr::Num(n) if n.f() == 0.0));
                    if name == "set_player_pos" && !origin {
                        for (key, kind) in self.kinds {
                            if kind.is_some_and(|k| !crate::chars::MOVES_WITH_PLAYER.contains(&k)) {
                                self.found.push(format!("unsupported: set_player_pos with {}", char_name(*key)));
                            }
                        }
                    }
                }
                self.expr(fun);
                for a in args {
                    self.expr(a);
                }
            }
        }
    }

    /// An action list's execute_action call, whose character and action are
    /// constants.
    fn action(&mut self, args: &[Expr]) {
        let (Some(Expr::Num(Num::Int(c))), Some(Expr::Num(Num::Int(a)))) = (args.first(), args.get(1)) else {
            return;
        };
        let Some(action) = crate::eval::action_from(*a) else { return };
        let mut params = Params::default();
        if let Some(Expr::Map(fields)) = args.get(2) {
            for (k, v) in fields {
                if let Expr::Num(n) = v {
                    params.0.push((k.clone(), n.f() as i64));
                }
            }
        }
        let key = *c as u16;
        // an unsupported character is reported once, by itself
        let Some((_, Some(kind))) = self.kinds.iter().find(|k| k.0 == key) else { return };
        if let Some(what) = unsupported_action(*kind, action, &params) {
            self.found.push(format!("unsupported: action {} {what}", char_name(key)));
        }
        if *kind == CharKind::Dahlia && action == Action::Burst && self.chars.iter().any(|c| c.key == key && c.max_level < 70) {
            self.found.push("unsupported: dahlia's burst below ascension 4".into());
        }
        // Nefer's held charged attack moves the player (Slither): every
        // member needs its module checked for it. A hold that is not a
        // constant counts as one that may be held.
        if *kind == CharKind::Nefer && action == crate::types::Action::Charge {
            let hold = match args.get(2) {
                Some(Expr::Map(fields)) => match fields.iter().find(|(k, _)| k == "hold") {
                    Some((_, Expr::Num(n))) => Some(n.f() as i64),
                    Some(_) => None,
                    None => Some(0),
                },
                _ => Some(0),
            };
            if crate::chars::nefer::may_slither(hold) {
                for (k, kd) in self.kinds {
                    if kd.is_some_and(|x| !crate::chars::MOVES_WITH_PLAYER.contains(&x)) {
                        self.found.push(format!("unsupported: nefer charge[hold] (Slither moves the player) with {}", char_name(*k)));
                    }
                }
            }
        }
    }
}
