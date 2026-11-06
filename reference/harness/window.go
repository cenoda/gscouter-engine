package harness

// Gscouter's window for the site's score (internal/score/score.go at the
// Gscouter commit CI checks out): with no warmup on a fight with a
// duration (0–120 s from full energy), the program's last top-level while
// loop is marked into cycles, and the window ends at the bucket boundary
// nearest the last completed cycle, so that an unfinished cycle's setup at
// the end of the clock does not count. Copied from Gscouter: markCycles,
// multiChar, collectChars, and the marker handling of its stillGuard.

import (
	"github.com/genshinsim/gcsim/pkg/core"
	"github.com/genshinsim/gcsim/pkg/core/action"
	"github.com/genshinsim/gcsim/pkg/gcs/ast"
)

// Marker is Gscouter's score.Marker: a delay of this many frames or more
// marks a point of the program and is not played.
const Marker = 987654

// cutsAtCycles is whether a run's window ends at the last completed cycle.
func cutsAtCycles(warmup, duration float64) bool {
	return warmup == 0 && duration > 0
}

// syncEvaluator is a program gcsim runs on the fight's goroutine, as
// eval.Eval is.
type syncEvaluator interface {
	action.Evaluator
	RunSync(exec func(*action.Eval) bool) error
}

// cycleGuard records the markers the program reaches and does not pass
// them to gcsim (Gscouter's stillGuard, without its stuck check).
type cycleGuard struct {
	syncEvaluator
	c                          *core.Core
	cycles, cycleEnd, maxCycle int
}

func (g *cycleGuard) RunSync(exec func(*action.Eval) bool) error {
	return g.syncEvaluator.RunSync(func(e *action.Eval) bool {
		if e.Action == action.ActionDelay && e.Param["f"] >= Marker {
			if g.c.F > g.cycleEnd {
				g.cycles++
				g.maxCycle = max(g.maxCycle, g.c.F-g.cycleEnd)
				g.cycleEnd = g.c.F
			}
			return true
		}
		return exec(e)
	})
}

// endFrame is the frame the window ends on, for a fight of duration
// frames (Gscouter's simulateWith).
func (g *cycleGuard) endFrame(duration int) int {
	if g.cycleEnd > 0 && (g.cycles >= 2 || g.cycleEnd > duration/2) && duration-g.cycleEnd <= g.maxCycle*3/2 {
		return g.cycleEnd
	}
	return duration
}

func cycleMarker() ast.Node {
	return &ast.CallExpr{
		Fun:  &ast.Ident{Value: "delay"},
		Args: []ast.Expr{&ast.NumberLit{IntVal: Marker}},
	}
}

func markCycles(gcsl ast.Node) {
	root, ok := gcsl.(*ast.BlockStmt)
	if !ok {
		return
	}
	var loop *ast.WhileStmt
	for _, n := range root.List {
		if w, ok := n.(*ast.WhileStmt); ok {
			loop = w
		}
	}
	if loop == nil || loop.WhileBlock == nil || !multiChar(loop) {
		return
	}
	var list, since []ast.Node
	markedFor := false
	for _, n := range loop.WhileBlock.List {
		if f, ok := n.(*ast.ForStmt); ok && f.Body != nil && multiChar(f.Body) {
			if multiChar(since...) {
				list = append(list, cycleMarker())
			}
			since = nil
			f.Body.Append(cycleMarker())
			markedFor = true
		} else {
			since = append(since, n)
		}
		list = append(list, n)
	}
	if !markedFor || multiChar(since...) {
		list = append(list, cycleMarker())
	}
	loop.WhileBlock.List = list
}

// multiChar reports whether two or more characters execute actions in nodes.
func multiChar(nodes ...ast.Node) bool {
	chars := map[int64]bool{}
	collectChars(nodes, chars)
	return len(chars) >= 2
}

func collectChars(nodes []ast.Node, chars map[int64]bool) {
	for _, n := range nodes {
		switch s := n.(type) {
		case *ast.BlockStmt:
			if s != nil {
				collectChars(s.List, chars)
			}
		case *ast.IfStmt:
			if s.IfBlock != nil {
				collectChars(s.IfBlock.List, chars)
			}
			if s.ElseBlock != nil {
				collectChars([]ast.Node{s.ElseBlock}, chars)
			}
		case *ast.SwitchStmt:
			for _, c := range s.Cases {
				if c.Body != nil {
					collectChars(c.Body.List, chars)
				}
			}
			if s.Default != nil {
				collectChars(s.Default.List, chars)
			}
		case *ast.WhileStmt:
			if s.WhileBlock != nil {
				collectChars(s.WhileBlock.List, chars)
			}
		case *ast.ForStmt:
			if s.Body != nil {
				collectChars(s.Body.List, chars)
			}
		case *ast.CallExpr:
			if id, ok := s.Fun.(*ast.Ident); ok && id.Value == "execute_action" && len(s.Args) > 0 {
				if lit, ok := s.Args[0].(*ast.NumberLit); ok {
					chars[lit.IntVal] = true
				}
			}
		}
	}
}
