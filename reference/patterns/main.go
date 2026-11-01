// Command patterns writes the cases of engine/src/pattern_tests.rs: dendro
// cores at random points around Gscouter's enemy (radius 2 at 0,2.4; cores
// 2.5 from its center, radius 2) against random circles, fans and boxes
// facing the enemy, and fans turned from it, with the answers of gcsim's own IntersectCircle and
// IntersectRectangle. Every number is rounded to an f32 first, as the
// engine keeps patterns.
//
// Usage:
//
//	go run ./patterns > cases.txt
package main

import (
	"fmt"
	"math"
	"math/rand"
	"strconv"
	"strings"

	"github.com/genshinsim/gcsim/pkg/core/info"
)

func main() {
	r := rand.New(rand.NewSource(1))
	dir := info.Point{X: 0, Y: 1}
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
			desc = fmt.Sprintf("Pattern::Fan { x: %s, y: %s, r: %s, fan: %s }", lit(ox), lit(oy), lit(rr), lit(fan))
		case 1:
			w, h := f32(0.1+r.Float64()*4), f32(0.5+r.Float64()*8)
			b := info.NewRectangle(info.Point{X: ox, Y: oy}, w, h, dir)
			hit = core.IntersectRectangle(*b)
			desc = fmt.Sprintf("Pattern::Box { x: %s, y: %s, w: %s, h: %s }", lit(ox), lit(oy), lit(w), lit(h))
		case 2:
			c := info.NewCircle(info.Point{X: ox, Y: oy}, rr, dir, 360)
			hit = core.IntersectCircle(*c)
			desc = fmt.Sprintf("Pattern::Circle { x: %s, y: %s, r: %s }", lit(ox), lit(oy), lit(rr))
		}
		fmt.Printf("(%s, %s, %s, %v),\n", desc, lit(cx), lit(cy), hit)
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
		desc := fmt.Sprintf("Pattern::FanDir { x: %s, y: %s, r: %s, fan: %s, dir: %s }", lit(ox), lit(oy), lit(rr), lit(fan), lit(deg))
		fmt.Printf("(%s, %s, %s, %v),\n", desc, lit(cx), lit(cy), hit)
	}
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
