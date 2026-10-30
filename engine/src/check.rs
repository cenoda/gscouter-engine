//! Lists what a config needs that the engine does not implement, without
//! running it: characters, weapons, sets, params, targets, and the actions
//! and built-ins its program calls.

use crate::gcsl::{char_name, Block, Expr, Num, ParsedConfig, Stmt};
use crate::profile;
use crate::sim::{unsupported_action, CharKind, Params};

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
    let mut w = Walker { kinds: &kinds, found: Vec::new() };
    w.block(&p.program);
    for s in w.found {
        add(s);
    }
    out
}

struct Walker<'a> {
    kinds: &'a [(u16, Option<CharKind>)],
    found: Vec<String>,
}

impl Walker<'_> {
    fn block(&mut self, b: &Block) {
        for s in &b.0 {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
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
            Stmt::Fn(_, lit) => self.block(&lit.body),
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

    fn expr(&mut self, e: &Expr) {
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
    }
}
