// Command compare runs Gscouter's library teams under gcsim and under the
// Rust engine (gse) and checks that they agree within 1%.
//
// For each team it assembles the looped config Gscouter scores (gscouter
// -dump), asks gse what in it the engine does not implement (gse --check),
// and, when nothing is missing, runs the config under both engines: the
// library build and builds with their stats moved (ER, crit, ATK, HP, EM),
// including one without ER from artifacts, whose waits for energy decide
// Gscouter's fillers. It compares team DPS, per-character DPS, waits and
// per-ability hits and damage.
//
// Team and character DPS pass when the 95% confidence interval of the
// difference lies within ±1% of gcsim, fail when it lies outside, and are
// inconclusive otherwise. Iterations start at -iter, grow until each
// engine's standard error is at most -target-se of the mean, and grow
// further, up to -max-iter, while any verdict is inconclusive.
//
// Teams the engine cannot run are counted as "unsupported: <what>", and the
// census of what blocks how many teams measures the port's progress.
//
// Usage:
//
//	compare -gscouter ../Gscouter [-teams id,id] [-census] [-json out.json] [-md out.md]
package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"math"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"slices"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/cenoda/gscouter-engine/reference/harness"
)

// bound is the agreement Gscouter needs between the engines.
const bound = 0.01

// z is the two-sided 95% normal quantile.
const z = 1.96

var (
	gscouterDir = flag.String("gscouter", "", "Gscouter checkout (its teams/ and cmd/gscouter)")
	gsePath     = flag.String("gse", "../engine/target/release/gse", "the engine's executable")
	teamsFlag   = flag.String("teams", "", "comma-separated team ids (default: the whole library)")
	censusOnly  = flag.Bool("census", false, "only classify teams as supported or not; run nothing")
	noVariants  = flag.Bool("no-variants", false, "compare library builds only")
	iter0       = flag.Int("iter", 1000, "iterations of the first batch")
	maxIter     = flag.Int("max-iter", 512000, "most iterations a config may grow to")
	targetSE    = flag.Float64("target-se", 0.003, "standard error, relative to the mean, each engine must reach")
	warmup      = flag.Float64("warmup", 150, "seconds of the looped fight excluded from the window")
	workers     = flag.Int("j", runtime.NumCPU(), "workers")
	jsonOut     = flag.String("json", "", "write the results as JSON to this file")
	mdOut       = flag.String("md", "", "also write the report to this file")
)

// A variant moves stats of every character's build.
type variant struct {
	Name string `json:"name"`
	Desc string `json:"desc"`
	ops  []op
	// force uses every burst without its readiness check, so that it
	// waits for energy.
	force bool
}

// op scales a stat in every `add stats` line, then adds to the last one.
type op struct {
	stat  string
	scale float64
	add   float64
}

var variants = []variant{
	{"er-0", "no ER from artifacts", []op{{"er", 0, 0}}, false},
	{"er-0-forced", "no ER from artifacts, bursts used without their readiness checks so that they wait for energy", []op{{"er", 0, 0}}, true},
	{"er-half", "half the ER from artifacts", []op{{"er", 0.5, 0}}, false},
	{"er-half-forced", "half the ER from artifacts, bursts used without their readiness checks", []op{{"er", 0.5, 0}}, true},
	{"er+", "ER +30%", []op{{"er", 1, 0.3}}, false},
	{"crit-", "CR −20%, CD −40%", []op{{"cr", 1, -0.2}, {"cd", 1, -0.4}}, false},
	{"crit+", "CR +20%, CD +40%", []op{{"cr", 1, 0.2}, {"cd", 1, 0.4}}, false},
	{"atk+", "ATK +40%", []op{{"atk%", 1, 0.4}}, false},
	{"hp+", "HP +40%", []op{{"hp%", 1, 0.4}}, false},
	{"em+", "EM +150", []op{{"em", 1, 150}}, false},
}

type Metric struct {
	Name    string  `json:"name"`
	Gcsim   float64 `json:"gcsim"`
	GcsimSE float64 `json:"gcsim_se"`
	Gse     float64 `json:"gse"`
	GseSE   float64 `json:"gse_se"`
	// Diff is (gse - gcsim) / gcsim; DiffSE its standard error.
	Diff    float64 `json:"diff"`
	DiffSE  float64 `json:"diff_se"`
	Verdict string  `json:"verdict"`
}

type Wait struct {
	Char  string             `json:"char"`
	Gcsim map[string]float64 `json:"gcsim"`
	Gse   map[string]float64 `json:"gse"`
}

type Abil struct {
	Name      string  `json:"name"`
	GcsimHits float64 `json:"gcsim_hits"`
	GseHits   float64 `json:"gse_hits"`
	GcsimDmg  float64 `json:"gcsim_dmg"`
	GseDmg    float64 `json:"gse_dmg"`
}

type Result struct {
	Team    string   `json:"team"`
	Variant string   `json:"variant"`
	Desc    string   `json:"desc"`
	Iter    int      `json:"iterations"`
	Verdict string   `json:"verdict"`
	Error   string   `json:"error,omitempty"`
	Metrics []Metric `json:"metrics,omitempty"`
	Waits   []Wait   `json:"waits,omitempty"`
	Abils   []Abil   `json:"abils,omitempty"`
	// EnergyWaits is gcsim's seconds per fight waiting for energy.
	EnergyWaits  float64 `json:"energy_waits"`
	GcsimSeconds float64 `json:"gcsim_seconds"`
	GseSeconds   float64 `json:"gse_seconds"`
}

type TeamStatus struct {
	ID          string   `json:"id"`
	Status      string   `json:"status"` // supported, unsupported, error
	Unsupported []string `json:"unsupported,omitempty"`
	Error       string   `json:"error,omitempty"`
}

type Blocker struct {
	Name  string `json:"name"`
	Teams int    `json:"teams"`
	// Only counts the teams it alone blocks.
	Only int `json:"only"`
}

type Census struct {
	Teams       int            `json:"teams"`
	Supported   int            `json:"supported"`
	Unsupported int            `json:"unsupported"`
	Errors      int            `json:"errors"`
	Blockers    []Blocker      `json:"blockers"`
	ByKind      map[string]int `json:"by_kind"`
	Status      []TeamStatus   `json:"status"`
}

type Report struct {
	Date     string    `json:"date"`
	Gscouter string    `json:"gscouter_commit"`
	Bound    float64   `json:"bound"`
	TargetSE float64   `json:"target_se"`
	Warmup   float64   `json:"warmup"`
	Variants []variant `json:"variants"`
	Results  []Result  `json:"results"`
	Census   Census    `json:"census"`
	Pass     bool      `json:"pass"`
}

func main() {
	flag.Parse()
	if *gscouterDir == "" {
		fail(errors.New("-gscouter is required"))
	}
	tmp, err := os.MkdirTemp("", "compare")
	if err != nil {
		fail(err)
	}
	defer os.RemoveAll(tmp)
	gscouter := filepath.Join(tmp, "gscouter")
	build := exec.Command("go", "build", "-o", gscouter, "./cmd/gscouter")
	build.Dir = *gscouterDir
	if out, err := build.CombinedOutput(); err != nil {
		fail(fmt.Errorf("building gscouter: %v\n%s", err, out))
	}
	gse, err := filepath.Abs(*gsePath)
	if err != nil {
		fail(err)
	}

	ids := listTeams()
	if *teamsFlag != "" {
		ids = strings.Split(*teamsFlag, ",")
	}
	rep := &Report{Date: time.Now().UTC().Format(time.RFC3339), Gscouter: gitHead(*gscouterDir), Bound: bound, TargetSE: *targetSE, Warmup: *warmup}
	if !*noVariants {
		rep.Variants = variants
	}

	// census: dump and check every team
	statuses := make([]TeamStatus, len(ids))
	configs := make([]string, len(ids))
	parallel(len(ids), *workers, func(i int) {
		id := ids[i]
		st := TeamStatus{ID: id}
		cfg, err := dump(gscouter, id, "")
		if err != nil {
			st.Status, st.Error = "error", err.Error()
		} else if miss, err := check(gse, tmp, id, cfg); err != nil {
			st.Status, st.Error = "error", err.Error()
		} else if len(miss) > 0 {
			st.Status, st.Unsupported = "unsupported", miss
		} else {
			st.Status = "supported"
			configs[i] = cfg
		}
		statuses[i] = st
	})

	if !*censusOnly {
		for i, id := range ids {
			if statuses[i].Status != "supported" {
				continue
			}
			jobs := []struct {
				v   variant
				cfg string
			}{{variant{Name: "library", Desc: "library build"}, configs[i]}}
			if !*noVariants {
				base, err := os.ReadFile(filepath.Join(*gscouterDir, "teams", id, "build.txt"))
				if err != nil {
					fail(err)
				}
				for _, v := range variants {
					path := filepath.Join(tmp, id+"-"+v.Name+".txt")
					if err := os.WriteFile(path, []byte(v.apply(string(base))), 0o644); err != nil {
						fail(err)
					}
					cfg, err := dump(gscouter, id, path)
					if err != nil {
						fail(fmt.Errorf("%s %s: %w", id, v.Name, err))
					}
					if v.force {
						forced := burstReady.ReplaceAllString(cfg, "1")
						if forced == cfg {
							// every burst already waits for energy: er-0 covers it
							continue
						}
						cfg = forced
					}
					jobs = append(jobs, struct {
						v   variant
						cfg string
					}{v, cfg})
				}
			}
			for _, j := range jobs {
				fmt.Fprintf(os.Stderr, "%s / %s ...\n", id, j.v.Name)
				r := compare(gse, tmp, id, j.v, j.cfg)
				fmt.Fprintf(os.Stderr, "%s / %s: %s at %d iterations\n", id, j.v.Name, r.Verdict, r.Iter)
				// a feature only the run finds makes the team unsupported
				if r.Verdict == "unsupported" {
					statuses[i].Status = "unsupported"
					statuses[i].Unsupported = []string{r.Error}
				}
				rep.Results = append(rep.Results, r)
			}
		}
	}
	rep.Census = census(statuses)
	rep.Pass = true
	for _, r := range rep.Results {
		if r.Verdict != "pass" && r.Verdict != "unsupported" {
			rep.Pass = false
		}
	}

	md := markdown(rep)
	fmt.Print(md)
	if *mdOut != "" {
		if err := os.WriteFile(*mdOut, []byte(md), 0o644); err != nil {
			fail(err)
		}
	}
	if *jsonOut != "" {
		b, _ := json.MarshalIndent(rep, "", "  ")
		if err := os.WriteFile(*jsonOut, b, 0o644); err != nil {
			fail(err)
		}
	}
	if !rep.Pass {
		os.Exit(1)
	}
}

func fail(err error) {
	fmt.Fprintln(os.Stderr, "error:", err)
	os.Exit(2)
}

func listTeams() []string {
	ents, err := os.ReadDir(filepath.Join(*gscouterDir, "teams"))
	if err != nil {
		fail(err)
	}
	var ids []string
	for _, e := range ents {
		if e.IsDir() {
			ids = append(ids, e.Name())
		}
	}
	return ids
}

func gitHead(dir string) string {
	out, err := exec.Command("git", "-C", dir, "rev-parse", "HEAD").Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(out))
}

func parallel(n, workers int, f func(i int)) {
	jobs := make(chan int)
	var wg sync.WaitGroup
	for range max(workers, 1) {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for i := range jobs {
				f(i)
			}
		}()
	}
	for i := range n {
		jobs <- i
	}
	close(jobs)
	wg.Wait()
}

// dump is the looped config Gscouter assembles for the team, with its
// library build or the build file given.
func dump(gscouter, id, build string) (string, error) {
	dir := filepath.Join("teams", id)
	if build == "" {
		build = filepath.Join(dir, "build.txt")
	}
	cmd := exec.Command(gscouter, "-team", dir, "-build", build, "-dump")
	cmd.Dir = *gscouterDir
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	out, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("gscouter -dump: %v: %s", err, strings.TrimSpace(stderr.String()))
	}
	return string(out), nil
}

func writeConfig(tmp, name, cfg string) string {
	path := filepath.Join(tmp, name+".cfg")
	if err := os.WriteFile(path, []byte(cfg), 0o644); err != nil {
		fail(err)
	}
	return path
}

// check is what the config needs that the engine does not implement.
func check(gse, tmp, id, cfg string) ([]string, error) {
	path := writeConfig(tmp, id+"-check", cfg)
	var stderr bytes.Buffer
	cmd := exec.Command(gse, "--check", path)
	cmd.Stderr = &stderr
	out, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("gse --check: %v: %s", err, strings.TrimSpace(stderr.String()))
	}
	var res struct {
		Unsupported []string `json:"unsupported"`
	}
	if err := json.Unmarshal(out, &res); err != nil {
		return nil, fmt.Errorf("gse --check: %v", err)
	}
	return res.Unsupported, nil
}

var errUnsupported = errors.New("unsupported")

func runGse(gse, path string, n int, seed int64) (*harness.Output, error) {
	var stderr bytes.Buffer
	cmd := exec.Command(gse, path, strconv.Itoa(n), fmt.Sprint(*warmup), strconv.Itoa(*workers), "--seed", strconv.FormatInt(seed, 10))
	cmd.Stderr = &stderr
	out, err := cmd.Output()
	if err != nil {
		msg := strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(stderr.String()), "error: "))
		if i := strings.Index(msg, "unsupported: "); i >= 0 {
			return nil, fmt.Errorf("%w: %s", errUnsupported, msg[i+len("unsupported: "):])
		}
		return nil, fmt.Errorf("gse: %v: %s", err, msg)
	}
	var res harness.Output
	if err := json.Unmarshal(out, &res); err != nil {
		return nil, fmt.Errorf("gse output: %v", err)
	}
	return &res, nil
}

// merge combines two batches of iterations into one, as if run together.
func merge(a, b *harness.Output) *harness.Output {
	if a == nil {
		return b
	}
	na, nb := float64(a.Iterations), float64(b.Iterations)
	n := na + nb
	st := func(x, y harness.Stat) harness.Stat {
		m := (na*x.Mean + nb*y.Mean) / n
		// sums of squares about each batch's mean, from its standard error
		ss := x.StdErr*x.StdErr*na*(na-1) + y.StdErr*y.StdErr*nb*(nb-1) + na*(x.Mean-m)*(x.Mean-m) + nb*(y.Mean-m)*(y.Mean-m)
		return harness.Stat{Mean: m, StdErr: math.Sqrt(ss / (n - 1) / n)}
	}
	out := &harness.Output{Iterations: a.Iterations + b.Iterations, Warmup: a.Warmup, Seconds: a.Seconds + b.Seconds, Workers: a.Workers,
		Team: st(a.Team, b.Team), Chars: map[string]harness.Stat{}, Frames: (na*a.Frames + nb*b.Frames) / n,
		Waits: map[string]map[string]float64{}, Abils: map[string][2]float64{}}
	for k := range a.Chars {
		out.Chars[k] = st(a.Chars[k], b.Chars[k])
	}
	for _, w := range []*harness.Output{a, b} {
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

func metric(name string, g, r harness.Stat) Metric {
	m := Metric{Name: name, Gcsim: g.Mean, GcsimSE: g.StdErr, Gse: r.Mean, GseSE: r.StdErr}
	switch {
	case g.Mean == 0 && r.Mean == 0:
		m.Verdict = "pass"
		return m
	case g.Mean == 0:
		m.Diff, m.Verdict = math.Inf(1), "fail"
		return m
	}
	m.Diff = (r.Mean - g.Mean) / g.Mean
	m.DiffSE = math.Hypot(g.StdErr, r.StdErr) / math.Abs(g.Mean)
	switch d := math.Abs(m.Diff); {
	case d+z*m.DiffSE <= bound:
		m.Verdict = "pass"
	case d-z*m.DiffSE > bound:
		m.Verdict = "fail"
	default:
		m.Verdict = "inconclusive"
	}
	return m
}

// needed is the iterations at which every mean's standard error reaches
// the target.
func needed(outs ...*harness.Output) int {
	n := 0
	for _, o := range outs {
		stats := []harness.Stat{o.Team}
		for _, s := range o.Chars {
			stats = append(stats, s)
		}
		for _, s := range stats {
			if s.Mean == 0 {
				continue
			}
			rel := s.StdErr / math.Abs(s.Mean)
			n = max(n, int(math.Ceil(float64(o.Iterations)*(rel/(*targetSE))*(rel/(*targetSE)))))
		}
	}
	return n
}

func compare(gse, tmp, id string, v variant, cfg string) Result {
	res := Result{Team: id, Variant: v.Name, Desc: v.Desc}
	path := writeConfig(tmp, id+"-"+v.Name, cfg)
	var g, r *harness.Output
	n := 0
	run := func(more int) error {
		start := time.Now()
		gb, err := harness.Run(cfg, more, *warmup, int64(1+n), *workers, true)
		if err != nil {
			return fmt.Errorf("gcsim: %w", err)
		}
		res.GcsimSeconds += time.Since(start).Seconds()
		start = time.Now()
		rb, err := runGse(gse, path, more, int64(1+n))
		if err != nil {
			return err
		}
		res.GseSeconds += time.Since(start).Seconds()
		g, r = merge(g, gb), merge(r, rb)
		n += more
		return nil
	}
	var err error
	if err = run(*iter0); err == nil {
		if need := min(needed(g, r), *maxIter); need > n {
			err = run(need - n)
		}
	}
	for err == nil {
		res.Metrics = metrics(g, r)
		res.Verdict = overall(res.Metrics)
		if res.Verdict != "inconclusive" || n >= *maxIter {
			break
		}
		err = run(min(4*n, *maxIter) - n)
	}
	res.Iter = n
	if err != nil {
		res.Verdict, res.Error = "error", err.Error()
		if errors.Is(err, errUnsupported) {
			res.Verdict, res.Error = "unsupported", "unsupported: "+strings.TrimPrefix(err.Error(), "unsupported: ")
		}
		return res
	}
	for _, c := range charNames(g) {
		res.Waits = append(res.Waits, Wait{Char: c, Gcsim: g.Waits[c], Gse: r.Waits[c]})
		res.EnergyWaits += g.Waits[c]["insufficient_energy"]
	}
	keys := map[string]bool{}
	for k := range g.Abils {
		keys[k] = true
	}
	for k := range r.Abils {
		keys[k] = true
	}
	for _, k := range slices.Sorted(mapsKeys(keys)) {
		res.Abils = append(res.Abils, Abil{Name: k, GcsimHits: g.Abils[k][0], GseHits: r.Abils[k][0], GcsimDmg: g.Abils[k][1], GseDmg: r.Abils[k][1]})
	}
	return res
}

func mapsKeys(m map[string]bool) func(func(string) bool) {
	return func(yield func(string) bool) {
		for k := range m {
			if !yield(k) {
				return
			}
		}
	}
}

func charNames(o *harness.Output) []string {
	var cs []string
	for c := range o.Chars {
		cs = append(cs, c)
	}
	sort.Strings(cs)
	return cs
}

func metrics(g, r *harness.Output) []Metric {
	ms := []Metric{metric("team", g.Team, r.Team)}
	for _, c := range charNames(g) {
		rc, ok := r.Chars[c]
		if !ok {
			rc = harness.Stat{}
		}
		ms = append(ms, metric(c, g.Chars[c], rc))
	}
	return ms
}

func overall(ms []Metric) string {
	v := "pass"
	for _, m := range ms {
		switch m.Verdict {
		case "fail":
			return "fail"
		case "inconclusive":
			v = "inconclusive"
		}
	}
	return v
}

// burstReady is a rotation's check that a burst is ready.
var burstReady = regexp.MustCompile(`\.\w+\.burst\.ready\b`)

var statsLine = regexp.MustCompile(`^(\s*)(\S+)(\s+add\s+stats\s+)(.*?);(.*)$`)

// apply moves the build's stats: each op scales its stat in every
// `add stats` line, then adds to the character's last one.
func (v variant) apply(build string) string {
	lines := strings.Split(build, "\n")
	last := map[string]int{}
	for i, l := range lines {
		if m := statsLine.FindStringSubmatch(l); m != nil {
			last[m[2]] = i
		}
	}
	for i, l := range lines {
		m := statsLine.FindStringSubmatch(l)
		if m == nil {
			continue
		}
		var keys []string
		vals := map[string]float64{}
		for _, kv := range strings.Fields(m[4]) {
			k, s, ok := strings.Cut(kv, "=")
			x, err := strconv.ParseFloat(s, 64)
			if !ok || err != nil {
				fail(fmt.Errorf("build line %q: cannot read %q", l, kv))
			}
			if _, seen := vals[k]; !seen {
				keys = append(keys, k)
			}
			vals[k] += x
		}
		for _, o := range v.ops {
			if x, ok := vals[o.stat]; ok {
				vals[o.stat] = x * o.scale
			}
			if last[m[2]] == i && o.add != 0 {
				if _, ok := vals[o.stat]; !ok {
					keys = append(keys, o.stat)
				}
				vals[o.stat] = max(0, vals[o.stat]+o.add)
			}
		}
		var kvs []string
		for _, k := range keys {
			kvs = append(kvs, k+"="+strconv.FormatFloat(vals[k], 'g', -1, 64))
		}
		lines[i] = m[1] + m[2] + m[3] + strings.Join(kvs, " ") + ";" + m[5]
	}
	return strings.Join(lines, "\n")
}

func census(sts []TeamStatus) Census {
	c := Census{Teams: len(sts), Status: sts, ByKind: map[string]int{}}
	count := map[string]*Blocker{}
	for _, s := range sts {
		switch s.Status {
		case "supported":
			c.Supported++
		case "error":
			c.Errors++
		case "unsupported":
			c.Unsupported++
			kinds := map[string]bool{}
			for _, u := range s.Unsupported {
				name := strings.TrimPrefix(u, "unsupported: ")
				b := count[name]
				if b == nil {
					b = &Blocker{Name: name}
					count[name] = b
				}
				b.Teams++
				if len(s.Unsupported) == 1 {
					b.Only++
				}
				kind, _, _ := strings.Cut(name, " ")
				kinds[kind] = true
			}
			for k := range kinds {
				c.ByKind[k]++
			}
		}
	}
	for _, b := range count {
		c.Blockers = append(c.Blockers, *b)
	}
	sort.Slice(c.Blockers, func(i, j int) bool {
		a, b := c.Blockers[i], c.Blockers[j]
		if a.Teams != b.Teams {
			return a.Teams > b.Teams
		}
		return a.Name < b.Name
	})
	return c
}

func pct(x float64) string {
	if math.IsInf(x, 0) {
		return "∞"
	}
	return fmt.Sprintf("%+.2f%%", 100*x)
}

func waitStr(w map[string]float64) string {
	var keys []string
	total := 0.0
	for k, v := range w {
		keys = append(keys, k)
		total += v
	}
	if len(keys) == 0 {
		return "0"
	}
	sort.Strings(keys)
	var parts []string
	for _, k := range keys {
		parts = append(parts, fmt.Sprintf("%s %.2f", k, w[k]))
	}
	return fmt.Sprintf("%.2f (%s)", total, strings.Join(parts, ", "))
}

func markdown(rep *Report) string {
	var b strings.Builder
	w := func(f string, a ...any) { fmt.Fprintf(&b, f, a...) }
	if len(rep.Results) > 0 {
		w("## Accuracy\n\n")
		w("Bound ±%.0f%% of gcsim; a verdict is pass when the 95%% interval of the difference lies inside it, fail when it lies outside, inconclusive otherwise. DPS over the window from %.0f s.\n\n", 100*bound, rep.Warmup)
		w("| Team | Build | Iterations | Verdict | Who | gcsim | gse | Difference | ±95%% |\n|---|---|---|---|---|---|---|---|---|\n")
		for _, r := range rep.Results {
			if len(r.Metrics) == 0 {
				w("| %s | %s | %d | **%s** | | | | %s | |\n", r.Team, r.Variant, r.Iter, r.Verdict, r.Error)
				continue
			}
			for i, m := range r.Metrics {
				team, build, it, verdict := "", "", "", ""
				if i == 0 {
					team, build, it, verdict = r.Team, r.Variant, strconv.Itoa(r.Iter), "**"+r.Verdict+"**"
				}
				mv := ""
				if m.Verdict != "pass" {
					mv = " (" + m.Verdict + ")"
				}
				w("| %s | %s | %s | %s | %s%s | %.1f ± %.1f | %.1f ± %.1f | %s | %.2f%% |\n", team, build, it, verdict, m.Name, mv, m.Gcsim, m.GcsimSE, m.Gse, m.GseSE, pct(m.Diff), 100*z*m.DiffSE)
			}
		}
		w("\n## Waits\n\nSeconds per fight, in the window, that actions waited, by reason.\n\n")
		w("| Team | Build | Who | gcsim | gse |\n|---|---|---|---|---|\n")
		for _, r := range rep.Results {
			for _, wt := range r.Waits {
				if len(wt.Gcsim) == 0 && len(wt.Gse) == 0 {
					continue
				}
				w("| %s | %s | %s | %s | %s |\n", r.Team, r.Variant, wt.Char, waitStr(wt.Gcsim), waitStr(wt.Gse))
			}
		}
		teams := map[string]float64{}
		for _, r := range rep.Results {
			teams[r.Team] = max(teams[r.Team], r.EnergyWaits)
		}
		for _, t := range slices.Sorted(func(yield func(string) bool) {
			for k := range teams {
				if !yield(k) {
					return
				}
			}
		}) {
			if teams[t] == 0 {
				w("\n**%s: no build waited for energy under gcsim**; the comparison has no ER-starved case for it.\n", t)
			}
		}
		w("\n## Abilities\n\nHits and damage per fight in the window; rows whose damage differs by more than 1%% of the character's damage, or whose hits differ by more than 2%%.\n\n")
		w("| Team | Build | Ability | Hits gcsim | Hits gse | Damage gcsim | Damage gse | Difference |\n|---|---|---|---|---|---|---|---|\n")
		for _, r := range rep.Results {
			total := map[string]float64{}
			for _, a := range r.Abils {
				c, _, _ := strings.Cut(a.Name, "/")
				total[c] += a.GcsimDmg
			}
			for _, a := range r.Abils {
				c, _, _ := strings.Cut(a.Name, "/")
				dmgOff := total[c] > 0 && math.Abs(a.GseDmg-a.GcsimDmg) > 0.01*total[c]
				// attacks of no damage, such as Hu Tao's 0 dmg hit at Paramita's
				// start, are in the JSON only
				hitsOff := math.Abs(a.GseHits-a.GcsimHits) > 0.02*max(a.GcsimHits, a.GseHits) && max(a.GcsimHits, a.GseHits) >= 1 && max(a.GcsimDmg, a.GseDmg) > 0
				if !dmgOff && !hitsOff {
					continue
				}
				diff := ""
				if a.GcsimDmg > 0 {
					diff = pct((a.GseDmg - a.GcsimDmg) / a.GcsimDmg)
				}
				w("| %s | %s | %s | %.2f | %.2f | %.0f | %.0f | %s |\n", r.Team, r.Variant, a.Name, a.GcsimHits, a.GseHits, a.GcsimDmg, a.GseDmg, diff)
			}
		}
	}
	c := rep.Census
	w("\n## Unsupported\n\n%d teams: %d supported, %d unsupported, %d errors.\n\n", c.Teams, c.Supported, c.Unsupported, c.Errors)
	if len(c.ByKind) > 0 {
		w("Teams blocked by kind: ")
		var ks []string
		for k, v := range c.ByKind {
			ks = append(ks, fmt.Sprintf("%s %d", k, v))
		}
		sort.Strings(ks)
		w("%s.\n\n", strings.Join(ks, ", "))
	}
	if len(c.Blockers) > 0 {
		w("| Unsupported | Teams it blocks | Teams it alone blocks |\n|---|---|---|\n")
		for i, bl := range c.Blockers {
			if i == 40 {
				w("| … %d more | | |\n", len(c.Blockers)-40)
				break
			}
			w("| %s | %d | %d |\n", bl.Name, bl.Teams, bl.Only)
		}
	}
	var errs []TeamStatus
	for _, s := range c.Status {
		if s.Status == "error" {
			errs = append(errs, s)
		}
	}
	if len(errs) > 0 {
		w("\nErrors:\n\n")
		for _, s := range errs {
			w("- %s: %s\n", s.ID, s.Error)
		}
	}
	if len(rep.Results) > 0 {
		if rep.Pass {
			w("\n**All comparisons pass.**\n")
		} else {
			w("\n**Not every comparison passes.**\n")
		}
	}
	return b.String()
}
