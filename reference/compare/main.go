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
// further, up to -max-iter, while any verdict is inconclusive: each step
// by the factor at which the inconclusive intervals would clear the bound,
// from twice to four times.
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
// its probe build: weapons and sets the engine lacks can be replaced.
//
// gcsim's results depend only on the config and the seeds, and the cache
// keeps them, so -gcsim-only runs gcsim alone on the builds selected, for
// teams the engine does not support yet too: seeds from 1 in whole chunks,
// to -iter and then to the iterations at which gcsim's own standard errors
// reach -target-se, up to -max-iter, as compare would. It warms the cache
// while a port is written, so that the port's compare runs only the engine.
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
	buildsFlag  = flag.String("builds", "", "comma-separated team/build pairs as the report names them (team/library, team/er-0, team/cons:xingqiu=C0, team/probe:kuki-c6): compare only those, of any kind; implies -only library,variants,buildspace,probes and -teams")
	censusOnly  = flag.Bool("census", false, "only classify teams as supported or not; run nothing")
	noVariants  = flag.Bool("no-variants", false, "compare library builds and probes only (same as -only library,probes)")
	onlyFlag    = flag.String("only", "library,variants,probes", "comma-separated kinds of build to compare: library, variants, buildspace, probes")
	bsWeapons   = flag.Int("buildspace-weapons", 2, "build space: other weapons of the class per member, chosen by a hash of team, member and weapon (0: every one the engine has)")
	dryRun      = flag.Bool("dry-run", false, "list the builds that would be compared, and compare none")
	shardFlag   = flag.String("shard", "", "k/n: compare only the k-th of n parts of the teams that have builds to compare (k from 0); the census still covers every team")
	iter0       = flag.Int("iter", 1000, "iterations of the first batch")
	maxIter     = flag.Int("max-iter", 512000, "most iterations a config may grow to")
	targetSE    = flag.Float64("target-se", 0.003, "standard error, relative to the mean, each engine must reach")
	warmup      = flag.Float64("warmup", 150, "seconds of the looped fight excluded from the window")
	workers     = flag.Int("j", runtime.NumCPU(), "workers")
	jsonOut     = flag.String("json", "", "write the results as JSON to this file")
	mdOut       = flag.String("md", "", "also write the report to this file")
	probesDir   = flag.String("probes", "probes", "directory of probes: library builds with a weapon, set or character line changed")
	cacheDir    = flag.String("cache", harness.DefaultCacheDir(), "directory of stored gcsim results, by config and chunk of seeds; empty for none")
	deadline    = flag.Duration("deadline", 0, "after this long, start no more builds and grow none further; builds not started are reported as deferred (0: no limit)")
	gcsimOnly   = flag.Bool("gcsim-only", false, "run only gcsim, into the cache, on the selected builds, whether the engine supports their teams or not")

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
}

// op scales a stat in every `add stats` line, then adds to the last one.
type op struct {
	stat  string
	scale float64
	add   float64
}

var variants = []variant{
	{"er-0", "no ER from artifacts", []op{{"er", 0, 0}}, false, nil},
	{"er-0-forced", "no ER from artifacts, bursts used without their readiness checks so that they wait for energy", []op{{"er", 0, 0}}, true, nil},
	{"er-half", "half the ER from artifacts", []op{{"er", 0.5, 0}}, false, nil},
	{"er-half-forced", "half the ER from artifacts, bursts used without their readiness checks", []op{{"er", 0.5, 0}}, true, nil},
	{"er+", "ER +30%", []op{{"er", 1, 0.3}}, false, nil},
	{"crit-", "CR −20%, CD −40%", []op{{"cr", 1, -0.2}, {"cd", 1, -0.4}}, false, nil},
	{"crit+", "CR +20%, CD +40%", []op{{"cr", 1, 0.2}, {"cd", 1, 0.4}}, false, nil},
	{"atk+", "ATK +40%", []op{{"atk%", 1, 0.4}}, false, nil},
	{"hp+", "HP +40%", []op{{"hp%", 1, 0.4}}, false, nil},
	{"em+", "EM +150", []op{{"em", 1, 150}}, false, nil},
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
}

type job struct {
	v   variant
	cfg string
	err error
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
	if *gcsimOnly && gcsimCache == nil {
		fail(errors.New("-gcsim-only needs a cache (-cache)"))
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

	ids := listTeams()
	if *teamsFlag != "" {
		ids = strings.Split(*teamsFlag, ",")
	}
	kinds, err := parseOnly(*onlyFlag)
	if err != nil {
		fail(err)
	}
	if *buildsFlag != "" {
		if *teamsFlag != "" || *gcsimOnly || *censusOnly {
			fail(fmt.Errorf("-builds takes no -teams, -gcsim-only or -census"))
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
		kinds, err = parseOnly("library,variants,buildspace,probes")
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
	rep := &Report{Date: time.Now().UTC().Format(time.RFC3339), Gscouter: gitHead(*gscouterDir), Bound: bound, TargetSE: *targetSE, Warmup: *warmup, Only: slices.Sorted(mapsKeys(kinds)), Shard: *shardFlag}
	if *deadline > 0 {
		rep.Deadline = deadline.String()
	}
	if kinds["variants"] {
		rep.Variants = variants
	}
	if kinds["buildspace"] {
		if *gcsimOnly {
			// its weapons are those the engine has, its teams the supported
			fail(errors.New("-gcsim-only does not take buildspace: it needs the engine"))
		}
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
		} else if *gcsimOnly {
			// gcsim needs no support from the engine
			st.Status = "dumped"
			configs[i] = cfg
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
		// the teams with builds to compare, in the order listed, split
		// into parts by position
		part := 0
		for i, id := range ids {
			supported := statuses[i].Status == "supported"
			// -gcsim-only takes every team, and reports one whose dump failed
			selected := supported || *gcsimOnly
			var probes []probe
			if kinds["probes"] {
				probes = probesFor(id)
			}
			if !(selected && (kinds["library"] || kinds["variants"] || kinds["buildspace"])) && len(probes) == 0 {
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
				cfg, err := dump(gscouter, id, path)
				if err != nil {
					dumpErr(pr.v, err)
					continue
				}
				jobs = append(jobs, job{v: pr.v, cfg: cfg})
			}
			for _, j := range jobs {
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
						w = warm(id, j.v, j.cfg)
					}
					fmt.Fprintf(os.Stderr, "%s / %s: %d iterations, %d of %d chunks cached, gcsim %.0f s %s\n", id, j.v.Name, w.Iter, w.Cached, w.Chunks, w.GcsimSeconds, w.Error)
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
				r := compare(gse, tmp, id, j.v, j.cfg)
				fmt.Fprintf(os.Stderr, "%s / %s: %s at %d iterations\n", id, j.v.Name, r.Verdict, r.Iter)
				// a probe exists to check what it adds, so it must run
				if r.Verdict == "unsupported" && strings.HasPrefix(j.v.Name, "probe:") {
					r.Verdict = "error"
				}
				// a feature only the run finds makes the team unsupported,
				// unless a build-space build alone carries it
				if r.Verdict == "unsupported" && supported && j.v.change == nil {
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
	for _, r := range rep.Results {
		if r.Verdict != "pass" && r.Verdict != "unsupported" && r.Verdict != "dry-run" && r.Verdict != "deferred" {
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
		case "library", "variants", "buildspace", "probes":
			kinds[k] = true
		case "":
		default:
			return nil, fmt.Errorf("-only: unknown kind %q (library, variants, buildspace, probes)", k)
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

// growth is the factor to grow the iterations by while a verdict is
// inconclusive: from each inconclusive metric's distance to the bound, the
// iterations at which its interval would just clear it, with a quarter more
// for the noise in that estimate. It is at least 2, so that the looks at the
// data stay as few as with a fixed factor, and at most 4, the factor used
// before, so that no build runs longer than it did.
func growth(ms []Metric) float64 {
	g := 2.0
	for _, m := range ms {
		if m.Verdict != "inconclusive" {
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
	run := func(more int) error {
		if gcsimCache != nil {
			// whole chunks, which the cache can keep
			more = (more + harness.Chunk - 1) / harness.Chunk * harness.Chunk
			if n+more > *maxIter {
				more = max(*maxIter-n, 0)
			}
		}
		start := time.Now()
		gb, err := gcsimCache.RunCached(cfg, more, *warmup, int64(1+n), *workers, true)
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
	for err == nil {
		res.Metrics = metrics(g, r)
		res.Verdict = overall(res.Metrics)
		// past -deadline an inconclusive build stays inconclusive
		if res.Verdict != "inconclusive" || n >= *maxIter || pastDeadline() {
			break
		}
		err = run(min(int(float64(n)*growth(res.Metrics)), *maxIter) - n)
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
		gb, err := gcsimCache.RunCached(cfg, more, *warmup, int64(1+n), *workers, true)
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
			w("| %s | %s | %d | %d of %d | %.0f | %s |\n", r.Team, r.Variant, r.Iter, r.Cached, r.Chunks, r.GcsimSeconds, r.Error)
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

// A probe replaces lines of a team's library build.
type probe struct {
	v     variant
	team  string
	lines []string
}

var probeLine = regexp.MustCompile(`^\s*(\S+)\s+(char|add\s+weapon|add\s+set|add\s+stats)\b`)

// probesFor reads the probes of a team. A probe file is comments (#), a
// line "team <id>", and build lines: a character's char or weapon line
// replaces its own, and its set or stats lines replace all of its set or
// stats lines.
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
		pr := probe{v: variant{Name: "probe:" + strings.TrimSuffix(e.Name(), ".txt")}}
		for _, l := range strings.Split(string(b), "\n") {
			t := strings.TrimSpace(l)
			switch {
			case t == "":
			case strings.HasPrefix(t, "#"):
				if pr.v.Desc == "" {
					pr.v.Desc = strings.TrimSpace(strings.TrimPrefix(t, "#"))
				}
			case strings.HasPrefix(t, "team "):
				pr.team = strings.TrimSpace(strings.TrimPrefix(t, "team "))
			case probeLine.MatchString(t):
				pr.lines = append(pr.lines, t)
			default:
				fail(fmt.Errorf("probe %s: cannot read %q", e.Name(), t))
			}
		}
		if pr.team == id {
			out = append(out, pr)
		}
	}
	return out
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
			case r.Verdict == "pass":
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
		w("Bound ±%.0f%% of gcsim; a verdict is pass when the 95%% interval of the difference lies inside it, fail when it lies outside, inconclusive otherwise. DPS over the window from %.0f s.\n\n", 100*bound, rep.Warmup)
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
