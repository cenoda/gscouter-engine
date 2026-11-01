// Command manifest writes the list of library teams Gscouter may run on the
// engine, from the reports of one complete compare run (every kind of
// build, every part, at one commit) and the WebAssembly module built at
// that commit.
//
// Usage:
//
//	manifest -wasm gse_wasm.wasm -commit <sha> -out manifest.json compare-*/compare.json
//
// A team is verified when the census counts it supported and its library
// build, every variant, every page config the engine runs and every
// build-space build the engine runs pass. Nothing else counts: a build
// that failed, stayed inconclusive, errored or was deferred, or a kind of
// build with no result at all, leaves the team unverified, and the
// manifest says why. Page configs and build-space builds the engine
// cannot run are listed, for the caller to send to gcsim.
//
// The module's SHA-256 names the engine: the manifest holds only for that
// module, and Gscouter's cache key carries it.
package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"os"
	"sort"
	"strings"
)

// The parts of compare's report that the manifest reads.
type result struct {
	Team    string `json:"team"`
	Variant string `json:"variant"`
	Verdict string `json:"verdict"`
	Error   string `json:"error"`
	Build   *struct {
		Axis string `json:"axis"`
	} `json:"build"`
}

type report struct {
	Gscouter string   `json:"gscouter_commit"`
	Only     []string `json:"only"`
	Results  []result `json:"results"`
	Census   struct {
		Status []struct {
			ID     string `json:"id"`
			Status string `json:"status"`
		} `json:"status"`
	} `json:"census"`
}

// Manifest is what Gscouter reads.
type Manifest struct {
	Engine   Engine          `json:"engine"`
	Gscouter string          `json:"gscouter_commit"`
	Teams    map[string]Team `json:"teams"`
	// Verified counts the teams verified; Supported those the engine
	// runs the library build of.
	Verified  int `json:"verified"`
	Supported int `json:"supported"`
}

type Engine struct {
	Commit     string `json:"commit"`
	WasmSHA256 string `json:"wasm_sha256"`
}

type Team struct {
	Verified bool `json:"verified"`
	// Why lists what keeps a team unverified.
	Why []string `json:"why,omitempty"`
	// Builds counts the team's builds by kind: passed of those run.
	Builds map[string]string `json:"builds"`
	// Unsupported lists the page configs and build-space builds the
	// engine cannot run, with what each lacks.
	Unsupported []string `json:"unsupported,omitempty"`
}

// kinds are compare's kinds of build, each needed for a verdict.
var kinds = []string{"library", "variants", "buildspace", "pages"}

func kindOf(r result) string {
	switch {
	case r.Variant == "library":
		return "library"
	case r.Build != nil:
		return "buildspace"
	case strings.HasPrefix(r.Variant, "page"):
		return "pages"
	case strings.HasPrefix(r.Variant, "probe:"):
		return "probes"
	}
	return "variants"
}

func main() {
	wasm := flag.String("wasm", "", "the WebAssembly module built at the commit")
	commit := flag.String("commit", "", "the engine's commit")
	out := flag.String("out", "manifest.json", "file to write")
	flag.Parse()
	m, err := build(*wasm, *commit, flag.Args())
	if err == nil {
		var b []byte
		if b, err = json.MarshalIndent(m, "", "  "); err == nil {
			err = os.WriteFile(*out, append(b, '\n'), 0o644)
		}
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, "manifest:", err)
		os.Exit(1)
	}
	fmt.Printf("%d of %d supported teams verified\n", m.Verified, m.Supported)
}

func build(wasm, commit string, paths []string) (*Manifest, error) {
	if wasm == "" || commit == "" || len(paths) == 0 {
		return nil, errors.New("want -wasm, -commit and the reports")
	}
	b, err := os.ReadFile(wasm)
	if err != nil {
		return nil, err
	}
	sum := sha256.Sum256(b)
	m := &Manifest{Engine: Engine{Commit: commit, WasmSHA256: hex.EncodeToString(sum[:])}, Teams: map[string]Team{}}

	supported := map[string]bool{}
	ran := map[string]bool{} // kinds some report ran
	byTeam := map[string][]result{}
	for _, p := range paths {
		b, err := os.ReadFile(p)
		if err != nil {
			return nil, err
		}
		var r report
		if err := json.Unmarshal(b, &r); err != nil {
			return nil, fmt.Errorf("%s: %w", p, err)
		}
		if m.Gscouter == "" {
			m.Gscouter = r.Gscouter
		} else if r.Gscouter != m.Gscouter {
			return nil, fmt.Errorf("%s: Gscouter at %s, others at %s: the reports are not of one run", p, r.Gscouter, m.Gscouter)
		}
		for _, k := range r.Only {
			ran[k] = true
		}
		for _, s := range r.Census.Status {
			if s.Status == "supported" {
				supported[s.ID] = true
			}
		}
		for _, res := range r.Results {
			byTeam[res.Team] = append(byTeam[res.Team], res)
		}
	}
	var missing []string
	for _, k := range kinds {
		if !ran[k] {
			missing = append(missing, k)
		}
	}
	if len(missing) > 0 {
		return nil, fmt.Errorf("no report ran %s: a manifest needs one complete run", strings.Join(missing, ", "))
	}

	for id := range supported {
		m.Supported++
		t := Team{Builds: map[string]string{}}
		pass, total := map[string]int{}, map[string]int{}
		// a run can report a build twice (a team in two parts never is,
		// but a rerun's report may be added): the last verdict counts
		last := map[string]result{}
		var order []string
		for _, r := range byTeam[id] {
			if _, ok := last[r.Variant]; !ok {
				order = append(order, r.Variant)
			}
			last[r.Variant] = r
		}
		for _, v := range order {
			r := last[v]
			k := kindOf(r)
			if k == "probes" {
				continue // probes check content, not this team
			}
			switch r.Verdict {
			case "pass":
				total[k]++
				pass[k]++
			case "unsupported":
				if k == "pages" || k == "buildspace" {
					t.Unsupported = append(t.Unsupported, v+": "+strings.TrimPrefix(r.Error, "unsupported: "))
					continue
				}
				total[k]++
				t.Why = append(t.Why, v+": unsupported")
			default:
				total[k]++
				t.Why = append(t.Why, v+": "+r.Verdict)
			}
		}
		for _, k := range kinds {
			t.Builds[k] = fmt.Sprintf("%d/%d", pass[k], total[k])
			if total[k] == 0 && k != "buildspace" {
				// a team may have no build-space build the engine runs;
				// every other kind has at least one
				t.Why = append(t.Why, "no "+k+" result")
			}
		}
		sort.Strings(t.Unsupported)
		t.Verified = len(t.Why) == 0
		if t.Verified {
			m.Verified++
		}
		m.Teams[id] = t
	}
	return m, nil
}
