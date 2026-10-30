// Package harness runs a gcsim config the way Gscouter scores it and
// returns the numbers the Rust engine is checked against: team and
// character DPS over a window, their standard errors, waits, and hits and
// damage by ability.
package harness

import (
	"errors"
	"math"
	"sync"
	"time"

	"github.com/genshinsim/gcsim/pkg/core/event"
	"github.com/genshinsim/gcsim/pkg/core/info"
	"github.com/genshinsim/gcsim/pkg/gcs/ast"
	"github.com/genshinsim/gcsim/pkg/gcs/eval"
	"github.com/genshinsim/gcsim/pkg/gcs/parser"
	"github.com/genshinsim/gcsim/pkg/simulation"
	"github.com/genshinsim/gcsim/pkg/stats"

	_ "github.com/genshinsim/gcsim/pkg/simulator" // registers characters, weapons, sets
)

const bucketFrames = 30

type Stat struct {
	Mean   float64 `json:"mean"`
	StdErr float64 `json:"stderr"`
}

type Output struct {
	Iterations int             `json:"iterations"`
	Warmup     float64         `json:"warmup"`
	Seconds    float64         `json:"seconds"`
	Workers    int             `json:"workers"`
	Team       Stat            `json:"team"`
	Chars      map[string]Stat `json:"chars"`
	Frames     float64         `json:"frames"`
	// Waits is seconds per iteration in the window that actions waited, by character and reason.
	Waits map[string]map[string]float64 `json:"waits"`
	// Abils is hits and damage per iteration in the window, by "char/ability".
	Abils map[string][2]float64 `json:"abils,omitempty"`
}

type iteration struct {
	team   float64
	chars  []float64
	names  []string
	frames int
	waits  []map[string]float64
}

// Run runs n iterations of the config, iteration i with seed seed+i, and
// averages them over the window from warmup seconds to the end. With abil
// it also counts hits and damage by ability.
func Run(cfg string, n int, warmup float64, seed int64, workers int, abil bool) (*Output, error) {
	file := ast.NewFile()
	simcfg, gcsl, err := parser.New(file, cfg).Parse()
	if err != nil {
		return nil, err
	}
	if len(simcfg.Errors) > 0 {
		return nil, errors.Join(simcfg.Errors...)
	}
	simcfg.Settings.CollectStats = []string{"damage", "action", "shield"}
	startBucket := int(warmup*60) / bucketFrames
	ab := &abils{on: abil, start: startBucket * bucketFrames, total: map[string][2]float64{}}

	start := time.Now()
	results := make([]iteration, n)
	errs := make([]error, n)
	jobs := make(chan int)
	var wg sync.WaitGroup
	for range workers {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for i := range jobs {
				res, err := once(file, simcfg, gcsl, seed+int64(i), ab)
				if err != nil {
					errs[i] = err
					continue
				}
				results[i] = summarize(&res, startBucket)
			}
		}()
	}
	for i := range n {
		jobs <- i
	}
	close(jobs)
	wg.Wait()
	if err := errors.Join(errs...); err != nil {
		return nil, err
	}
	elapsed := time.Since(start).Seconds()

	out := &Output{Iterations: n, Warmup: warmup, Seconds: elapsed, Workers: workers, Chars: map[string]Stat{}}
	team := make([]float64, n)
	var frames float64
	for i, r := range results {
		team[i] = r.team
		frames += float64(r.frames) / float64(n)
	}
	out.Team = stat(team)
	out.Frames = frames
	for c, name := range results[0].names {
		xs := make([]float64, n)
		for i, r := range results {
			xs[i] = r.chars[c]
		}
		out.Chars[name] = stat(xs)
	}
	if abil {
		out.Abils = map[string][2]float64{}
		for k, v := range ab.total {
			out.Abils[k] = [2]float64{v[0] / float64(n), v[1] / float64(n)}
		}
	}
	out.Waits = map[string]map[string]float64{}
	for c, name := range results[0].names {
		out.Waits[name] = map[string]float64{}
		for _, r := range results {
			for k, v := range r.waits[c] {
				out.Waits[name][k] += v / float64(n)
			}
		}
	}
	return out, nil
}

type abils struct {
	on    bool
	start int
	mu    sync.Mutex
	total map[string][2]float64
}

func once(file *ast.File, simcfg *info.ActionList, gcsl ast.Node, seed int64, ab *abils) (stats.Result, error) {
	cfg := simcfg.Copy()
	c, err := simulation.NewCore(seed, false, cfg)
	if err != nil {
		return stats.Result{}, err
	}
	ev, err := eval.NewEvaluator(file, gcsl.Copy(), c)
	if err != nil {
		return stats.Result{}, err
	}
	sim, err := simulation.New(cfg, ev, c)
	if err != nil {
		return stats.Result{}, err
	}
	if ab.on {
		local := map[string][2]float64{}
		c.Events.Subscribe(event.OnEnemyDamage, func(args ...any) {
			if c.F < ab.start {
				return
			}
			atk := args[1].(*info.AttackEvent)
			k := c.Player.ByIndex(atk.Info.ActorIndex).Base.Key.String() + "/" + atk.Info.Abil
			v := local[k]
			v[0]++
			v[1] += args[2].(float64)
			local[k] = v
		}, "abil")
		res, err := sim.Run()
		ab.mu.Lock()
		for k, v := range local {
			a := ab.total[k]
			a[0] += v[0]
			a[1] += v[1]
			ab.total[k] = a
		}
		ab.mu.Unlock()
		return res, err
	}
	return sim.Run()
}

// summarize is Gscouter's window: damage from startBucket to the end.
func summarize(res *stats.Result, startBucket int) iteration {
	startFrame := startBucket * bucketFrames
	window := float64(res.Duration-startFrame) / 60
	it := iteration{chars: make([]float64, len(res.Characters)), names: make([]string, len(res.Characters)), frames: res.Duration, waits: make([]map[string]float64, len(res.Characters))}
	cumulative := make([]float64, len(res.DamageBuckets))
	var sum float64
	for i, d := range res.DamageBuckets {
		sum += d
		cumulative[i] = sum
		if i >= startBucket {
			it.team += d
		}
	}
	it.team /= window
	cc := func(c, i int) float64 {
		if i < 0 {
			return 0
		}
		i = min(i, len(cumulative)-1)
		return res.Characters[c].DamageCumulativeContrib[i] * cumulative[i]
	}
	last := len(cumulative) - 1
	for c := range res.Characters {
		it.names[c] = res.Characters[c].Name
		if last >= 0 {
			it.chars[c] = (cc(c, last) - cc(c, startBucket-1)) / window
		}
		it.waits[c] = map[string]float64{}
		for _, f := range res.Characters[c].FailedActions {
			if overlap := min(f.End, res.Duration) - max(f.Start, startFrame); overlap > 0 {
				it.waits[c][f.Reason] += float64(overlap) / 60
			}
		}
	}
	return it
}

func stat(xs []float64) Stat {
	var m float64
	for _, x := range xs {
		m += x
	}
	m /= float64(len(xs))
	var v float64
	for _, x := range xs {
		v += (x - m) * (x - m)
	}
	if len(xs) > 1 {
		v /= float64(len(xs) - 1)
	}
	return Stat{Mean: m, StdErr: math.Sqrt(v / float64(len(xs)))}
}
