package harness

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"runtime/debug"
)

// Chunk is how many iterations the cache stores together: seeds 1 to
// Chunk, Chunk+1 to 2*Chunk, and so on.
const Chunk = 500

// version is part of every cache key. Raise it when Run or summarize
// changes what it measures, so that old results are not read back.
const version = 1

// Cache stores gcsim's results by config and chunk of seeds. gcsim is
// fixed by go.mod and a seed always gives the same fight, so a stored
// chunk is what running it again would give.
type Cache struct {
	dir   string
	gcsim string
}

// DefaultCacheDir is where the cache lives unless told otherwise.
func DefaultCacheDir() string {
	d, err := os.UserCacheDir()
	if err != nil {
		return ""
	}
	return filepath.Join(d, "gscouter-engine", "gcsim")
}

// OpenCache uses dir, or returns nil (no cache) when dir is empty.
func OpenCache(dir string) (*Cache, error) {
	if dir == "" {
		return nil, nil
	}
	info, ok := debug.ReadBuildInfo()
	if !ok {
		return nil, errors.New("cache: no build info to tell gcsim's version")
	}
	var gcsim string
	for _, m := range info.Deps {
		if m.Path == "github.com/genshinsim/gcsim" {
			gcsim = m.Path + "@" + m.Version
			if m.Replace != nil {
				gcsim = m.Replace.Path + "@" + m.Replace.Version
			}
		}
	}
	if gcsim == "" {
		return nil, errors.New("cache: gcsim is not among the dependencies")
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, err
	}
	return &Cache{dir: dir, gcsim: gcsim}, nil
}

func (c *Cache) path(cfg string, warmup float64, abil bool, chunk int) string {
	h := sha256.New()
	fmt.Fprintf(h, "v%d\n%s\nwarmup=%v abil=%v\n%s", version, c.gcsim, warmup, abil, cfg)
	key := hex.EncodeToString(h.Sum(nil))
	return filepath.Join(c.dir, key[:2], fmt.Sprintf("%s-%d.json", key, chunk))
}

func (c *Cache) load(path string) *Output {
	b, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	var out Output
	if json.Unmarshal(b, &out) != nil || out.Iterations != Chunk {
		return nil
	}
	return &out
}

func (c *Cache) store(path string, out *Output) error {
	b, err := json.Marshal(out)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	// several runs may write the same chunk; a rename never shows half
	tmp, err := os.CreateTemp(filepath.Dir(path), ".tmp-*")
	if err != nil {
		return err
	}
	if _, err := tmp.Write(b); err != nil {
		tmp.Close()
		os.Remove(tmp.Name())
		return err
	}
	if err := tmp.Close(); err != nil {
		os.Remove(tmp.Name())
		return err
	}
	return os.Rename(tmp.Name(), path)
}

// RunCached is Run, with the whole chunks of seeds in [seed, seed+n) read
// from the cache, or run and stored. The rest runs uncached. Without a
// cache it is Run.
func (c *Cache) RunCached(cfg string, n int, warmup float64, seed int64, workers int, abil bool) (*Output, error) {
	if c == nil {
		return Run(cfg, n, warmup, seed, workers, abil)
	}
	var out *Output
	add := func(seed int64, n int) error {
		if n == 0 {
			return nil
		}
		b, err := Run(cfg, n, warmup, seed, workers, abil)
		if err != nil {
			return err
		}
		out = Merge(out, b)
		return nil
	}
	end := seed + int64(n)
	// the first seed of the first whole chunk
	first := seed
	if r := (seed - 1) % Chunk; r != 0 {
		first = seed - r + Chunk
	}
	if first >= end {
		return Run(cfg, n, warmup, seed, workers, abil)
	}
	if err := add(seed, int(first-seed)); err != nil {
		return nil, err
	}
	s := first
	for ; s+Chunk <= end; s += Chunk {
		path := c.path(cfg, warmup, abil, int((s-1)/Chunk))
		b := c.load(path)
		if b == nil {
			var err error
			if b, err = Run(cfg, Chunk, warmup, s, workers, abil); err != nil {
				return nil, err
			}
			if err := c.store(path, b); err != nil {
				return nil, err
			}
		}
		out = Merge(out, b)
	}
	if err := add(s, int(end-s)); err != nil {
		return nil, err
	}
	return out, nil
}

// Merge combines two batches of iterations into one, as if run together
// (up to rounding).
func Merge(a, b *Output) *Output {
	if a == nil {
		return b
	}
	na, nb := float64(a.Iterations), float64(b.Iterations)
	n := na + nb
	st := func(x, y Stat) Stat {
		m := (na*x.Mean + nb*y.Mean) / n
		// sums of squares about each batch's mean, from its standard error
		ss := x.StdErr*x.StdErr*na*(na-1) + y.StdErr*y.StdErr*nb*(nb-1) + na*(x.Mean-m)*(x.Mean-m) + nb*(y.Mean-m)*(y.Mean-m)
		return Stat{Mean: m, StdErr: math.Sqrt(ss / (n - 1) / n)}
	}
	out := &Output{Iterations: a.Iterations + b.Iterations, Warmup: a.Warmup, Seconds: a.Seconds + b.Seconds, Workers: a.Workers,
		Team: st(a.Team, b.Team), Chars: map[string]Stat{}, Frames: (na*a.Frames + nb*b.Frames) / n,
		Waits: map[string]map[string]float64{}}
	if a.Abils != nil || b.Abils != nil {
		out.Abils = map[string][2]float64{}
	}
	for k := range a.Chars {
		out.Chars[k] = st(a.Chars[k], b.Chars[k])
	}
	for _, w := range []*Output{a, b} {
		f := float64(w.Iterations) / n
		for c, rs := range w.Waits {
			if out.Waits[c] == nil {
				out.Waits[c] = map[string]float64{}
			}
			for r, v := range rs {
				out.Waits[c][r] += v * f
			}
		}
		for k, v := range w.Abils {
			x := out.Abils[k]
			out.Abils[k] = [2]float64{x[0] + v[0]*f, x[1] + v[1]*f}
		}
	}
	return out
}
