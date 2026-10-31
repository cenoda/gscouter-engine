// Command reference runs a gcsim config and prints the numbers the Rust
// engine is checked against: team and character DPS over a window, their
// standard errors, waits, and the time the simulation took.
//
// Usage:
//
//	reference -config cfg.txt -iter 1000 -warmup 150
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"runtime"
	"time"

	"github.com/cenoda/gscouter-engine/reference/harness"
)

func main() {
	path := flag.String("config", "", "gcsim config file")
	iter := flag.Int("iter", 1000, "iterations")
	warmup := flag.Float64("warmup", 0, "seconds excluded from the window")
	seed := flag.Int64("seed", 1, "seed of the first iteration")
	workers := flag.Int("j", runtime.NumCPU(), "workers")
	abil := flag.Bool("abil", false, "count hits and damage by ability")
	cacheDir := flag.String("cache", harness.DefaultCacheDir(), "directory of stored results, by config and chunk of seeds; empty for none")
	flag.Parse()
	cache, err := harness.OpenCache(*cacheDir)
	if err != nil {
		fail(err)
	}
	raw, err := os.ReadFile(*path)
	if err != nil {
		fail(err)
	}
	start := time.Now()
	out, err := cache.RunCached(string(raw), *iter, *warmup, *seed, *workers, *abil)
	if err != nil {
		fail(err)
	}
	out.Seconds = time.Since(start).Seconds()
	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	enc.Encode(out)
}

func fail(err error) {
	fmt.Fprintln(os.Stderr, "error:", err)
	os.Exit(1)
}
