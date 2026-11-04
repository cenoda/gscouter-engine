package main

import "testing"

func TestStorable(t *testing.T) {
	for _, c := range []struct {
		r    Result
		want bool
	}{
		{Result{Verdict: "pass"}, true},
		{Result{Verdict: "fail"}, true},
		{Result{Verdict: "idle"}, true},
		{Result{Verdict: "both-error"}, true},
		{Result{Verdict: "inconclusive"}, false},
		{Result{Verdict: "error"}, false},
		{Result{Verdict: "unsupported"}, false},
		{Result{Verdict: "deferred"}, false},
		// a verdict decided by its estimate can change with more seeds cached
		{Result{Verdict: "pass", Metrics: []Metric{{Verdict: "pass"}, {Verdict: "pass", Settled: true}}}, false},
	} {
		if got := storable(c.r); got != c.want {
			t.Errorf("storable(%+v) = %v, want %v", c.r, got, c.want)
		}
	}
}
