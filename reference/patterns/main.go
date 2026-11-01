// Command patterns writes engine/src/pattern_tests.rs: dendro cores at
// random points around Gscouter's enemy (radius 2 at 0,2.4; cores 2.5 from
// its center, radius 2) against random circles, fans and boxes facing the
// enemy, and fans turned from it, with the answers of gcsim's own
// IntersectCircle and IntersectRectangle; then fans and boxes facing any
// direction (as they do once set_player_pos moves the player) against
// random circles, and points against circles, fans and boxes (gcsim's
// PointInShape, which IsWithinArea asks). Every number of the first cases
// is rounded to an f32 first, as the engine kept patterns; directions are
// written in full.
//
// Usage:
//
//	go run ./patterns > ../engine/src/pattern_tests.rs
package main

import (
	"fmt"
	"math"
	"math/rand"
	"strconv"
	"strings"

	"github.com/genshinsim/gcsim/pkg/core/info"
)

const header = `//! Hit patterns against dendro cores, with the answers of gcsim's own
//! shape code: ` + "`cd reference && go run ./patterns > ../engine/src/pattern_tests.rs`" + `
//! writes this file.

use super::Pattern;

/// Cores at random points around the enemy against random patterns
/// (and fans turned from the enemy, as Chevreuse's mines),
/// with the answers of gcsim's own IntersectCircle and
/// IntersectRectangle on the same numbers.
#[test]
fn patterns_hit_as_gcsim() {
    let cases: &[(Pattern, f64, f64, bool)] = &[
`

const turned = `    ];
    for (i, &(p, x, y, hit)) in cases.iter().enumerate() {
        assert_eq!(p.hits_circle(x, y, 2.0), Some(hit), "case {i}: {p:?} on ({x}, {y})");
    }
}

/// Fans and boxes facing any direction (the player's, once it has moved)
/// against circles of any radius: (pattern, x, y, radius, hit).
#[test]
fn turned_patterns_hit_as_gcsim() {
    let cases: &[(Pattern, f64, f64, f64, bool)] = &[
`

const points = `    ];
    for (i, &(p, x, y, r, hit)) in cases.iter().enumerate() {
        assert_eq!(p.hits_circle(x, y, r), Some(hit), "case {i}: {p:?} on ({x}, {y}) r {r}");
    }
}

/// Points in circles, fans and boxes facing any direction (gcsim's
/// PointInShape, which IsWithinArea asks).
#[test]
fn points_in_patterns_as_gcsim() {
    let cases: &[(Pattern, f64, f64, bool)] = &[
`

const footer = `    ];
    for (i, &(p, x, y, inside)) in cases.iter().enumerate() {
        assert_eq!(p.contains_point(x, y), Some(inside), "case {i}: {p:?} at ({x}, {y})");
    }
}

/// Fans and boxes carry their direction: an attack is two f64 wider than
/// before set_player_pos.
#[test]
fn pattern_size() {
    assert_eq!(std::mem::size_of::<Pattern>(), 56);
}
`

func main() {
	r := rand.New(rand.NewSource(1))
	dir := info.Point{X: 0, Y: 1}
	fmt.Print(header)
	for i := range 400 {
		a := r.Float64() * 2 * math.Pi
		cx, cy := f32(2.5*math.Sin(a)), f32(2.4+2.5*math.Cos(a))
		core := info.NewCircle(info.Point{X: cx, Y: cy}, 2, dir, 360)
		ox, oy := f32(r.Float64()*2-1), f32(r.Float64()*6-2)
		rr := f32(0.1 + r.Float64()*3)
		var hit bool
		var desc string
		switch i % 3 {
		case 0:
			fan := f32(10 + r.Float64()*340)
			c := info.NewCircle(info.Point{X: ox, Y: oy}, rr, dir, fan)
			hit = core.IntersectCircle(*c)
			desc = fmt.Sprintf("Pattern::Fan { x: %s, y: %s, r: %s, fan: %s, dx: 0.0, dy: 1.0 }", lit(ox), lit(oy), lit(rr), lit(fan))
		case 1:
			w, h := f32(0.1+r.Float64()*4), f32(0.5+r.Float64()*8)
			b := info.NewRectangle(info.Point{X: ox, Y: oy}, w, h, dir)
			hit = core.IntersectRectangle(*b)
			desc = fmt.Sprintf("Pattern::Box { x: %s, y: %s, w: %s, h: %s, dx: 0.0, dy: 1.0 }", lit(ox), lit(oy), lit(w), lit(h))
		case 2:
			c := info.NewCircle(info.Point{X: ox, Y: oy}, rr, dir, 360)
			hit = core.IntersectCircle(*c)
			desc = fmt.Sprintf("Pattern::Circle { x: %s, y: %s, r: %s }", lit(ox), lit(oy), lit(rr))
		}
		fmt.Printf("        (%s, %s, %s, %v),\n", desc, lit(cx), lit(cy), hit)
	}
	// fans turned by a whole number of degrees, as Chevreuse's burst mines
	// (a fan of 60 degrees and radius 6 on the enemy) and at random
	for i := range 200 {
		a := r.Float64() * 2 * math.Pi
		cx, cy := f32(2.5*math.Sin(a)), f32(2.4+2.5*math.Cos(a))
		core := info.NewCircle(info.Point{X: cx, Y: cy}, 2, dir, 360)
		deg := float64(r.Intn(360))
		ox, oy, rr, fan := 0.0, 2.4, 6.0, 60.0
		if i%2 == 1 {
			ox, oy = f32(r.Float64()*2-1), f32(r.Float64()*6-2)
			rr, fan = f32(0.1+r.Float64()*3), f32(10+r.Float64()*340)
		}
		c := info.NewCircle(info.Point{X: ox, Y: oy}, rr, info.DegreesToDirection(deg), fan)
		hit := core.IntersectCircle(*c)
		desc := fmt.Sprintf("Pattern::fan_dir(%s, %s, %s, %s, %s)", lit(ox), lit(oy), lit(rr), lit(fan), lit(deg))
		fmt.Printf("        (%s, %s, %s, %v),\n", desc, lit(cx), lit(cy), hit)
	}
	// fans and boxes facing the enemy from a player moved anywhere near it
	// (CalcDirection, as set_player_pos turns the player), against circles
	fmt.Print(turned)
	for i := range 300 {
		p := info.Point{X: r.Float64()*8 - 4, Y: r.Float64()*8 - 4}
		d := info.CalcDirection(p, info.Point{X: 0, Y: 2.4})
		ox, oy := r.Float64()*2-1, r.Float64()*6-2
		c := info.CalcOffsetPoint(p, info.Point{X: ox, Y: oy}, d)
		t := info.Point{X: r.Float64()*10 - 5, Y: r.Float64()*10 - 3}
		tr := 0.3 + r.Float64()*2
		trg := info.NewCircle(t, tr, dir, 360)
		var hit bool
		var desc string
		switch i % 3 {
		case 0:
			rr, fan := 0.1+r.Float64()*4, 10+r.Float64()*340
			s := info.NewCircle(c, rr, d, fan)
			hit = trg.IntersectCircle(*s)
			desc = fmt.Sprintf("Pattern::Fan { x: %s, y: %s, r: %s, fan: %s, dx: %s, dy: %s }", l64(c.X), l64(c.Y), l64(rr), l64(fan), l64(d.X), l64(d.Y))
		case 1:
			w, h := 0.1+r.Float64()*6, 0.5+r.Float64()*10
			b := info.NewRectangle(c, w, h, d)
			hit = trg.IntersectRectangle(*b)
			desc = fmt.Sprintf("Pattern::Box { x: %s, y: %s, w: %s, h: %s, dx: %s, dy: %s }", l64(c.X), l64(c.Y), l64(w), l64(h), l64(d.X), l64(d.Y))
		case 2:
			rr, fan := 0.1+r.Float64()*4, 10+r.Float64()*340
			s := info.NewCircle(c, rr, d, fan)
			hit = trg.IntersectCircle(*s)
			desc = fmt.Sprintf("Pattern::FanDir { x: %s, y: %s, r: %s, fan: %s, dx: %s, dy: %s }", l64(c.X), l64(c.Y), l64(rr), l64(fan), l64(d.X), l64(d.Y))
		}
		fmt.Printf("        (%s, %s, %s, %s, %v),\n", desc, l64(t.X), l64(t.Y), l64(tr), hit)
	}
	// points against circles, fans and boxes facing anywhere
	fmt.Print(points)
	for i := range 300 {
		p := info.Point{X: r.Float64()*8 - 4, Y: r.Float64()*8 - 4}
		d := info.CalcDirection(p, info.Point{X: 0, Y: 2.4})
		if i%5 == 0 {
			d = dir
		}
		ox, oy := r.Float64()*2-1, r.Float64()*6-2
		c := info.CalcOffsetPoint(p, info.Point{X: ox, Y: oy}, d)
		t := info.Point{X: r.Float64()*10 - 5, Y: r.Float64()*10 - 3}
		var in bool
		var desc string
		switch i % 3 {
		case 0:
			rr, fan := 0.5+r.Float64()*6, 10+r.Float64()*340
			s := info.NewCircle(c, rr, d, fan)
			in = s.PointInShape(t)
			desc = fmt.Sprintf("Pattern::Fan { x: %s, y: %s, r: %s, fan: %s, dx: %s, dy: %s }", l64(c.X), l64(c.Y), l64(rr), l64(fan), l64(d.X), l64(d.Y))
		case 1:
			w, h := 0.5+r.Float64()*8, 0.5+r.Float64()*10
			b := info.NewRectangle(c, w, h, d)
			in = b.PointInShape(t)
			desc = fmt.Sprintf("Pattern::Box { x: %s, y: %s, w: %s, h: %s, dx: %s, dy: %s }", l64(c.X), l64(c.Y), l64(w), l64(h), l64(d.X), l64(d.Y))
		case 2:
			rr := 0.5 + r.Float64()*6
			s := info.NewCircle(c, rr, d, 360)
			in = s.PointInShape(t)
			desc = fmt.Sprintf("Pattern::Circle { x: %s, y: %s, r: %s }", l64(c.X), l64(c.Y), l64(rr))
		}
		fmt.Printf("        (%s, %s, %s, %v),\n", desc, l64(t.X), l64(t.Y), in)
	}
	fmt.Print(footer)
}

func f32(x float64) float64 { return float64(float32(x)) }

// lit is a Rust float literal of x, as short as an f32 needs.
func lit(x float64) string {
	s := strconv.FormatFloat(x, 'g', -1, 32)
	if !strings.ContainsAny(s, ".e") {
		s += ".0"
	}
	return s
}

// l64 is a Rust float literal of x, exact.
func l64(x float64) string {
	s := strconv.FormatFloat(x, 'g', -1, 64)
	if !strings.ContainsAny(s, ".e") {
		s += ".0"
	}
	return s
}
