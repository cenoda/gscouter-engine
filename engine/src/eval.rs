//! gcsim's evaluator (pkg/gcs/eval), ported to keep its semantics: block
//! scopes, closures, return and control values, both sides of && and ||
//! evaluated, and loops that ignore a return. It runs as gcsim's RunSync
//! does: each action is handed to the fight, which runs until it needs the
//! next one, and the program then carries on.

use std::cell::RefCell;
use std::rc::Rc;

use crate::gcsl::{binary_op, char_name, unary_op, Block, Const, Ctrl, Expr, FuncLit, Num, Stmt};
use crate::sim::{Params, Sim};
use crate::types::Action;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Builtin {
    F,
    Rand,
    RandNorm,
    Print,
    Wait,
    Delay,
    Type,
    ExecuteAction,
    SetSwapIcd,
    SetParticleDelay,
    Movement,
    IsEven,
    Sin,
    Cos,
    Asin,
    Acos,
    Unsupported(&'static str),
}

const BUILTINS: &[(&str, Builtin)] = &[
    ("f", Builtin::F),
    ("rand", Builtin::Rand),
    ("randnorm", Builtin::RandNorm),
    ("print", Builtin::Print),
    ("wait", Builtin::Wait),
    ("sleep", Builtin::Wait),
    ("delay", Builtin::Delay),
    ("type", Builtin::Type),
    ("execute_action", Builtin::ExecuteAction),
    ("set_target_pos", Builtin::Unsupported("set_target_pos")),
    ("set_player_pos", Builtin::Unsupported("set_player_pos")),
    ("set_default_target", Builtin::Unsupported("set_default_target")),
    ("set_swap_icd", Builtin::SetSwapIcd),
    ("set_particle_delay", Builtin::SetParticleDelay),
    ("kill_target", Builtin::Unsupported("kill_target")),
    ("is_target_dead", Builtin::Unsupported("is_target_dead")),
    ("pick_up_crystallize", Builtin::Unsupported("pick_up_crystallize")),
    ("set_starting_verdant_dew", Builtin::Unsupported("set_starting_verdant_dew")),
    ("movement", Builtin::Movement),
    ("sin", Builtin::Sin),
    ("cos", Builtin::Cos),
    ("asin", Builtin::Asin),
    ("acos", Builtin::Acos),
    ("is_even", Builtin::IsEven),
    ("set_on_tick", Builtin::Unsupported("set_on_tick")),
];

/// Whether gcsl has the built-in and the engine does not implement it.
pub fn unsupported_builtin(name: &str) -> bool {
    BUILTINS.iter().any(|(n, b)| *n == name && matches!(b, Builtin::Unsupported(_)))
}

#[derive(Clone)]
enum Value<'a> {
    Null,
    Num(Num),
    Str(Rc<str>),
    Func(Rc<FuncVal<'a>>),
    Builtin(Builtin),
    Map(Rc<Vec<(String, Value<'a>)>>),
    Ret(Box<Value<'a>>),
    Ctrl(Ctrl),
}

impl Value<'_> {
    fn typ(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Num(_) => "number",
            Value::Str(_) => "string",
            Value::Func(_) => "function",
            Value::Builtin(_) => "builtin_function",
            Value::Map(_) => "map",
            Value::Ret(_) => "return",
            Value::Ctrl(_) => "control",
        }
    }
    fn truthy(&self) -> bool {
        match self {
            Value::Num(n) => n.truthy(),
            Value::Str(_) => true,
            _ => false,
        }
    }
    fn constant(&self) -> Option<Const> {
        match self {
            Value::Num(n) => Some(Const::Num(*n)),
            Value::Str(s) => Some(Const::Str(s.to_string())),
            _ => None,
        }
    }
}

fn from_const<'a>(c: Const) -> Value<'a> {
    match c {
        Const::Num(n) => Value::Num(n),
        Const::Str(s) => Value::Str(s.into()),
    }
}

struct FuncVal<'a> {
    lit: &'a FuncLit,
    env: Env<'a>,
}

type Var<'a> = Rc<RefCell<Value<'a>>>;

struct Scope<'a> {
    parent: Option<Env<'a>>,
    vars: RefCell<Vec<(&'a str, Var<'a>)>>,
}

type Env<'a> = Rc<Scope<'a>>;

fn new_env<'a>(parent: Option<&Env<'a>>) -> Env<'a> {
    Rc::new(Scope { parent: parent.cloned(), vars: RefCell::new(Vec::new()) })
}

impl<'a> Scope<'a> {
    fn has_own(&self, name: &str) -> bool {
        self.vars.borrow().iter().any(|v| v.0 == name)
    }
    fn set(&self, name: &'a str, v: Value<'a>) {
        let mut vars = self.vars.borrow_mut();
        match vars.iter().position(|x| x.0 == name) {
            Some(i) => vars[i].1 = Rc::new(RefCell::new(v)),
            None => vars.push((name, Rc::new(RefCell::new(v)))),
        }
    }
    fn get(&self, name: &str) -> Option<Var<'a>> {
        if let Some(v) = self.vars.borrow().iter().find(|v| v.0 == name) {
            return Some(v.1.clone());
        }
        self.parent.as_ref().and_then(|p| p.get(name))
    }
}

/// Why evaluation stopped: an error, or the fight ended (gcsim's
/// ErrTerminated, which is not an error).
pub enum Stop {
    Err(String),
    Terminated,
}

type R<'a> = Result<Value<'a>, Stop>;

fn err<'a, T>(msg: String) -> Result<T, Stop> {
    Err(Stop::Err(msg))
}

pub struct Eval<'s> {
    sim: &'s mut Sim,
}

/// Runs the program against the fight, then the fight to its end.
pub fn run(sim: &mut Sim, program: &Block) -> Result<(), String> {
    let global = new_env(None);
    for (name, b) in BUILTINS {
        global.set(name, Value::Builtin(*b));
    }
    let res = {
        let mut e = Eval { sim: &mut *sim };
        e.block(program, &global)
    };
    match res {
        Ok(_) | Err(Stop::Terminated) => {
            sim.finish();
            Ok(())
        }
        Err(Stop::Err(m)) => Err(m),
    }
}

impl<'s> Eval<'s> {
    fn block<'a>(&mut self, b: &'a Block, env: &Env<'a>) -> R<'a> {
        let scope = new_env(Some(env));
        let mut last = Value::Null;
        for s in &b.0 {
            let v = self.stmt(s, &scope)?;
            if matches!(v, Value::Ret(_) | Value::Ctrl(_)) {
                return Ok(v);
            }
            last = v;
        }
        Ok(last)
    }

    fn stmt<'a>(&mut self, s: &'a Stmt, env: &Env<'a>) -> R<'a> {
        match s {
            Stmt::Block(b) => self.block(b, env),
            Stmt::Expr(e) => self.expr(e, env),
            Stmt::Let(name, val) => {
                let v = self.expr(val, env)?;
                if env.has_own(name) {
                    return err(format!("variable {name} already exists; cannot redeclare"));
                }
                env.set(name, v);
                Ok(Value::Null)
            }
            Stmt::Fn(name, lit) => {
                if env.has_own(name) {
                    return err(format!("function {name} already exists; cannot redeclare"));
                }
                env.set(name, Value::Func(Rc::new(FuncVal { lit, env: new_env(Some(env)) })));
                Ok(Value::Null)
            }
            Stmt::Assign(name, val) => {
                let v = self.expr(val, env)?;
                let Some(var) = env.get(name) else {
                    return err(format!("variable {name} does not exist"));
                };
                *var.borrow_mut() = v.clone();
                Ok(v)
            }
            Stmt::Return(e) => Ok(Value::Ret(Box::new(self.expr(e, env)?))),
            Stmt::Ctrl(c) => Ok(Value::Ctrl(*c)),
            Stmt::If(cond, then, other) => {
                if self.expr(cond, env)?.truthy() {
                    self.block(then, env)
                } else if let Some(o) = other {
                    self.stmt(o, env)
                } else {
                    Ok(Value::Null)
                }
            }
            Stmt::While(cond, body) => {
                loop {
                    if !self.expr(cond, env)?.truthy() {
                        break;
                    }
                    if let Value::Ctrl(Ctrl::Break) = self.block(body, env)? {
                        break;
                    }
                }
                Ok(Value::Null)
            }
            Stmt::For(init, cond, post, body) => {
                let scope = new_env(Some(env));
                if let Some(i) = init {
                    // gcsim ignores an error here unless the fight ended
                    if let Err(Stop::Terminated) = self.stmt(i, &scope) {
                        return Err(Stop::Terminated);
                    }
                }
                loop {
                    if let Some(c) = cond {
                        if !self.expr(c, &scope)?.truthy() {
                            break;
                        }
                    }
                    if let Value::Ctrl(Ctrl::Break) = self.block(body, &scope)? {
                        break;
                    }
                    if let Some(p) = post {
                        if let Err(Stop::Terminated) = self.stmt(p, &scope) {
                            return Err(Stop::Terminated);
                        }
                    }
                }
                Ok(Value::Null)
            }
            Stmt::Switch(cond, cases, default) => self.switch(cond.as_ref(), cases, default.as_ref(), env),
        }
    }

    fn switch<'a>(&mut self, cond: Option<&'a Expr>, cases: &'a [(Expr, Block)], default: Option<&'a Block>, env: &Env<'a>) -> R<'a> {
        let v = match cond {
            None => None,
            Some(c) => match self.expr(c, env)? {
                Value::Null => None,
                Value::Num(n) => Some(n),
                other => return err(format!("switch condition does not evaluate to a number, got {}", other.typ())),
            },
        };
        let mut ft = false;
        let mut found = false;
        for (c, body) in cases {
            let Value::Num(cn) = self.expr(c, env)? else {
                return err("switch case condition does not evaluate to a number".into());
            };
            let hit = match v {
                None => cn.truthy(),
                Some(v) => cn.f() == v.f(),
            };
            if hit || ft {
                found = true;
                match self.block(body, env)? {
                    Value::Ctrl(Ctrl::Fallthrough) => ft = true,
                    Value::Ctrl(Ctrl::Break) => return Ok(Value::Null),
                    Value::Ctrl(_) => {}
                    res => return Ok(res),
                }
            }
        }
        if !found || ft {
            return match default {
                Some(d) => self.block(d, env),
                None => Ok(Value::Null),
            };
        }
        Ok(Value::Null)
    }

    fn expr<'a>(&mut self, e: &'a Expr, env: &Env<'a>) -> R<'a> {
        match e {
            Expr::Num(n) => Ok(Value::Num(*n)),
            Expr::Str(s) => Ok(Value::Str(s.as_str().into())),
            Expr::Func(lit) => Ok(Value::Func(Rc::new(FuncVal { lit, env: new_env(Some(env)) }))),
            Expr::Ident(name) => match env.get(name) {
                Some(v) => Ok(v.borrow().clone()),
                None => err(format!("variable {name} does not exist")),
            },
            Expr::Field(fields) => self.sim.field(fields).map(Value::Num).map_err(Stop::Err),
            Expr::Map(fields) => {
                let mut out = Vec::with_capacity(fields.len());
                for (k, v) in fields {
                    out.push((k.clone(), self.expr(v, env)?));
                }
                Ok(Value::Map(Rc::new(out)))
            }
            Expr::Unary(op, r) => {
                let r = self.expr(r, env)?;
                let Some(c) = r.constant() else {
                    return err(format!("unary expression does not evaluate to a comparable variable, got {}", r.typ()));
                };
                unary_op(*op, &c).map(from_const).map_err(Stop::Err)
            }
            Expr::Binary(op, l, r) => {
                let l = self.expr(l, env)?;
                let r = self.expr(r, env)?;
                let (Some(a), Some(b)) = (l.constant(), r.constant()) else {
                    return err("binary expression does not evaluate to a comparable variable".into());
                };
                binary_op(*op, &a, &b).map(from_const).map_err(Stop::Err)
            }
            Expr::Call(fun, args, line) => self.call(fun, args, *line, env),
        }
    }

    fn call<'a>(&mut self, fun: &'a Expr, args: &'a [Expr], line: u32, env: &Env<'a>) -> R<'a> {
        let f = self.expr(fun, env)?;
        match f {
            Value::Builtin(b) => self.builtin(b, args, line, env),
            Value::Func(fv) => {
                if args.len() != fv.lit.args.len() {
                    return err(format!("ln{line}: unmatched number of params for fn"));
                }
                let local = new_env(Some(&fv.env));
                for (name, a) in fv.lit.args.iter().zip(args) {
                    let v = self.expr(a, env)?;
                    local.set(name, v);
                }
                match self.block(&fv.lit.body, &local)? {
                    Value::Ret(v) => Ok(*v),
                    Value::Null if fv.lit.returns_number => Ok(Value::Num(Num::Int(0))),
                    Value::Null => Ok(Value::Null),
                    v @ Value::Num(_) if fv.lit.returns_number => Ok(v),
                    v => err(format!("ln{line}: fn returned an invalid type; got {}", v.typ())),
                }
            }
            _ => err(format!("ln{line}: invalid function call")),
        }
    }

    fn args<'a>(&mut self, args: &'a [Expr], line: u32, types: &[&str], env: &Env<'a>) -> Result<Vec<Value<'a>>, Stop> {
        if args.len() != types.len() {
            return err(format!("ln{line}: invalid number of params (expected {}, got {})", types.len(), args.len()));
        }
        let mut out = Vec::with_capacity(args.len());
        for (i, (a, t)) in args.iter().zip(types).enumerate() {
            let v = self.expr(a, env)?;
            if v.typ() != *t {
                return err(format!("ln{line}: argument #{} should evaluate to {t}, got {}", i + 1, v.typ()));
            }
            out.push(v);
        }
        Ok(out)
    }

    fn num<'a>(&mut self, args: &'a [Expr], line: u32, env: &Env<'a>) -> Result<Num, Stop> {
        match self.args(args, line, &["number"], env)?.pop() {
            Some(Value::Num(n)) => Ok(n),
            _ => unreachable!(),
        }
    }

    fn builtin<'a>(&mut self, b: Builtin, args: &'a [Expr], line: u32, env: &Env<'a>) -> R<'a> {
        let float = |f: f64| Ok(Value::Num(Num::Float(f)));
        match b {
            Builtin::F => Ok(Value::Num(Num::Int(self.sim.f as i64))),
            Builtin::Rand => float(self.sim.rng.float64()),
            Builtin::RandNorm => float(self.sim.rng.norm_float64()),
            Builtin::Print => {
                for a in args {
                    self.expr(a, env)?;
                }
                Ok(Value::Null)
            }
            Builtin::Wait => {
                let f = self.num(args, line, env)?.i();
                if f >= 0 && !self.sim.wait(f as i32) {
                    return Err(Stop::Terminated);
                }
                Ok(Value::Null)
            }
            Builtin::Delay => {
                let f = self.num(args, line, env)?.i();
                if f >= 0 {
                    if self.sim.done {
                        return Err(Stop::Terminated);
                    }
                    self.sim.add_delay(f as i32);
                }
                Ok(Value::Null)
            }
            Builtin::Type => {
                if args.len() != 1 {
                    return err(format!("ln{line}: invalid number of params for type"));
                }
                let v = self.expr(&args[0], env)?;
                Ok(Value::Str(v.typ().into()))
            }
            Builtin::ExecuteAction => {
                let vs = self.args(args, line, &["number", "number", "map"], env)?;
                let (Value::Num(c), Value::Num(a), Value::Map(m)) = (&vs[0], &vs[1], &vs[2]) else { unreachable!() };
                let mut params = Params::default();
                for (k, v) in m.iter() {
                    let Value::Num(n) = v else {
                        return err(format!("ln{line}: map params should evaluate to a number"));
                    };
                    params.0.push((k.clone(), n.f() as i64));
                }
                let key = c.i() as u16;
                let Some(ci) = self.sim.index_of(key) else {
                    return err(format!("ln{line}: can't execute action: {} is not on this team", char_name(key)));
                };
                let Some(action) = action_from(a.i()) else {
                    return err(format!("ln{line}: invalid action {}", a.i()));
                };
                if let Some(what) = crate::sim::unsupported_action(self.sim.chars[ci].kind, action, &params) {
                    return err(format!("unsupported: action {} {what}", char_name(key)));
                }
                if ci != self.sim.player.active as usize && !self.sim.queue_action(ci, Action::Swap, &Params::default()) {
                    return Err(Stop::Terminated);
                }
                if !self.sim.queue_action(ci, action, &params) {
                    return Err(Stop::Terminated);
                }
                Ok(Value::Null)
            }
            Builtin::SetSwapIcd => {
                let f = self.num(args, line, env)?.i();
                if f < 0 {
                    return err(format!("ln{line}: invald value for set_swap_icd, expected non-negative integer, got {f}"));
                }
                self.sim.swap_icd = f as i32;
                Ok(Value::Null)
            }
            Builtin::SetParticleDelay => {
                let vs = self.args(args, line, &["string", "number"], env)?;
                let (Value::Str(name), Value::Num(d)) = (&vs[0], &vs[1]) else { unreachable!() };
                let Some(key) = crate::gcsl::char_key(name) else {
                    return err(format!("ln{line}: set_particle_delay first argument {name} is not a valid character"));
                };
                let Some(ci) = self.sim.index_of(key) else {
                    return err(format!("ln{line}: set_particle_delay: {name} is not on this team"));
                };
                self.sim.particle_delay[ci] = d.i() as i32;
                Ok(Value::Null)
            }
            Builtin::Movement => {
                let f = self.num(args, line, env)?.f();
                if f < 0.0 {
                    return err(format!("invalid value for movement, expected non-negative number, got {f}"));
                }
                // OnMovement: none of the engine's characters listen to it
                Ok(Value::Null)
            }
            Builtin::IsEven => {
                let v = self.num(args, line, env)?.i();
                Ok(Value::Num(Num::from_bool(v % 2 == 0)))
            }
            Builtin::Sin => float(self.num(args, line, env)?.f().sin()),
            Builtin::Cos => float(self.num(args, line, env)?.f().cos()),
            Builtin::Asin => float(self.num(args, line, env)?.f().asin()),
            Builtin::Acos => float(self.num(args, line, env)?.f().acos()),
            Builtin::Unsupported(name) => err(format!("unsupported: builtin {name}")),
        }
    }
}

pub fn action_from(i: i64) -> Option<Action> {
    Some(match i {
        1 => Action::Skill,
        2 => Action::Burst,
        3 => Action::Attack,
        4 => Action::Charge,
        5 => Action::HighPlunge,
        6 => Action::LowPlunge,
        7 => Action::Aim,
        8 => Action::Dash,
        9 => Action::Jump,
        10 => Action::Swap,
        11 => Action::Walk,
        _ => return None,
    })
}
