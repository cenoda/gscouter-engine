package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

// The configs a result page runs besides the library's (-only pages).
//
// A page does not run only the looped config gscouter -dump gives: its
// search (Gscouter's steady.Best) tries the team's other rotations and
// filler candidates for members that wait for energy or their skill, and
// the page also scores the rotation it picks once, without a duration
// (the one-pass config, warmup 0). Each of them can run on the engine, so
// each is compared. gse-pages (pages.go.in), built inside Gscouter's
// module with -overlay so that Gscouter stays unmodified, runs the
// search on gcsim with the server's settings and prints the configs.
//
// Pages are compared for the library build and for er-0, whose members
// starve and so bring the fillers. A config the engine cannot run is
// reported as unsupported, as a build-space build is: a page with it
// stays on gcsim.

// pagesSource is gse-pages' source, relative to the reference directory
// compare runs from.
const pagesSource = "compare/pages.go.in"

// buildPages builds gse-pages into tmp and returns its path.
func buildPages(tmp string) (string, error) {
	src, err := filepath.Abs(pagesSource)
	if err != nil {
		return "", err
	}
	if _, err := os.Stat(src); err != nil {
		return "", fmt.Errorf("gse-pages: %w (compare runs from the reference directory)", err)
	}
	gscouter, err := filepath.Abs(*gscouterDir)
	if err != nil {
		return "", err
	}
	overlay, err := json.Marshal(map[string]map[string]string{"Replace": {filepath.Join(gscouter, "cmd", "gse-pages", "main.go"): src}})
	if err != nil {
		return "", err
	}
	overlayPath := filepath.Join(tmp, "pages-overlay.json")
	if err := os.WriteFile(overlayPath, overlay, 0o644); err != nil {
		return "", err
	}
	out := filepath.Join(tmp, "gse-pages")
	cmd := exec.Command("go", "build", "-overlay", overlayPath, "-o", out, "./cmd/gse-pages")
	cmd.Dir = gscouter
	if b, err := cmd.CombinedOutput(); err != nil {
		return "", fmt.Errorf("building gse-pages: %v\n%s", err, b)
	}
	return out, nil
}

// pageConfig is one config gse-pages prints.
type pageConfig struct {
	Name   string  `json:"name"`
	Warmup float64 `json:"warmup"`
	Config string  `json:"config"`
}

// pageConfigs runs gse-pages on a team with the build file given.
func pageConfigs(pages, id, build string) ([]pageConfig, error) {
	cmd := exec.Command(pages, "-team", filepath.Join("teams", id), "-build", build)
	cmd.Dir = *gscouterDir
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	out, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("gse-pages: %v: %s", err, strings.TrimSpace(stderr.String()))
	}
	var cs []pageConfig
	if err := json.Unmarshal(out, &cs); err != nil {
		return nil, fmt.Errorf("gse-pages: %w", err)
	}
	return cs, nil
}

// pageJobs are the jobs of a supported team's pages: its library build's
// and its er-0 build's configs, but the library's own looped config, which
// is the library build. libraryCfg is that config.
func pageJobs(pages, gse, tmp, id, libraryCfg string) []job {
	base, err := os.ReadFile(filepath.Join(*gscouterDir, "teams", id, "build.txt"))
	if err != nil {
		fail(err)
	}
	var er0 variant
	for _, v := range variants {
		if v.Name == "er-0" {
			er0 = v
		}
	}
	builds := []struct{ prefix, build string }{{"page", string(base)}, {"page-er-0", er0.apply(string(base))}}
	var jobs []job
	for _, b := range builds {
		path := filepath.Join(tmp, id+"-"+b.prefix+".txt")
		if err := os.WriteFile(path, []byte(b.build), 0o644); err != nil {
			fail(err)
		}
		cs, err := pageConfigs(pages, id, path)
		if err != nil {
			jobs = append(jobs, job{v: variant{Name: b.prefix, Desc: "a page's configs", page: true}, err: fmt.Errorf("%s %s: %w", id, b.prefix, err)})
			continue
		}
		for _, c := range cs {
			if c.Config == libraryCfg {
				continue
			}
			w := c.Warmup
			v := variant{Name: b.prefix + ":" + c.Name, Desc: pageDesc(b.prefix, c.Name), page: true, warmup: &w}
			if !want(id, v.Name) {
				continue
			}
			miss, err := check(gse, tmp, id+"-"+strings.NewReplacer(":", "-", ".", "-").Replace(v.Name), c.Config)
			if err != nil {
				jobs = append(jobs, job{v: v, err: fmt.Errorf("%s %s: %w", id, v.Name, err)})
				continue
			}
			if len(miss) > 0 {
				var what []string
				for _, u := range miss {
					what = append(what, strings.TrimPrefix(u, "unsupported: "))
				}
				jobs = append(jobs, job{v: v, err: fmt.Errorf("%w: %s", errUnsupported, strings.Join(what, "; "))})
				continue
			}
			jobs = append(jobs, job{v: v, cfg: c.Config})
		}
	}
	return jobs
}

func pageDesc(prefix, name string) string {
	build := "library build"
	if prefix == "page-er-0" {
		build = "no ER from artifacts"
	}
	if name == "once" {
		return build + ", the page's one-pass config (warmup 0)"
	}
	return build + ", the page's search: rotation and candidate " + strings.TrimPrefix(name, "looped:")
}
