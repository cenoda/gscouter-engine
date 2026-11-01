package main

// What gcsim's cache already holds, without running gcsim: compare may grow
// a build past -max-iter on chunks the cache has (gcsim is not run past
// -max-iter), and -gcsim-only -seeds counts the chunks it found cached.
//
// The chunk paths are harness.Cache's (reference/harness/cache.go). They
// are derived here, not exported from harness, because CI keys its cache
// by the hash of reference/harness/*.go, and a change there would start
// the cache afresh. TestCacheKeys checks that the two agree, and compare
// checks it again at run time: the chunks it has just run through the
// cache must be where cacheView looks.

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime/debug"

	"github.com/cenoda/gscouter-engine/reference/harness"
)

// harnessCacheVersion is harness.Cache's version, part of every key.
const harnessCacheVersion = 1

// cacheView reads the chunks of harness.Cache's directory.
type cacheView struct {
	dir   string
	gcsim string
}

// openCacheView is a view of the cache in dir, or nil when dir is empty.
func openCacheView(dir string) (*cacheView, error) {
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
	return &cacheView{dir: dir, gcsim: gcsim}, nil
}

// path is harness.Cache.path.
func (c *cacheView) path(cfg string, warmup float64, abil bool, chunk int) string {
	h := sha256.New()
	fmt.Fprintf(h, "v%d\n%s\nwarmup=%v abil=%v\n%s", harnessCacheVersion, c.gcsim, warmup, abil, cfg)
	key := hex.EncodeToString(h.Sum(nil))
	return filepath.Join(c.dir, key[:2], fmt.Sprintf("%s-%d.json", key, chunk))
}

// has is whether the cache holds the chunk as harness.Cache.load reads it:
// so that RunCached reads it back rather than run gcsim.
func (c *cacheView) has(cfg string, warmup float64, abil bool, chunk int) bool {
	b, err := os.ReadFile(c.path(cfg, warmup, abil, chunk))
	if err != nil {
		return false
	}
	var out harness.Output
	return json.Unmarshal(b, &out) == nil && out.Iterations == harness.Chunk
}

// cachedFrom is the first chunk from chunk on that the cache does not hold.
func (c *cacheView) cachedFrom(cfg string, warmup float64, abil bool, chunk int) int {
	for c.has(cfg, warmup, abil, chunk) {
		chunk++
	}
	return chunk
}
