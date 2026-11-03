package main

// Verdicts reused across runs. The engine is deterministic: a build whose
// engine output on its first fpIter seeds is the same, to the last bit, as
// when a verdict was stored ran the same engine code on those fights, and
// gcsim's side is fixed by the config and the seeds. With -reuse, such a
// build takes the stored verdict instead of running again, so a push that
// changes one character runs in full only the builds whose fights it moves.
//
// The fingerprint is the engine's whole output (team, characters, waits,
// abilities' hits and damage) over fpIter fights, on fpWorkers threads (its
// sums depend on how the fights are split between threads). A change that
// moves no bit of those fights and only later ones would be missed, so CI
// reuses on work branches only: main's runs, the schedule and releases run
// every build in full and store what they find. A fingerprint that differs
// for another reason (another machine) only costs the full run.
//
// Verdicts are stored beside gcsim's chunks, under <-cache>/verdicts, so
// CI's cache jobs keep and merge them as they do the chunks.

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
)

var reuseFlag = flag.Bool("reuse", false, "take the stored verdict of a build whose engine output on its first 500 seeds is the same to the last bit as when the verdict was stored (in -cache's verdicts/); every run with -cache stores what it finds")

const (
	fpIter    = 500
	fpWorkers = 4
	// verdictVersion is part of every verdict's key. Raise it when compare
	// decides differently on the same results (the bound, the growth, the
	// settling, what a verdict holds).
	verdictVersion = 1
)

type storedVerdict struct {
	Fingerprint string `json:"fingerprint"`
	Result      Result `json:"result"`
}

// fingerprint is the hash of the engine's output on seeds 1 to fpIter, its
// timing left out.
func fingerprint(gse, path string, warmup float64) (string, error) {
	out, err := exec.Command(gse, path, strconv.Itoa(fpIter), fmt.Sprint(warmup), strconv.Itoa(fpWorkers), "--seed", "1").Output()
	if err != nil {
		return "", err
	}
	var m map[string]any
	if err := json.Unmarshal(out, &m); err != nil {
		return "", err
	}
	delete(m, "seconds")
	b, err := json.Marshal(m) // keys sorted, floats exact
	if err != nil {
		return "", err
	}
	h := sha256.Sum256(b)
	return hex.EncodeToString(h[:]), nil
}

// verdictPath is where a build's verdict is stored: by gcsim's version, the
// settings that decide a verdict, the warmup and the config.
func verdictPath(cfg string, warmup float64) string {
	h := sha256.New()
	fmt.Fprintf(h, "verdict v%d\n%s\nbound=%v iter=%d max-iter=%d target-se=%v settle=%v char-share=%v warmup=%v\n%s",
		verdictVersion, cacheKeys.gcsim, bound, *iter0, *maxIter, *targetSE, *settle, *charShare, warmup, cfg)
	k := hex.EncodeToString(h.Sum(nil))
	return filepath.Join(cacheKeys.dir, "verdicts", k[:2], k+".json")
}

// storable is a verdict that the same engine output always gives: not one
// decided by its estimate or still inconclusive (more of gcsim's seeds in
// the cache can change those), not an error or a deferral.
func storable(r Result) bool {
	switch r.Verdict {
	case "pass", "fail", "idle", "both-error":
	default:
		return false
	}
	for _, m := range r.Metrics {
		if m.Settled {
			return false
		}
	}
	return true
}

// compareReusing is compare, taking a stored verdict when -reuse is set and
// the build's fingerprint matches it, and storing a storable verdict.
func compareReusing(gse, tmp, id string, v variant, cfg string) Result {
	if cacheKeys == nil {
		return compare(gse, tmp, id, v, cfg)
	}
	fp, err := fingerprint(gse, writeConfig(tmp, id+"-"+v.Name+"-fp", cfg), v.warm())
	if err != nil {
		// the engine refuses or fails: compare says how
		return compare(gse, tmp, id, v, cfg)
	}
	path := verdictPath(cfg, v.warm())
	if *reuseFlag {
		if b, err := os.ReadFile(path); err == nil {
			var s storedVerdict
			if json.Unmarshal(b, &s) == nil && s.Fingerprint == fp {
				r := s.Result
				r.Reused = true
				r.GcsimSeconds, r.GseSeconds = 0, 0
				return r
			}
		}
	}
	r := compare(gse, tmp, id, v, cfg)
	if storable(r) {
		if b, err := json.Marshal(storedVerdict{Fingerprint: fp, Result: r}); err == nil {
			if os.MkdirAll(filepath.Dir(path), 0o755) == nil {
				tmpPath := path + ".tmp"
				if os.WriteFile(tmpPath, b, 0o644) == nil {
					os.Rename(tmpPath, path)
				}
			}
		}
	}
	return r
}
