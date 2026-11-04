package main

import (
	"encoding/json"
	"math"
)

// MarshalJSON writes a difference JSON cannot hold as null: an idle
// build whose gcsim DPS is 0 and whose engine DPS is not has an infinite
// difference (the report shows ∞), and json.Marshal refuses the whole
// report for it. The verdict stands beside it.
func (m Metric) MarshalJSON() ([]byte, error) {
	type plain Metric
	out := struct {
		plain
		Diff   *float64 `json:"diff"`
		DiffSE *float64 `json:"diff_se"`
	}{plain: plain(m)}
	finite := func(x float64) *float64 {
		if math.IsInf(x, 0) || math.IsNaN(x) {
			return nil
		}
		return &x
	}
	out.Diff, out.DiffSE = finite(m.Diff), finite(m.DiffSE)
	return json.Marshal(out)
}
