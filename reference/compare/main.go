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
// inconclusive otherwise. A character below -char-share of gcsim's team
// DPS is reported but not judged: what it does reaches the team's DPS,
// and its own share is too small for Gscouter's score to show. The
// accuracy wanted is Gscouter's: the site scores 50 iterations, whose
// standard error is about 0.7%, so a build runs -iter iterations (4,000:
// about 0.1% on the team's difference), and a build that fails or is
// inconclusive there grows, by twice to four times a step, to -max-iter
// (32,000), since a team whose energy waits spiral is heavy-tailed. A
// verdict still inconclusive at -max-iter is decided by its estimate
// (-settle): pass inside the bound, fail outside it. A port's systematic
// error shows as a difference far above that noise; small ones that add up
// over a team's items show in its team DPS, and small ones the whole
// engine shares show in the report's drift, the mean difference over every
// build compared. -target-se, when set, also grows a build until each
// engine's standard error reaches it; -settle=false keeps an inconclusive
// verdict and grows it further on the seeds gcsim's cache holds, running
// only the engine.
//
// Teams the engine cannot run are counted as "unsupported: <what>", and the
// census of what blocks how many teams measures the port's progress.
//
// Usage:
//
//	compare -gscouter ../Gscouter [-teams id,id] [-census] [-gcsim-only] [-only library,variants,buildspace,probes] [-shard k/n] [-json out.json] [-md out.md]
//
// The variants take about an hour per team, so CI compares library builds
// and probes on every push (-only library,probes) and the variants on a
// schedule, a few parts of the teams a day (-only variants -shard k/n).
//
// The build-space variants (-only buildspace; buildspace.go) change one
// axis of one member of a supported team's library build: its
// constellation, its refinement (1 and 5) or its weapon (others of the
// class that the engine has), so that the builds players bring are
// compared, not only the library's. They are many (about 40 per team) and
// run on a schedule in parts, like the variants. A build the engine cannot
// run is not run: it is reported as unsupported, with what it lacks.
//
// A weapon, set or character no supported team carries would go
// unchecked, so the probes (probes/*.txt) put it on one: each names a
// library team and the lines that replace those of its build, and is
// compared like a variant of it. A probe's team need not be supported, only
// its probe build: weapons and sets the engine lacks can be replaced. A
// probe can also replace lines of the team's rotation (parseProbe), to
// reach actions no library rotation uses.
//
// gcsim's results depend only on the config and the seeds, and the cache
// keeps them, so -gcsim-only runs gcsim alone on the builds selected, for
// teams the engine does not support yet too: seeds from 1 in whole chunks,
// to -iter and then to the iterations at which gcsim's own standard errors
// reach -target-se, up to -max-iter, as compare would. It warms the cache
// while a port is written, so that the port's compare runs only the engine.
// With -seeds lo:hi it runs exactly iterations lo to hi of each build (with
// -builds, any kind; build-space builds need the engine to assemble them),
// so that CI jobs can split a heavy-tailed build's seeds between them and
// leave its compare only the engine to run.
package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"hash/fnv"
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
	gscouterDir   = flag.String("gscouter", "", "Gscouter checkout (its teams/ and cmd/gscouter)")
	gsePath       = flag.String("gse", "../engine/target/release/gse", "the engine's executable")
	teamsFlag     = flag.String("teams", "", "comma-separated team ids (default: the whole library)")
	buildsFlag    = flag.String("builds", "", "comma-separated team/build pairs as the report names them (team/library, team/er-0, team/cons:xingqiu=C0, team/probe:kuki-c6): compare only those, of any kind; implies -only library,variants,buildspace,probes and -teams")
	censusOnly    = flag.Bool("census", false, "only classify teams as supported or not; run nothing")
	noVariants    = flag.Bool("no-variants", false, "compare library builds and probes only (same as -only library,probes)")
	onlyFlag      = flag.String("only", "library,variants,probes", "comma-separated kinds of build to compare: library, variants, buildspace, probes, pages")
	bsWeapons     = flag.Int("buildspace-weapons", 2, "build space: other weapons of the class per member, chosen by a hash of team, member and weapon (0: every one the engine has)")
	dryRun        = flag.Bool("dry-run", false, "list the builds that would be compared, and compare none")
	reverse       = flag.Bool("reverse", false, "take the teams in reverse order (for a second host warming the cache beside one that runs forward)")
	shardFlag     = flag.String("shard", "", "k/n: compare only the k-th of n parts of the teams that have builds to compare (k from 0); the census still covers every team")
	iter0         = flag.Int("iter", 4000, "iterations of the first batch")
	maxIter       = flag.Int("max-iter", 32000, "most iterations gcsim runs to for a build that fails or is inconclusive at -iter; with -settle=false, a build still inconclusive there grows further on the seeds gcsim's cache already holds for its config (whole chunks from seed 1), running only the engine")
	targetSE      = flag.Float64("target-se", 0, "standard error, relative to the mean, each engine must reach (0: none)")
	settle        = flag.Bool("settle", true, "decide a verdict still inconclusive at -max-iter by its estimate: pass inside the bound, fail outside it")
	charShare     = flag.Float64("char-share", 0.10, "characters below this share of gcsim's team DPS are reported, not judged")
	sampleN       = flag.Int("sample", 0, "variants, build space and pages: run at most this many builds of each kind per team, chosen by a hash of team and build (0: every one); the page's one-pass configs are always run, and builds the engine cannot run are all reported")
	warmup        = flag.Float64("warmup", 150, "seconds of the looped fight excluded from the window")
	workers       = flag.Int("j", runtime.NumCPU(), "workers")
	jsonOut       = flag.String("json", "", "write the results as JSON to this file")
	mdOut         = flag.String("md", "", "also write the report to this file")
	probesDir     = flag.String("probes", "probes", "directory of probes: library builds with lines replaced, and rotations with hunks replaced")
	configsOut    = flag.String("configs", "", "also write the config of each build selected to this directory, as <team>-<build>.txt (with -dry-run, without comparing)")
	cacheDir      = flag.String("cache", harness.DefaultCacheDir(), "directory of stored gcsim results, by config and chunk of seeds; empty for none")
	deadline      = flag.Duration("deadline", 0, "after this long, start no more builds and grow none further; builds not started are reported as deferred (0: no limit)")
	gcsimOnly     = flag.Bool("gcsim-only", false, "run only gcsim, into the cache, on the selected builds, whether the engine supports their teams or not (build-space builds need the engine to assemble and check them)")
	supportedOnly = flag.Bool("supported-only", false, "with -gcsim-only: take only the teams the engine supports, as compare without -gcsim-only does, so that -shard k/n names the same part as a compare run (a host warming the cache ahead of CI's jobs)")
	seedsFlag     = flag.String("seeds", "", "lo:hi, with -gcsim-only: run gcsim on iterations lo to hi of each selected build (seeds lo+1 to hi), whole chunks, into the cache, and nothing else, so that several jobs can share a build's seeds; respects -deadline")

	// cacheKeys reads what the cache holds without running gcsim.
	cacheKeys *cacheView
	// seedLo, seedHi are -seeds; seedHi is 0 without it.
	seedLo, seedHi int

	gcsimCache *harness.Cache
)

// A variant moves stats of every character's build.
type variant struct {
	Name string `json:"name"`
	Desc string `json:"desc"`
	ops  []op
	// force uses every burst without its readiness check, so that it
	// waits for energy.
	force bool
	// change is the axis a build-space build changes.
	change *BuildChange
	// page marks a config a result page runs besides the library's
	// (pages.go); warmup, when set, is its own in place of -warmup.
	page   bool
	warmup *float64
}

// warm is the seconds excluded from the build's window.
func (v variant) warm() float64 {
	if v.warmup != nil {
		return *v.warmup
	}
	return *warmup
}

// op scales a stat in every `add stats` line, then adds to the last one.
type op struct {
	stat  string
	scale float64
	add   float64
}

var variants = []variant{
	{"er-0", "no ER from artifacts", []op{{"er", 0, 0}}, false, nil, false, nil},
	{"er-0-forced", "no ER from artifacts, bursts used without their readiness checks so that they wait for energy", []op{{"er", 0, 0}}, true, nil, false, nil},
	{"er-half", "half the ER from artifacts", []op{{"er", 0.5, 0}}, false, nil, false, nil},
	{"er-half-forced", "half the ER from artifacts, bursts used without their readiness checks", []op{{"er", 0.5, 0}}, true, nil, false, nil},
	{"er+", "ER +30%", []op{{"er", 1, 0.3}}, false, nil, false, nil},
	{"crit-", "CR −20%, CD −40%", []op{{"cr", 1, -0.2}, {"cd", 1, -0.4}}, false, nil, false, nil},
	{"crit+", "CR +20%, CD +40%", []op{{"cr", 1, 0.2}, {"cd", 1, 0.4}}, false, nil, false, nil},
	{"atk+", "ATK +40%", []op{{"atk%", 1, 0.4}}, false, nil, false, nil},
	{"hp+", "HP +40%", []op{{"hp%", 1, 0.4}}, false, nil, false, nil},
	{"em+", "EM +150", []op{{"em", 1, 150}}, false, nil, false, nil},
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
	// Minor is a character below -char-share of the team's DPS: its
	// verdict is reported, not counted.
	Minor bool `json:"minor,omitempty"`
	// Settled is a verdict decided by its estimate at -max-iter.
	Settled bool `json:"settled,omitempty"`
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
	Team    string `json:"team"`
	Variant string `json:"variant"`
	Desc    string `json:"desc"`
	Iter    int    `json:"iterations"`
	Verdict string `json:"verdict"`
	Error   string `json:"error,omitempty"`
	// Build is what a build-space build changes.
	Build   *BuildChange `json:"build,omitempty"`
	Metrics []Metric     `json:"metrics,omitempty"`
	Waits   []Wait       `json:"waits,omitempty"`
	Abils   []Abil       `json:"abils,omitempty"`
	// EnergyWaits is gcsim's seconds per fight waiting for energy.
	EnergyWaits  float64 `json:"energy_waits"`
	GcsimSeconds float64 `json:"gcsim_seconds"`
	GseSeconds   float64 `json:"gse_seconds"`
	// Reused is a verdict taken from an earlier run (-reuse): the build's
	// engine output on its first seeds was the same to the last bit.
	Reused bool `json:"reused,omitempty"`
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
	Date     string  `json:"date"`
	Gscouter string  `json:"gscouter_commit"`
	Bound    float64 `json:"bound"`
	TargetSE float64 `json:"target_se"`
	Iter     int     `json:"iter"`
	MaxIter  int     `json:"max_iter"`
	Settle   bool    `json:"settle"`
	Share    float64 `json:"char_share"`
	Sample   int     `json:"sample,omitempty"`
	Warmup   float64 `json:"warmup"`
	// Drift is the mean of the team's difference over the builds compared.
	Drift    *Drift    `json:"drift,omitempty"`
	Variants []variant `json:"variants"`
	Results  []Result  `json:"results"`
	Census   Census    `json:"census"`
	// Only is the kinds of build compared, Shard the part of the teams.
	Only  []string `json:"only"`
	Shard string   `json:"shard,omitempty"`
	// Deadline is -deadline, when one was given.
	Deadline string `json:"deadline,omitempty"`
	// Warmed is what -gcsim-only ran, in place of Results and Census.
	Warmed []Warmed `json:"warmed,omitempty"`
	// BuildSpace counts the build-space builds by axis, and what stops
	// those the engine cannot run.
	BuildSpace        *BuildSpace `json:"buildspace,omitempty"`
	BuildSpaceWeapons int         `json:"buildspace_weapons,omitempty"`
	Pass              bool        `json:"pass"`
}

// Warmed is a build -gcsim-only ran gcsim on.
type Warmed struct {
	Team    string `json:"team"`
	Variant string `json:"variant"`
	Iter    int    `json:"iterations"`
	// Chunks is the whole chunks of seeds run, Cached how many of them the
	// cache already had.
	Chunks       int     `json:"chunks"`
	Cached       int     `json:"cached"`
	GcsimSeconds float64 `json:"gcsim_seconds"`
	Error        string  `json:"error,omitempty"`
	// Seeds is -seeds, when given; Stopped says where -deadline stopped
	// it (not an error: the build's compare runs what is missing).
	Seeds   string `json:"seeds,omitempty"`
	Stopped string `json:"stopped,omitempty"`
}

type job struct {
	v   variant
	cfg string
	err error
}

// sample keeps at most n of the jobs that run, chosen by a hash of team
// and build so that a team's choice is stable and different teams draw
// different builds; a page's one-pass config is always kept, past n, and
// a job that cannot run (an error, or what the engine lacks) is always
// kept, so that it is reported. n <= 0 keeps every job.
func sample(id string, jobs []job, n int) []job {
	if n <= 0 {
		return jobs
	}
	h := func(name string) uint64 {
		f := fnv.New64a()
		f.Write([]byte(id + "/" + name))
		return f.Sum64()
	}
	var runs []int
	keep := make([]bool, len(jobs))
	for i, j := range jobs {
		switch {
		case j.err != nil, j.v.page && strings.HasSuffix(j.v.Name, ":once"):
			keep[i] = true
		default:
			runs = append(runs, i)
		}
	}
	sort.SliceStable(runs, func(a, b int) bool { return h(jobs[runs[a]].v.Name) < h(jobs[runs[b]].v.Name) })
	for _, i := range runs[:min(n, len(runs))] {
		keep[i] = true
	}
	var out []job
	for i, j := range jobs {
		if keep[i] {
			out = append(out, j)
		}
	}
	return out
}

// Drift is the mean of the team's difference to gcsim over the builds
// compared, with its standard error: a port's error that every build
// shares a little of, which no build's verdict shows.
type Drift struct {
	Builds int     `json:"builds"`
	Mean   float64 `json:"mean"`
	SE     float64 `json:"se"`
}

// driftOf is the drift over results that have a team difference.
func driftOf(rs []Result) *Drift {
	var xs []float64
	for _, r := range rs {
		if r.Verdict == "idle" || len(r.Metrics) == 0 || r.Metrics[0].Name != "team" || math.IsInf(r.Metrics[0].Diff, 0) || math.IsNaN(r.Metrics[0].Diff) {
			continue
		}
		xs = append(xs, r.Metrics[0].Diff)
	}
	if len(xs) == 0 {
		return nil
	}
	d := &Drift{Builds: len(xs)}
	for _, x := range xs {
		d.Mean += x
	}
	d.Mean /= float64(len(xs))
	if len(xs) > 1 {
		v := 0.0
		for _, x := range xs {
			v += (x - d.Mean) * (x - d.Mean)
		}
		d.SE = math.Sqrt(v / float64(len(xs)-1) / float64(len(xs)))
	}
	return d
}

// wanted is -builds: team, then build names. Empty for every build.
var wanted = map[string]map[string]bool{}

// found records the -builds that were met, to name those that were not.
var found = map[string]bool{}

// want is whether -builds selects build name of team id.
func want(id, name string) bool {
	if len(wanted) == 0 {
		return true
	}
	if wanted[id][name] {
		found[id+"/"+name] = true
		return true
	}
	return false
}

// started is when compare started, for -deadline.
var started = time.Now()

// pastDeadline is whether -deadline has passed: a CI job must write its
// report and save gcsim's chunks before the runner's time limit stops it.
func pastDeadline() bool {
	return *deadline > 0 && time.Since(started) > *deadline
}

func main() {
	flag.Parse()
	if *gscouterDir == "" {
		fail(errors.New("-gscouter is required"))
	}
	var err error
	if gcsimCache, err = harness.OpenCache(*cacheDir); err != nil {
		fail(err)
	}
	if cacheKeys, err = openCacheView(*cacheDir); err != nil {
		fail(err)
	}
	if *supportedOnly && !*gcsimOnly {
		fail(errors.New("-supported-only needs -gcsim-only"))
	}
	if *gcsimOnly && gcsimCache == nil {
		fail(errors.New("-gcsim-only needs a cache (-cache)"))
	}
	if *seedsFlag != "" {
		if !*gcsimOnly {
			fail(errors.New("-seeds needs -gcsim-only"))
		}
		if seedLo, seedHi, err = parseSeeds(*seedsFlag); err != nil {
			fail(err)
		}
	}
	if *bsWeapons < 0 {
		fail(errors.New("-buildspace-weapons: want 0 or more"))
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
	pages := ""
	if strings.Contains(*onlyFlag, "pages") || *buildsFlag != "" {
		if pages, err = buildPages(tmp); err != nil {
			fail(err)
		}
	}

	ids := listTeams()
	if *teamsFlag != "" {
		ids = strings.Split(*teamsFlag, ",")
	}
	kinds, err := parseOnly(*onlyFlag)
	if err != nil {
		fail(err)
	}
	if *buildsFlag != "" {
		if *teamsFlag != "" || *censusOnly {
			fail(fmt.Errorf("-builds takes no -teams or -census"))
		}
		ids = nil
		for _, tb := range strings.Split(*buildsFlag, ",") {
			team, b, ok := strings.Cut(tb, "/")
			if !ok || team == "" || b == "" {
				fail(fmt.Errorf("-builds: want team/build, got %q", tb))
			}
			if wanted[team] == nil {
				wanted[team] = map[string]bool{}
				ids = append(ids, team)
			}
			wanted[team][b] = true
		}
		kinds, err = parseOnly("library,variants,buildspace,probes,pages")
		if err != nil {
			fail(err)
		}
	}
	if *noVariants {
		delete(kinds, "variants")
	}
	shard, shards, err := parseShard(*shardFlag)
	if err != nil {
		fail(err)
	}
	rep := &Report{Date: time.Now().UTC().Format(time.RFC3339), Gscouter: gitHead(*gscouterDir), Bound: bound, TargetSE: *targetSE, Iter: *iter0, MaxIter: *maxIter, Settle: *settle, Share: *charShare, Sample: *sampleN, Warmup: *warmup, Only: slices.Sorted(mapsKeys(kinds)), Shard: *shardFlag}
	if *deadline > 0 {
		rep.Deadline = deadline.String()
	}
	if kinds["variants"] {
		rep.Variants = variants
	}
	if kinds["buildspace"] {
		rep.BuildSpaceWeapons = *bsWeapons
	}
	weapons := &weaponSupport{gse: gse, tmp: tmp, known: map[string]bool{}}

	// census: dump and check every team
	statuses := make([]TeamStatus, len(ids))
	configs := make([]string, len(ids))
	parallel(len(ids), *workers, func(i int) {
		id := ids[i]
		st := TeamStatus{ID: id}
		cfg, err := dump(gscouter, id, "")
		if err != nil {
			st.Status, st.Error = "error", err.Error()
		} else if *gcsimOnly && !*supportedOnly && !needsEngine(id, kinds) {
			// gcsim needs no support from the engine
			st.Status = "dumped"
			configs[i] = cfg
		} else if miss, err := check(gse, tmp, id, cfg); err != nil {
			st.Status, st.Error = "error", err.Error()
		} else if len(miss) > 0 {
			st.Status, st.Unsupported = "unsupported", miss
			if *gcsimOnly && !*supportedOnly {
				// its library build, variants and probes need no engine
				configs[i] = cfg
			}
		} else {
			st.Status = "supported"
			configs[i] = cfg
		}
		statuses[i] = st
	})

	if !*censusOnly {
		// the teams with builds to compare, in the order listed, split
		// into parts by position
		order := make([]int, len(ids))
		for i := range order {
			order[i] = i
		}
		if *reverse {
			// a second host starts from the end, so two hosts warming the
			// cache meet in the middle instead of running the same teams;
			// the parts are numbered in this order
			slices.Reverse(order)
		}
		part := 0
		for _, i := range order {
			id := ids[i]
			supported := statuses[i].Status == "supported"
			// -gcsim-only takes every team, and reports one whose dump failed
			// (-supported-only: the teams a compare run takes, for its parts)
			selected := supported || (*gcsimOnly && !*supportedOnly)
			var probes []probe
			if kinds["probes"] {
				probes = probesFor(id)
			}
			if !(selected && (kinds["library"] || kinds["variants"] || kinds["buildspace"] || kinds["pages"])) && len(probes) == 0 {
				continue
			}
			part++
			if (part-1)%shards != shard {
				continue
			}
			if *gcsimOnly && statuses[i].Status == "error" {
				rep.Warmed = append(rep.Warmed, Warmed{Team: id, Variant: "library", Error: statuses[i].Error})
				continue
			}
			// dumpErr fails, or under -gcsim-only reports the build
			var jobs []job
			dumpErr := func(v variant, err error) {
				err = fmt.Errorf("%s %s: %w", id, v.Name, err)
				if !*gcsimOnly {
					fail(err)
				}
				jobs = append(jobs, job{v: v, err: err})
			}
			if selected && kinds["library"] && want(id, "library") {
				jobs = append(jobs, job{v: variant{Name: "library", Desc: "library build"}, cfg: configs[i]})
			}
			if selected && kinds["variants"] {
				base, err := os.ReadFile(filepath.Join(*gscouterDir, "teams", id, "build.txt"))
				if err != nil {
					fail(err)
				}
				start := len(jobs)
				for _, v := range variants {
					if !want(id, v.Name) {
						continue
					}
					path := filepath.Join(tmp, id+"-"+v.Name+".txt")
					if err := os.WriteFile(path, []byte(v.apply(string(base))), 0o644); err != nil {
						fail(err)
					}
					cfg, err := dump(gscouter, id, path)
					if err != nil {
						dumpErr(v, err)
						continue
					}
					if v.force {
						forced := burstReady.ReplaceAllString(cfg, "1")
						if forced == cfg {
							// every burst already waits for energy: er-0 covers it
							continue
						}
						cfg = forced
					}
					jobs = append(jobs, job{v: v, cfg: cfg})
				}
				jobs = append(jobs[:start:start], sample(id, jobs[start:], *sampleN)...)
			}
			if supported && kinds["buildspace"] {
				base, err := os.ReadFile(filepath.Join(*gscouterDir, "teams", id, "build.txt"))
				if err != nil {
					fail(err)
				}
				ms, err := members(string(base))
				if err != nil {
					fail(fmt.Errorf("%s: %w", id, err))
				}
				// whether the engine has a weapon does not depend on whose
				// line it replaces
				builds, err := buildSpace(id, string(base), *bsWeapons, func(w string) (bool, error) {
					return weapons.implemented(configs[i], strings.TrimSpace(ms[0].weapLine), ms[0].name, w)
				})
				if err != nil {
					fail(err)
				}
				start := len(jobs)
				for _, bb := range builds {
					if !want(id, bb.v.Name) {
						continue
					}
					path := filepath.Join(tmp, id+"-"+bb.v.Name+".txt")
					if err := os.WriteFile(path, []byte(bb.build), 0o644); err != nil {
						fail(err)
					}
					cfg, err := dump(gscouter, id, path)
					if err != nil {
						jobs = append(jobs, job{v: bb.v, err: fmt.Errorf("%s %s: %w", id, bb.v.Name, err)})
						continue
					}
					// what the engine lacks is reported, not run
					miss, err := check(gse, tmp, id+"-"+bb.v.Name, cfg)
					if err != nil {
						jobs = append(jobs, job{v: bb.v, err: fmt.Errorf("%s %s: %w", id, bb.v.Name, err)})
						continue
					}
					if len(miss) > 0 {
						var what []string
						for _, u := range miss {
							what = append(what, strings.TrimPrefix(u, "unsupported: "))
						}
						jobs = append(jobs, job{v: bb.v, err: fmt.Errorf("%w: %s", errUnsupported, strings.Join(what, "; "))})
						continue
					}
					jobs = append(jobs, job{v: bb.v, cfg: cfg})
				}
				jobs = append(jobs[:start:start], sample(id, jobs[start:], *sampleN)...)
			}
			if supported && kinds["pages"] {
				jobs = append(jobs, sample(id, pageJobs(pages, gse, tmp, id, configs[i]), *sampleN)...)
			}
			for _, pr := range probes {
				if !want(id, pr.v.Name) {
					continue
				}
				base, err := os.ReadFile(filepath.Join(*gscouterDir, "teams", id, "build.txt"))
				if err != nil {
					fail(err)
				}
				path := filepath.Join(tmp, id+"-"+pr.v.Name+".txt")
				if err := os.WriteFile(path, []byte(pr.apply(string(base))), 0o644); err != nil {
					fail(err)
				}
				dir, err := pr.teamDir(tmp, id)
				if err != nil {
					fail(err)
				}
				cfg, err := dumpDir(gscouter, dir, path)
				if err != nil {
					dumpErr(pr.v, err)
					continue
				}
				jobs = append(jobs, job{v: pr.v, cfg: cfg})
			}
			for _, j := range jobs {
				if *configsOut != "" && j.err == nil {
					if err := os.MkdirAll(*configsOut, 0o755); err != nil {
						fail(err)
					}
					name := id + "-" + strings.NewReplacer(":", "-", "=", "-", "/", "-").Replace(j.v.Name) + ".txt"
					if err := os.WriteFile(filepath.Join(*configsOut, name), []byte(j.cfg), 0o644); err != nil {
						fail(err)
					}
				}
				if *dryRun {
					if j.err != nil {
						fmt.Fprintf(os.Stderr, "%s / %s (%v)\n", id, j.v.Name, j.err)
					} else {
						fmt.Fprintf(os.Stderr, "%s / %s\n", id, j.v.Name)
					}
					// the build space's counts by axis, and what the engine
					// lacks, cost no run
					if j.v.change != nil {
						r := Result{Team: id, Variant: j.v.Name, Desc: j.v.Desc, Build: j.v.change, Verdict: "dry-run"}
						if j.err != nil {
							r.Verdict, r.Error = "error", j.err.Error()
							if errors.Is(j.err, errUnsupported) {
								r.Verdict, r.Error = "unsupported", "unsupported: "+strings.TrimPrefix(j.err.Error(), "unsupported: ")
							}
						}
						rep.Results = append(rep.Results, r)
					}
					continue
				}
				if *gcsimOnly {
					w := Warmed{Team: id, Variant: j.v.Name}
					if j.err != nil {
						w.Error = j.err.Error()
					} else {
						fmt.Fprintf(os.Stderr, "%s / %s ...\n", id, j.v.Name)
						if seedHi > 0 {
							w = warmSeeds(id, j.v, j.cfg)
						} else {
							w = warm(id, j.v, j.cfg)
						}
					}
					fmt.Fprintf(os.Stderr, "%s / %s: %d iterations, %d of %d chunks cached, gcsim %.0f s %s%s\n", id, j.v.Name, w.Iter, w.Cached, w.Chunks, w.GcsimSeconds, w.Error, w.Stopped)
					rep.Warmed = append(rep.Warmed, w)
					continue
				}
				if j.err != nil {
					// a build-space build that could not be assembled, or
					// that the engine cannot run
					r := Result{Team: id, Variant: j.v.Name, Desc: j.v.Desc, Build: j.v.change, Verdict: "error", Error: j.err.Error()}
					if errors.Is(j.err, errUnsupported) {
						r.Verdict, r.Error = "unsupported", "unsupported: "+strings.TrimPrefix(j.err.Error(), "unsupported: ")
					}
					fmt.Fprintf(os.Stderr, "%s / %s: %s\n", id, j.v.Name, r.Error)
					rep.Results = append(rep.Results, r)
					continue
				}
				if pastDeadline() {
					// not run, and reported so: the next run takes it, with
					// gcsim's chunks so far in the cache
					r := Result{Team: id, Variant: j.v.Name, Desc: j.v.Desc, Build: j.v.change, Verdict: "deferred", Error: fmt.Sprintf("not started before -deadline %s", *deadline)}
					fmt.Fprintf(os.Stderr, "%s / %s: deferred\n", id, j.v.Name)
					rep.Results = append(rep.Results, r)
					continue
				}
				fmt.Fprintf(os.Stderr, "%s / %s ...\n", id, j.v.Name)
				r := compareReusing(gse, tmp, id, j.v, j.cfg)
				reused := ""
				if r.Reused {
					reused = " (reused)"
				}
				fmt.Fprintf(os.Stderr, "%s / %s: %s at %d iterations%s\n", id, j.v.Name, r.Verdict, r.Iter, reused)
				// a probe exists to check what it adds, so it must run
				if r.Verdict == "unsupported" && strings.HasPrefix(j.v.Name, "probe:") {
					r.Verdict = "error"
				}
				// a feature only the run finds makes the team unsupported,
				// unless a build-space build alone carries it
				if r.Verdict == "unsupported" && supported && j.v.change == nil && !j.v.page {
					statuses[i].Status = "unsupported"
					statuses[i].Unsupported = []string{r.Error}
				}
				rep.Results = append(rep.Results, r)
			}
		}
	}
	// a build -builds names that no team had is an error, never skipped
	if len(wanted) > 0 && !*censusOnly {
		var missing []string
		for team, bs := range wanted {
			for b := range bs {
				if !found[team+"/"+b] {
					missing = append(missing, team+"/"+b)
				}
			}
		}
		if len(missing) > 0 {
			sort.Strings(missing)
			fail(fmt.Errorf("-builds: no such build (or its team is not supported): %s", strings.Join(missing, ", ")))
		}
	}
	rep.Pass = true
	if !*gcsimOnly {
		rep.Census = census(statuses)
	}
	rep.BuildSpace = buildSpaceSummary(rep.Results)
	rep.Drift = driftOf(rep.Results)
	for _, r := range rep.Results {
		if r.Verdict != "pass" && r.Verdict != "both-error" && r.Verdict != "idle" && r.Verdict != "unsupported" && r.Verdict != "dry-run" && r.Verdict != "deferred" {
			rep.Pass = false
		}
	}
	for _, w := range rep.Warmed {
		if w.Error != "" {
			rep.Pass = false
		}
	}

	md := markdown(rep)
	if *gcsimOnly {
		md = warmMarkdown(rep)
	}
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

// parseOnly reads -only: library, variants, probes.
func parseOnly(s string) (map[string]bool, error) {
	kinds := map[string]bool{}
	for _, k := range strings.Split(s, ",") {
		k = strings.TrimSpace(k)
		switch k {
		case "library", "variants", "buildspace", "probes", "pages":
			kinds[k] = true
		case "":
		default:
			return nil, fmt.Errorf("-only: unknown kind %q (library, variants, buildspace, probes, pages)", k)
		}
	}
	if len(kinds) == 0 {
		return nil, errors.New("-only: no kind given")
	}
	return kinds, nil
}

// parseShard reads -shard k/n; empty is the whole, 0/1.
func parseShard(s string) (int, int, error) {
	if s == "" {
		return 0, 1, nil
	}
	a, b, ok := strings.Cut(s, "/")
	k, err1 := strconv.Atoi(a)
	n, err2 := strconv.Atoi(b)
	if !ok || err1 != nil || err2 != nil || n < 1 || k < 0 || k >= n {
		return 0, 0, fmt.Errorf("-shard: want k/n with 0 <= k < n, got %q", s)
	}
	return k, n, nil
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
	return dumpDir(gscouter, filepath.Join("teams", id), build)
}

// dumpDir is dump with the team directory given, relative to Gscouter's
// directory or absolute.
func dumpDir(gscouter, dir, build string) (string, error) {
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

// bothError is a build that gcsim and the engine both refuse to run: the
// rotation errors under both (Yae Miko's plunge without Xianyun's burst up,
// when a variant starves the burst of energy). It is agreement, not a
// difference, and its verdict is "both-error".
type bothError struct{ gcsim, gse error }

func (e *bothError) Error() string {
	first := func(err error) string { m, _, _ := strings.Cut(err.Error(), "\n"); return m }
	return fmt.Sprintf("both refuse: gcsim: %s; %s", first(e.gcsim), first(e.gse))
}

// rareError runs the engine on seeds 1 to -max-iter of a build on which
// gcsim errored in the chunk from seed `from` (n seeds) and the engine did
// not: a rotation can error in a small share of fights only (a condition
// read the frame before the state it tests ends), and each engine draws
// its own fights. If the engine errors with the same message in any of
// them, the build is a both-error, with both messages and their seeds;
// otherwise nil, and the build stays an error.
func rareError(gse, path string, warm float64, gerr error, from, n int) *bothError {
	_, rerr := runGse(gse, path, *maxIter, 1, warm)
	if rerr == nil || errors.Is(rerr, errUnsupported) || !sameError(gerr.Error(), rerr.Error()) {
		return nil
	}
	seed := "?"
	if m := gseIteration.FindStringSubmatch(rerr.Error()); m != nil {
		i, _ := strconv.Atoi(m[1])
		seed = strconv.Itoa(1 + i)
	}
	return &bothError{
		gcsim: fmt.Errorf("%v (in seeds %d to %d)", gerr, from, from+n-1),
		gse:   fmt.Errorf("%v (seed %s, of seeds 1 to %d)", rerr, seed, *maxIter),
	}
}

// gseIteration finds the fight an engine error happened in (gse's
// "iteration i: ...", seed 1 + i from seed 1).
var gseIteration = regexp.MustCompile(`iteration (\d+): `)

// sameError compares a gcsim error with an engine error by their last
// clause, the message itself: gcsim prefixes it with "error encountered on
// <char> executing <action>: ", the engine with "iteration i: <char>: ".
func sameError(gcsimErr, gseErr string) bool {
	last := func(s string) string {
		s, _, _ = strings.Cut(s, "\n")
		if i := strings.LastIndex(s, ": "); i >= 0 {
			s = s[i+2:]
		}
		return strings.TrimSpace(s)
	}
	g, r := last(gcsimErr), last(gseErr)
	return g != "" && g == r
}

func runGse(gse, path string, n int, seed int64, warmup float64) (*harness.Output, error) {
	var stderr bytes.Buffer
	cmd := exec.Command(gse, path, strconv.Itoa(n), fmt.Sprint(warmup), strconv.Itoa(*workers), "--seed", strconv.FormatInt(seed, 10))
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

// idleDPS is team DPS below which a build does nothing worth scoring:
// real teams deal thousands. A build with both engines' team DPS below it
// is "idle" (counted as passing, listed in its own section of the report,
// and kept unverified by the manifest): its rotation does nothing in the
// scored window in either engine (Skirk-led library rotations that never
// clear First_Rot_Nuke wait the whole window for a burst), and comparing
// noise around zero fails at random.
const idleDPS = 10

func idle(g, r *harness.Output) bool {
	return g.Team.Mean < idleDPS && r.Team.Mean < idleDPS
}

// zeroDPS is DPS too small to be anything but floating-point residue:
// gcsim can leave 1e-12 for a character whose every hit in the window does
// no damage, where the engine sums an exact 0, and the relative difference
// of the two is -100%.
const zeroDPS = 1e-6

func metric(name string, g, r harness.Stat) Metric {
	m := Metric{Name: name, Gcsim: g.Mean, GcsimSE: g.StdErr, Gse: r.Mean, GseSE: r.StdErr}
	switch {
	case g.Mean == 0 && r.Mean == 0, math.Abs(g.Mean) < zeroDPS && math.Abs(r.Mean) < zeroDPS:
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
	if *targetSE <= 0 {
		return 0
	}
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

// growth is the factor to grow the iterations by while a verdict is
// inconclusive: from each inconclusive metric's distance to the bound, the
// iterations at which its interval would just clear it, with a quarter more
// for the noise in that estimate. It is at least 2, so that the looks at the
// data stay as few as with a fixed factor, and at most 4, the factor used
// before, so that no build runs longer than it did.
func growth(ms []Metric) float64 {
	g := 2.0
	for _, m := range ms {
		if m.Verdict != "inconclusive" || m.Minor {
			continue
		}
		gap := math.Abs(math.Abs(m.Diff) - bound)
		if gap == 0 {
			return 4
		}
		g = max(g, 1.25*(z*m.DiffSE/gap)*(z*m.DiffSE/gap))
	}
	return min(g, 4)
}

func compare(gse, tmp, id string, v variant, cfg string) Result {
	res := Result{Team: id, Variant: v.Name, Desc: v.Desc, Build: v.change}
	path := writeConfig(tmp, id+"-"+v.Name, cfg)
	var g, r *harness.Output
	n := 0
	// limit is -max-iter, or past it the seeds the cache holds
	limit := *maxIter
	run := func(more int) error {
		if gcsimCache != nil {
			// whole chunks, which the cache can keep
			more = (more + harness.Chunk - 1) / harness.Chunk * harness.Chunk
			if n+more > limit {
				more = max(limit-n, 0)
			}
		}
		start := time.Now()
		gb, err := gcsimCache.RunCached(cfg, more, v.warm(), int64(1+n), *workers, true)
		if err != nil {
			// a build gcsim cannot run (a rotation that errors under it)
			// agrees when the engine refuses it too
			if _, gerr := runGse(gse, path, more, int64(1+n), v.warm()); gerr != nil && !errors.Is(gerr, errUnsupported) {
				return &bothError{gcsim: err, gse: gerr}
			}
			// a rotation that errors in a few fights only (a race at
			// one frame) agrees when the engine, run on seeds up to
			// -max-iter, hits the same error in a fight of its own
			if both := rareError(gse, path, v.warm(), err, 1+n, more); both != nil {
				return both
			}
			return fmt.Errorf("gcsim: %w", err)
		}
		res.GcsimSeconds += time.Since(start).Seconds()
		start = time.Now()
		rb, err := runGse(gse, path, more, int64(1+n), v.warm())
		if err != nil {
			return err
		}
		res.GseSeconds += time.Since(start).Seconds()
		g, r = harness.Merge(g, gb), harness.Merge(r, rb)
		n += more
		return nil
	}
	var err error
	if err = run(*iter0); err == nil {
		if need := min(needed(g, r), *maxIter); need > n {
			err = run(need - n)
		}
	}
	extended := false
	for err == nil {
		res.Metrics = metrics(g, r)
		res.Verdict = overall(res.Metrics)
		// a rotation that does nothing in the scored window in either
		// engine has only noise around zero to compare
		if idle(g, r) {
			res.Verdict = "idle"
			break
		}
		if res.Verdict == "pass" {
			break
		}
		// a failure at -iter grows once to -max-iter: a team whose
		// energy waits spiral is heavy-tailed, and its first iterations
		// can be far off
		if res.Verdict == "fail" {
			if n >= limit || pastDeadline() {
				break
			}
			err = run(limit - n)
			continue
		}
		// past -deadline an inconclusive build stays inconclusive
		if pastDeadline() {
			break
		}
		if n >= limit {
			if *settle {
				res.Metrics = settled(res.Metrics)
				res.Verdict = overall(res.Metrics)
				break
			}
			if extended {
				break
			}
			// inconclusive at -max-iter: grow on the seeds gcsim's cache
			// already holds, if any, running only the engine
			extended = true
			c, cerr := cachedLimit(cfg, n, v.warm())
			if cerr != nil {
				err = cerr
				break
			}
			if c <= n {
				break
			}
			limit = c
			fmt.Fprintf(os.Stderr, "%s / %s: inconclusive at -max-iter %d; the cache holds gcsim's first %d seeds, growing on them with the engine alone\n", id, v.Name, n, c)
		}
		err = run(min(int(float64(n)*growth(res.Metrics)), limit) - n)
	}
	res.Iter = n
	if err != nil {
		res.Verdict, res.Error = "error", err.Error()
		var both *bothError
		if errors.As(err, &both) {
			res.Verdict = "both-error"
		}
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

// cachedLimit is how far past n iterations a build may grow on gcsim's
// cache alone: the iterations of the whole chunks from seed 1 that the
// cache holds for its config, when compare has run its first n seeds
// through the cache (n whole chunks); otherwise n, no further.
func cachedLimit(cfg string, n int, warmup float64) (int, error) {
	if gcsimCache == nil || cacheKeys == nil || n%harness.Chunk != 0 || n == 0 {
		return n, nil
	}
	// the chunks just run through the cache must be where cacheView looks:
	// if not, cachekeys.go no longer derives harness.Cache's paths
	last := n/harness.Chunk - 1
	if !cacheKeys.has(cfg, warmup, true, 0) || !cacheKeys.has(cfg, warmup, true, last) {
		return 0, errors.New("cache: chunks just run are not where cachekeys.go looks; it no longer derives harness.Cache's paths")
	}
	return cacheKeys.cachedFrom(cfg, warmup, true, last+1) * harness.Chunk, nil
}

// warmSeeds is -gcsim-only -seeds lo:hi: gcsim on the build's iterations
// lo to hi (seeds lo+1 to hi), chunk by chunk through the cache, until
// -deadline. Jobs given disjoint ranges of one build compute its chunks
// together; each chunk is the one compare would run or read.
func warmSeeds(id string, v variant, cfg string) Warmed {
	res := Warmed{Team: id, Variant: v.Name, Seeds: fmt.Sprintf("%d:%d", seedLo, seedHi)}
	for s := seedLo; s < seedHi; s += harness.Chunk {
		if pastDeadline() {
			res.Stopped = fmt.Sprintf("stopped at iteration %d of %d:%d by -deadline %s", s, seedLo, seedHi, *deadline)
			break
		}
		chunk := s / harness.Chunk
		if cacheKeys.has(cfg, v.warm(), true, chunk) {
			res.Cached++
		}
		start := time.Now()
		if _, err := gcsimCache.RunCached(cfg, harness.Chunk, v.warm(), int64(1+s), *workers, true); err != nil {
			res.Error = fmt.Sprintf("gcsim: %v", err)
			break
		}
		res.GcsimSeconds += time.Since(start).Seconds()
		if !cacheKeys.has(cfg, v.warm(), true, chunk) {
			res.Error = "cache: the chunk just run is not where cachekeys.go looks; it no longer derives harness.Cache's paths"
			break
		}
		res.Chunks++
		res.Iter += harness.Chunk
	}
	return res
}

// parseSeeds reads -seeds lo:hi, whole chunks with 0 <= lo < hi.
func parseSeeds(s string) (int, int, error) {
	a, b, ok := strings.Cut(s, ":")
	lo, err1 := strconv.Atoi(a)
	hi, err2 := strconv.Atoi(b)
	if !ok || err1 != nil || err2 != nil || lo < 0 || hi <= lo || lo%harness.Chunk != 0 || hi%harness.Chunk != 0 {
		return 0, 0, fmt.Errorf("-seeds: want lo:hi, multiples of %d with 0 <= lo < hi, got %q", harness.Chunk, s)
	}
	return lo, hi, nil
}

// needsEngine is whether -gcsim-only needs gse to select the team's
// builds: build-space builds are assembled from the weapons the engine
// has, for supported teams, and checked by it.
func needsEngine(id string, kinds map[string]bool) bool {
	if !kinds["buildspace"] {
		return false
	}
	if len(wanted) == 0 {
		return true
	}
	for b := range wanted[id] {
		if b != "library" && !strings.HasPrefix(b, "probe:") && !slices.ContainsFunc(variants, func(v variant) bool { return v.Name == b }) {
			return true
		}
	}
	return false
}

// warm runs gcsim alone on a build as compare runs it: through the cache,
// seeds from 1, whole chunks, to -iter and then to what gcsim's own
// standard errors need, up to -max-iter. The engine's errors may need
// more; compare runs those, and the inconclusive steps, itself.
func warm(id string, v variant, cfg string) Warmed {
	res := Warmed{Team: id, Variant: v.Name}
	var g *harness.Output
	n := 0
	run := func(more int) error {
		// as compare's run
		more = (more + harness.Chunk - 1) / harness.Chunk * harness.Chunk
		if n+more > *maxIter {
			more = max(*maxIter-n, 0)
		}
		before, err := countChunks(*cacheDir)
		if err != nil {
			return err
		}
		start := time.Now()
		gb, err := gcsimCache.RunCached(cfg, more, v.warm(), int64(1+n), *workers, true)
		if err != nil {
			return fmt.Errorf("gcsim: %w", err)
		}
		res.GcsimSeconds += time.Since(start).Seconds()
		after, err := countChunks(*cacheDir)
		if err != nil {
			return err
		}
		// n is whole chunks until -max-iter cuts a batch, which is the last
		chunks := more / harness.Chunk
		res.Chunks += chunks
		res.Cached += chunks - (after - before)
		g = harness.Merge(g, gb)
		n += more
		return nil
	}
	err := run(*iter0)
	if err == nil {
		if need := min(needed(g), *maxIter); need > n {
			err = run(need - n)
		}
	}
	res.Iter = n
	if err != nil {
		res.Error = err.Error()
	}
	return res
}

// countChunks is the number of chunks stored in the cache. The builds run
// one at a time, so what a run adds is what it stored.
func countChunks(dir string) (int, error) {
	n := 0
	err := filepath.WalkDir(dir, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.Type().IsRegular() && strings.HasSuffix(d.Name(), ".json") {
			n++
		}
		return nil
	})
	return n, err
}

func warmMarkdown(rep *Report) string {
	var b strings.Builder
	w := func(f string, a ...any) { fmt.Fprintf(&b, f, a...) }
	chunks, cached, secs, errs := 0, 0, 0.0, 0
	for _, r := range rep.Warmed {
		chunks += r.Chunks
		cached += r.Cached
		secs += r.GcsimSeconds
		if r.Error != "" {
			errs++
		}
	}
	scope := strings.Join(rep.Only, ", ")
	if rep.Shard != "" {
		scope += ", part " + rep.Shard + " of the teams"
	}
	w("## gcsim cache\n\ngcsim alone (-gcsim-only) on %s: %d builds, %d errors; %d chunks of %d seeds, %d of them already cached; gcsim %.0f s.\n\n", scope, len(rep.Warmed), errs, chunks, harness.Chunk, cached, secs)
	if len(rep.Warmed) > 0 {
		w("| Team | Build | Iterations | Chunks cached | gcsim s | Error |\n|---|---|---|---|---|---|\n")
		for _, r := range rep.Warmed {
			it := strconv.Itoa(r.Iter)
			if r.Seeds != "" {
				it += " (of " + r.Seeds + ")"
			}
			w("| %s | %s | %s | %d of %d | %.0f | %s%s |\n", r.Team, r.Variant, it, r.Cached, r.Chunks, r.GcsimSeconds, r.Error, r.Stopped)
		}
	}
	return b.String()
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
		m := metric(c, g.Chars[c], rc)
		m.Minor = g.Chars[c].Mean < *charShare*g.Team.Mean
		ms = append(ms, m)
	}
	return ms
}

// settled decides each inconclusive verdict by its estimate: pass inside
// the bound, fail outside it.
func settled(ms []Metric) []Metric {
	for i := range ms {
		if ms[i].Verdict != "inconclusive" {
			continue
		}
		ms[i].Settled = true
		ms[i].Verdict = "pass"
		if math.Abs(ms[i].Diff) > bound {
			ms[i].Verdict = "fail"
		}
	}
	return ms
}

func overall(ms []Metric) string {
	v := "pass"
	for _, m := range ms {
		if m.Minor {
			continue
		}
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

var spacedEq = regexp.MustCompile(`\s*=\s*`)

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
		// gcsl allows spaces around "=" (hp% = 0.466)
		for _, kv := range strings.Fields(spacedEq.ReplaceAllString(m[4], "=")) {
			k, s, ok := strings.Cut(kv, "=")
			x, err := product(s)
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

// A probe replaces lines of a team's library build, and hunks of its
// rotation.
type probe struct {
	v     variant
	team  string
	lines []string
	hunks []hunk
}

// A hunk replaces consecutive lines of a team's rotation.gcsl: the lines
// in old, compared without leading and trailing space, must occur exactly
// once in the rotation, and new takes their place.
type hunk struct {
	old, new []string
}

var probeLine = regexp.MustCompile(`^\s*(\S+)\s+(char|add\s+weapon|add\s+set|add\s+stats)\b`)

// probesFor reads the probes of a team (see parseProbe).
func probesFor(id string) []probe {
	ents, err := os.ReadDir(*probesDir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		fail(err)
	}
	var out []probe
	for _, e := range ents {
		if e.IsDir() || !strings.HasSuffix(e.Name(), ".txt") {
			continue
		}
		b, err := os.ReadFile(filepath.Join(*probesDir, e.Name()))
		if err != nil {
			fail(err)
		}
		pr, err := parseProbe(strings.TrimSuffix(e.Name(), ".txt"), string(b))
		if err != nil {
			fail(err)
		}
		if pr.team == id {
			out = append(out, pr)
		}
	}
	return out
}

// parseProbe reads a probe file: comments (#; the first is its
// description), a line "team <id>", build lines and rotation lines. A
// character's char or weapon line replaces its own, and its set or stats
// lines replace all of its set or stats lines. "rotation- <line>" lines
// followed by "rotation+ <line>" lines make a hunk of the rotation: the
// "-" lines must match consecutive lines of the team's rotation.gcsl
// exactly once (leading and trailing space aside), and the "+" lines
// replace them, indented as the first of them. A hunk may have no "+"
// lines (it deletes), not no "-" lines. Hunks apply in order, each to
// the rotation as the ones before it left it.
func parseProbe(name, text string) (probe, error) {
	pr := probe{v: variant{Name: "probe:" + name}}
	inNew := false
	for _, l := range strings.Split(text, "\n") {
		t := strings.TrimSpace(l)
		switch {
		case t == "":
		case strings.HasPrefix(t, "#"):
			if pr.v.Desc == "" {
				pr.v.Desc = strings.TrimSpace(strings.TrimPrefix(t, "#"))
			}
		case strings.HasPrefix(t, "team "):
			pr.team = strings.TrimSpace(strings.TrimPrefix(t, "team "))
		case t == "rotation-" || strings.HasPrefix(t, "rotation- "):
			if len(pr.hunks) == 0 || inNew {
				pr.hunks = append(pr.hunks, hunk{})
				inNew = false
			}
			h := &pr.hunks[len(pr.hunks)-1]
			h.old = append(h.old, strings.TrimSpace(strings.TrimPrefix(t, "rotation-")))
		case t == "rotation+" || strings.HasPrefix(t, "rotation+ "):
			if len(pr.hunks) == 0 {
				return pr, fmt.Errorf("probe %s: %q has no rotation- line before it to replace", name, t)
			}
			h := &pr.hunks[len(pr.hunks)-1]
			h.new = append(h.new, strings.TrimSpace(strings.TrimPrefix(t, "rotation+")))
			inNew = true
		case probeLine.MatchString(t):
			pr.lines = append(pr.lines, t)
		default:
			return pr, fmt.Errorf("probe %s: cannot read %q", name, t)
		}
	}
	if pr.team == "" {
		return pr, fmt.Errorf("probe %s: no team line", name)
	}
	if len(pr.lines) == 0 && len(pr.hunks) == 0 {
		return pr, fmt.Errorf("probe %s: changes nothing", name)
	}
	return pr, nil
}

func lineKey(l string) (string, bool) {
	m := probeLine.FindStringSubmatch(l)
	if m == nil {
		return "", false
	}
	return m[1] + " " + strings.Join(strings.Fields(m[2]), " "), true
}

func (pr probe) apply(build string) string {
	repl := map[string][]string{}
	for _, l := range pr.lines {
		k, _ := lineKey(l)
		repl[k] = append(repl[k], l)
	}
	var out []string
	done := map[string]bool{}
	for _, l := range strings.Split(build, "\n") {
		k, ok := lineKey(l)
		if !ok || repl[k] == nil {
			out = append(out, l)
			continue
		}
		if !done[k] {
			out = append(out, repl[k]...)
			done[k] = true
		}
	}
	for k := range repl {
		if !done[k] {
			fail(fmt.Errorf("probe %s: the build has no line %q to replace", pr.v.Name, k))
		}
	}
	return strings.Join(out, "\n")
}

// applyRotation applies the probe's hunks to a rotation. A hunk that
// matches nowhere, or in more than one place, is an error: the probe
// would otherwise compare a rotation other than the one it means.
func (pr probe) applyRotation(rot string) (string, error) {
	lines := strings.Split(rot, "\n")
	for i, h := range pr.hunks {
		var at []int
		for s := 0; s+len(h.old) <= len(lines); s++ {
			ok := true
			for j, o := range h.old {
				if strings.TrimSpace(lines[s+j]) != o {
					ok = false
					break
				}
			}
			if ok {
				at = append(at, s)
			}
		}
		if len(at) != 1 {
			return "", fmt.Errorf("probe %s: rotation hunk %d (%q) matches %d places in the rotation, not one", pr.v.Name, i+1, strings.Join(h.old, " / "), len(at))
		}
		s := at[0]
		l := lines[s]
		indent := l[:len(l)-len(strings.TrimLeft(l, " \t"))]
		var repl []string
		for _, n := range h.new {
			repl = append(repl, indent+n)
		}
		lines = slices.Concat(lines[:s], repl, lines[s+len(h.old):])
	}
	return strings.Join(lines, "\n"), nil
}

// teamDir is the team directory a probe dumps from: the library's, or,
// when the probe changes the rotation, a copy of it in tmp with the
// probe's rotation.gcsl.
func (pr probe) teamDir(tmp, id string) (string, error) {
	dir := filepath.Join(*gscouterDir, "teams", id)
	if len(pr.hunks) == 0 {
		return filepath.Join("teams", id), nil
	}
	rot, err := os.ReadFile(filepath.Join(dir, "rotation.gcsl"))
	if err != nil {
		return "", err
	}
	out, err := pr.applyRotation(string(rot))
	if err != nil {
		return "", err
	}
	cp, err := filepath.Abs(filepath.Join(tmp, id+"-"+strings.ReplaceAll(pr.v.Name, ":", "-")+"-team"))
	if err != nil {
		return "", err
	}
	if err := os.RemoveAll(cp); err != nil {
		return "", err
	}
	if err := os.CopyFS(cp, os.DirFS(dir)); err != nil {
		return "", err
	}
	if err := os.WriteFile(filepath.Join(cp, "rotation.gcsl"), []byte(out), 0o644); err != nil {
		return "", err
	}
	return cp, nil
}

// product reads a stat value, which gcsim lets be a product (0.0496*6).
func product(s string) (float64, error) {
	x := 1.0
	for _, f := range strings.Split(s, "*") {
		v, err := strconv.ParseFloat(f, 64)
		if err != nil {
			return 0, err
		}
		x *= v
	}
	return x, nil
}

func markdown(rep *Report) string {
	var b strings.Builder
	w := func(f string, a ...any) { fmt.Fprintf(&b, f, a...) }
	if len(rep.Results) > 0 {
		passed, notRun, deferred := 0, 0, 0
		for _, r := range rep.Results {
			switch {
			case r.Verdict == "pass" || r.Verdict == "both-error" || r.Verdict == "idle":
				passed++
			case r.Verdict == "deferred":
				deferred++
			case (r.Verdict == "unsupported" || r.Verdict == "dry-run") && r.Build != nil:
				notRun++
			}
		}
		scope := strings.Join(rep.Only, ", ")
		if rep.Shard != "" {
			scope += ", part " + rep.Shard + " of the teams"
		}
		w("## Accuracy\n\nCompared: %s. %d of %d pass.", scope, passed, len(rep.Results)-notRun-deferred)
		if deferred > 0 {
			w(" **%d builds are deferred**: -deadline (%s) passed before they started, so they have no verdict; the next run takes them.", deferred, rep.Deadline)
		}
		if notRun > 0 {
			w(" %d build-space builds are not run: the engine cannot run them, or -dry-run (under \"Build space\").", notRun)
		}
		w("\n\n")
		w("Bound ±%.0f%% of gcsim; a verdict is pass when the 95%% interval of the difference lies inside it, fail when it lies outside, inconclusive otherwise", 100*bound)
		if rep.Settle {
			w(", and an interval still across it at %d iterations is decided by its estimate (settled)", rep.MaxIter)
		}
		w(". Builds run %d iterations, and up to %d when they fail or are inconclusive there. Characters below %.0f%% of gcsim's team DPS are shown, not judged (minor). DPS over the window from %.0f s.\n\n", rep.Iter, rep.MaxIter, 100*rep.Share, rep.Warmup)
		if d := rep.Drift; d != nil {
			w("Drift: the team's difference averages %+.3f%% ± %.3f%% (95%%) over the %d builds compared.\n\n", 100*d.Mean, 100*z*d.SE, d.Builds)
		}
		var idles []string
		for _, r := range rep.Results {
			if r.Verdict == "idle" {
				idles = append(idles, fmt.Sprintf("%s / %s (gcsim %.1f, gse %.1f)", r.Team, r.Variant, r.Metrics[0].Gcsim, r.Metrics[0].Gse))
			}
		}
		if len(idles) > 0 {
			w("### Idle\n\nTeam DPS below %d in both engines: the rotation does nothing in the scored window in either engine; fix it in Gscouter's library. Counted as passing, out of the drift, and unverified in the manifest.\n\n", idleDPS)
			for _, s := range idles {
				w("- %s\n", s)
			}
			w("\n")
		}
		w("| Team | Build | Iterations | Verdict | Who | gcsim | gse | Difference | ±95%% |\n|---|---|---|---|---|---|---|---|---|\n")
		for _, r := range rep.Results {
			if (r.Verdict == "unsupported" || r.Verdict == "dry-run") && r.Build != nil || r.Verdict == "deferred" {
				continue
			}
			if len(r.Metrics) == 0 {
				w("| %s | %s | %d | **%s** | | | | %s | |\n", r.Team, r.Variant, r.Iter, r.Verdict, r.Error)
				continue
			}
			for i, m := range r.Metrics {
				team, build, it, verdict := "", "", "", ""
				if i == 0 {
					team, build, it, verdict = r.Team, r.Variant, strconv.Itoa(r.Iter), "**"+r.Verdict+"**"
					if r.Reused {
						verdict += " (reused)"
					}
				}
				mv := ""
				if m.Verdict != "pass" {
					mv = " (" + m.Verdict + ")"
				}
				if m.Settled {
					mv += " (settled)"
				}
				if m.Minor {
					mv += " (minor)"
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
			if r.Build != nil && len(r.Metrics) == 0 {
				// a build-space build not run says nothing of energy
				continue
			}
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
	buildSpaceMarkdown(w, rep)
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
	// the most blocking of each kind: characters are the unit of porting
	kinds := map[string][]Blocker{}
	for _, bl := range c.Blockers {
		k, _, _ := strings.Cut(bl.Name, " ")
		kinds[k] = append(kinds[k], bl)
	}
	for _, k := range []string{"character", "weapon", "set", "action", "builtin"} {
		bs := kinds[k]
		if len(bs) == 0 {
			continue
		}
		var top []string
		for i, bl := range bs {
			if i == 15 {
				break
			}
			top = append(top, fmt.Sprintf("%s %d", strings.TrimPrefix(bl.Name, k+" "), bl.Teams))
		}
		w("- **%s** (%d unsupported): %s\n", k, len(bs), strings.Join(top, ", "))
	}
	for k := range kinds {
		if !slices.Contains([]string{"character", "weapon", "set", "action", "builtin"}, k) {
			var top []string
			for _, bl := range kinds[k] {
				top = append(top, fmt.Sprintf("%s %d", bl.Name, bl.Teams))
			}
			w("- %s\n", strings.Join(top, ", "))
		}
	}
	w("\n")
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
	// the teams closest to support show what to port for them
	var near []string
	for _, s := range c.Status {
		if s.Status == "unsupported" && len(s.Unsupported) <= 2 {
			var what []string
			for _, u := range s.Unsupported {
				what = append(what, strings.TrimPrefix(u, "unsupported: "))
			}
			near = append(near, fmt.Sprintf("- %s: %s", s.ID, strings.Join(what, ", ")))
		}
	}
	if len(near) > 0 {
		sort.Strings(near)
		w("\nTeams one or two items from support:\n\n%s\n", strings.Join(near, "\n"))
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
