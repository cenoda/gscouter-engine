package harness

import (
	"math"
	"os"
	"testing"
)

// A cached run, read back or not, gives what an uncached run of the same
// seeds gives, whole chunks or not.
func TestRunCached(t *testing.T) {
	raw, err := os.ReadFile("../../configs/hutao-solo-looped.txt")
	if err != nil {
		t.Fatal(err)
	}
	cfg := string(raw)
	c, err := OpenCache(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	close := func(a, b float64) bool { return math.Abs(a-b) <= 1e-9*math.Max(1, math.Abs(a)) }
	for _, tc := range []struct {
		seed int64
		n    int
	}{{1, 1100}, {251, 900}, {601, 200}} {
		want, err := Run(cfg, tc.n, 150, tc.seed, 4, true)
		if err != nil {
			t.Fatal(err)
		}
		for pass := range 2 {
			got, err := c.RunCached(cfg, tc.n, 150, tc.seed, 4, true)
			if err != nil {
				t.Fatal(err)
			}
			if got.Iterations != want.Iterations || !close(got.Team.Mean, want.Team.Mean) || !close(got.Team.StdErr, want.Team.StdErr) || !close(got.Frames, want.Frames) {
				t.Fatalf("seed %d n %d pass %d: team %+v %v, want %+v %v", tc.seed, tc.n, pass, got.Team, got.Iterations, want.Team, want.Iterations)
			}
			for k, w := range want.Chars {
				if g := got.Chars[k]; !close(g.Mean, w.Mean) || !close(g.StdErr, w.StdErr) {
					t.Fatalf("seed %d n %d pass %d: %s %+v, want %+v", tc.seed, tc.n, pass, k, g, w)
				}
			}
			for k, w := range want.Abils {
				if g := got.Abils[k]; !close(g[0], w[0]) || !close(g[1], w[1]) {
					t.Fatalf("seed %d n %d pass %d: %s %v, want %v", tc.seed, tc.n, pass, k, g, w)
				}
			}
			for ch, ws := range want.Waits {
				for r, w := range ws {
					if g := got.Waits[ch][r]; !close(g, w) {
						t.Fatalf("seed %d n %d pass %d: wait %s %s %v, want %v", tc.seed, tc.n, pass, ch, r, g, w)
					}
				}
			}
		}
	}
}
