package main

import (
	"testing"

	"github.com/cenoda/gscouter-engine/reference/harness"
)

func TestSameError(t *testing.T) {
	g := "error encountered on mavuika executing skill: cannot recast E while not in nightsoul blessing"
	cases := []struct {
		gse  string
		want bool
	}{
		{"gse: exit status 1: iteration 6772: mavuika: cannot recast E while not in nightsoul blessing", true},
		{"gse: exit status 1: iteration 3: mavuika: cannot hold E while recasting", false},
		{"gse: exit status 1: iteration 3: player must use low_plunge or high_plunge while airborne", false},
		{"", false},
	}
	for _, c := range cases {
		if got := sameError(g, c.gse); got != c.want {
			t.Errorf("sameError(%q) = %v, want %v", c.gse, got, c.want)
		}
	}
	// a multi-line gcsim error compares its first line
	if !sameError("error encountered on x executing burst: boom\nmore", "iteration 1: x: boom") {
		t.Errorf("first line of a multi-line error")
	}
	if m := gseIteration.FindStringSubmatch("gse: exit status 1: iteration 6772: mavuika: x"); m == nil || m[1] != "6772" {
		t.Errorf("gseIteration: %v", m)
	}
}

func TestIdle(t *testing.T) {
	out := func(team float64) *harness.Output { return &harness.Output{Team: harness.Stat{Mean: team}} }
	cases := []struct {
		g, r float64
		want bool
	}{
		{1.3, 1.2, true},
		{0, 0.3, true},
		{9.9, 0, true},
		{10, 0.5, false},
		{0.5, 12, false},
		{61000, 61010, false},
	}
	for _, c := range cases {
		if got := idle(out(c.g), out(c.r)); got != c.want {
			t.Errorf("idle(%v, %v) = %v, want %v", c.g, c.r, got, c.want)
		}
	}
}
