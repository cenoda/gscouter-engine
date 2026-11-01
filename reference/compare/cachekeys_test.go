package main

import (
	"os"
	"testing"

	"github.com/cenoda/gscouter-engine/reference/harness"
)

// cacheView finds the chunks harness.Cache stores, where it stores them.
func TestCacheKeys(t *testing.T) {
	raw, err := os.ReadFile("../../configs/hutao-solo-looped.txt")
	if err != nil {
		t.Fatal(err)
	}
	cfg := string(raw)
	dir := t.TempDir()
	c, err := harness.OpenCache(dir)
	if err != nil {
		t.Fatal(err)
	}
	v, err := openCacheView(dir)
	if err != nil {
		t.Fatal(err)
	}
	if v.has(cfg, 150, true, 0) {
		t.Fatal("chunk 0 found in an empty cache")
	}
	// the second chunk, seeds 501 to 1000
	if _, err := c.RunCached(cfg, harness.Chunk, 150, 1+harness.Chunk, 4, true); err != nil {
		t.Fatal(err)
	}
	if !v.has(cfg, 150, true, 1) {
		t.Fatal("chunk 1 just stored is not where cacheView looks")
	}
	if v.has(cfg, 150, true, 0) || v.has(cfg, 150, false, 1) || v.has(cfg, 100, true, 1) {
		t.Fatal("cacheView finds a chunk not stored")
	}
	if got := v.cachedFrom(cfg, 150, true, 1); got != 2 {
		t.Fatalf("cachedFrom(1) = %d, want 2", got)
	}
	if got := v.cachedFrom(cfg, 150, true, 0); got != 0 {
		t.Fatalf("cachedFrom(0) = %d, want 0", got)
	}
}

func TestParseSeeds(t *testing.T) {
	for _, tc := range []struct {
		s      string
		lo, hi int
		ok     bool
	}{{"0:1000", 0, 1000, true}, {"500:1500", 500, 1500, true}, {"0:0", 0, 0, false}, {"1000:500", 0, 0, false}, {"0:750", 0, 0, false}, {"-500:500", 0, 0, false}, {"500", 0, 0, false}} {
		lo, hi, err := parseSeeds(tc.s)
		if (err == nil) != tc.ok || tc.ok && (lo != tc.lo || hi != tc.hi) {
			t.Errorf("parseSeeds(%q) = %d, %d, %v", tc.s, lo, hi, err)
		}
	}
}
