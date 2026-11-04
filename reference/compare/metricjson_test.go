package main

import (
	"encoding/json"
	"math"
	"strings"
	"testing"
)

// A report with an infinite difference (an idle build, gcsim 0) encodes,
// with the difference null; a finite one is written as before.
func TestMetricJSONInfinite(t *testing.T) {
	rep := Report{Results: []Result{{Team: "a", Variant: "library", Verdict: "idle", Metrics: []Metric{
		{Name: "team", Gcsim: 0, Gse: 0.01, Diff: math.Inf(1), DiffSE: math.NaN(), Verdict: "fail"},
		{Name: "x", Gcsim: 100, Gse: 101, Diff: 0.01, DiffSE: 0.002, Verdict: "pass"},
	}}}}
	b, err := json.Marshal(rep)
	if err != nil {
		t.Fatal(err)
	}
	s := string(b)
	for _, want := range []string{`"diff":null`, `"diff_se":null`, `"diff":0.01`, `"diff_se":0.002`, `"gse":0.01`, `"verdict":"fail"`} {
		if !strings.Contains(s, want) {
			t.Errorf("JSON lacks %s: %s", want, s)
		}
	}
	if strings.Count(s, `"diff":`) != 2 {
		t.Errorf("diff written twice: %s", s)
	}
}
