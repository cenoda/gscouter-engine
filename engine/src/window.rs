//! Gscouter's window for the site's score: with no warmup on a fight with
//! a duration (0–120 s from full energy), the program's loop is marked into
//! cycles and the window ends at the last completed one
//! (internal/score/score.go: markCycles, stillGuard, simulateWith), so that
//! an unfinished cycle's setup at the end of the clock does not count. The
//! marks are delays of `sim::MARKER`, which the fight records
//! (`Sim::mark_cycle`) and does not play; `Sim::window_end` is where the
//! window ends.

use std::collections::BTreeSet;

use crate::gcsl::{Block, Expr, Num, Stmt};
use crate::sim::MARKER;

/// Whether a run's window ends at the last completed cycle: Gscouter
/// marks cycles with no warmup on a fight with a duration.
pub fn cuts_at_cycles(warmup: f64, duration: f64) -> bool {
    warmup == 0.0 && duration > 0.0
}

/// Gscouter's `markCycles`: marks the end of each cycle of the program's
/// last top-level `while` loop, when two characters or more act in it. A
/// `for` loop in it where two characters or more act is a cycle, marked at
/// the end of its body, and so are the statements between two such loops
/// when two characters or more act in them; with no such `for` loop, the
/// loop's body is one cycle.
pub fn mark_cycles(program: &mut Block) {
    let Some(body) = program.0.iter_mut().rev().find_map(|s| match s {
        Stmt::While(_, b) => Some(b),
        _ => None,
    }) else {
        return;
    };
    if !multi_char(&body.0) {
        return;
    }
    let old = std::mem::take(&mut body.0);
    let mut list = Vec::with_capacity(old.len() + 2);
    let mut since = BTreeSet::new();
    let mut marked_for = false;
    for mut n in old {
        let cycle_for = match &mut n {
            Stmt::For(_, _, _, b) if multi_char(&b.0) => {
                b.0.push(cycle_marker());
                true
            }
            _ => false,
        };
        if cycle_for {
            if since.len() >= 2 {
                list.push(cycle_marker());
            }
            since.clear();
            marked_for = true;
        } else {
            collect_chars(std::slice::from_ref(&n), &mut since);
        }
        list.push(n);
    }
    if !marked_for || since.len() >= 2 {
        list.push(cycle_marker());
    }
    body.0 = list;
}

fn cycle_marker() -> Stmt {
    Stmt::Expr(Expr::Call(Box::new(Expr::Ident("delay".into())), vec![Expr::Num(Num::Int(MARKER))], 0))
}

/// Whether two characters or more act in the statements (Gscouter's
/// multiChar).
fn multi_char(stmts: &[Stmt]) -> bool {
    let mut chars = BTreeSet::new();
    collect_chars(stmts, &mut chars);
    chars.len() >= 2
}

/// The characters whose actions the statements execute, as Gscouter's
/// collectChars finds them: in blocks, branches and loop bodies, not in
/// functions or expressions.
fn collect_chars(stmts: &[Stmt], chars: &mut BTreeSet<i64>) {
    for s in stmts {
        match s {
            Stmt::Block(b) | Stmt::While(_, b) | Stmt::For(_, _, _, b) => collect_chars(&b.0, chars),
            Stmt::If(_, b, els) => {
                collect_chars(&b.0, chars);
                if let Some(e) = els {
                    collect_chars(std::slice::from_ref(&**e), chars);
                }
            }
            Stmt::Switch(_, cases, default) => {
                for (_, b) in cases {
                    collect_chars(&b.0, chars);
                }
                if let Some(b) = default {
                    collect_chars(&b.0, chars);
                }
            }
            Stmt::Expr(Expr::Call(f, args, _)) => {
                if let (Expr::Ident(name), Some(Expr::Num(k))) = (&**f, args.first())
                    && name == "execute_action"
                {
                    chars.insert(match k {
                        Num::Int(i) => *i,
                        Num::Float(x) => *x as i64,
                    });
                }
            }
            _ => {}
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcsl;

    /// Where the markers are: per statement of the loop's body, the number
    /// of markers it holds (a `for` loop's) or 'M' for a marker itself.
    fn marks(src: &str) -> String {
        let head = "target lvl=100 resist=0.1;\nhutao char lvl=90/90 cons=0 talent=9,9,9;\nhutao add weapon=\"staffofhoma\" refine=1 lvl=90/90;\nxingqiu char lvl=90/90 cons=0 talent=9,9,9;\nxingqiu add weapon=\"sacrificialsword\" refine=1 lvl=90/90;\nactive hutao;\n";
        let mut p = gcsl::parse(&format!("{head}{src}")).unwrap();
        mark_cycles(&mut p.program);
        let is_marker = |s: &Stmt| matches!(s, Stmt::Expr(Expr::Call(_, a, _)) if matches!(a.first(), Some(Expr::Num(Num::Int(MARKER)))));
        let body = p.program.0.iter().rev().find_map(|s| if let Stmt::While(_, b) = s { Some(b) } else { None }).unwrap();
        body.0
            .iter()
            .map(|s| match s {
                _ if is_marker(s) => "M".to_string(),
                Stmt::For(_, _, _, b) => b.0.iter().filter(|s| is_marker(s)).count().to_string(),
                _ => ".".into(),
            })
            .collect()
    }

    #[test]
    fn markers_as_gscouter_places_them() {
        // the body is one cycle
        assert_eq!(marks("while 1 { hutao attack; xingqiu skill; }"), "..M");
        // one character acts: no cycle
        assert_eq!(marks("while 1 { hutao attack; hutao skill; }"), "..");
        // a for loop where both act is a cycle; what follows it with one
        // character is not
        assert_eq!(marks("while 1 { for let i = 0; i < 2; i = i + 1 { hutao attack; xingqiu skill; } hutao attack; }"), "1.");
        // statements between two such loops where both act are a cycle too
        assert_eq!(
            marks("while 1 { for let i = 0; i < 2; i = i + 1 { hutao attack; xingqiu skill; } hutao attack; xingqiu skill; for let i = 0; i < 2; i = i + 1 { hutao attack; xingqiu skill; } }"),
            "1..M1"
        );
        // branches count, and only the last top-level loop is marked
        assert_eq!(marks("while 0 { hutao attack; xingqiu skill; }\nwhile 1 { if 1 { hutao attack; } else { xingqiu skill; } }"), ".M");
    }
}
