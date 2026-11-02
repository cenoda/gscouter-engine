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
	"math"
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
	Iter    int      `json:"iterations"`
	Metrics []metric `json:"metrics"`
}

// metric is one DPS of a build, team or character: Diff is
// (gse - gcsim) / gcsim, DiffSE its standard error.
type metric struct {
	Name   string  `json:"name"`
	Gcsim  float64 `json:"gcsim"`
	Gse    float64 `json:"gse"`
	Diff   float64 `json:"diff"`
	DiffSE float64 `json:"diff_se"`
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
	// Scores are the library build's and the one-pass page config's
	// DPS on both engines, for the owner, who approves each team that
	// moves to the engine.
	Scores []Score `json:"scores,omitempty"`
}

// Score is one build's team DPS on gcsim and the engine, with the 95%
// half-width of their difference, and the character whose DPS differs
// most.
type Score struct {
	Build      string  `json:"build"`
	Iterations int     `json:"iterations"`
	Gcsim      float64 `json:"gcsim"`
	Gse        float64 `json:"gse"`
	Diff       float64 `json:"diff"`
	Half95     float64 `json:"half95"`
	WorstChar  string  `json:"worst_char,omitempty"`
	WorstDiff  float64 `json:"worst_diff,omitempty"`
}

// scoreOf reads a result's team metric and its worst character.
func scoreOf(r result) (Score, bool) {
	s := Score{Build: r.Variant, Iterations: r.Iter}
	found := false
	for _, m := range r.Metrics {
		if m.Name == "team" {
			s.Gcsim, s.Gse, s.Diff, s.Half95 = m.Gcsim, m.Gse, m.Diff, 1.96*m.DiffSE
			found = true
		} else if s.WorstChar == "" || math.Abs(m.Diff) > math.Abs(s.WorstDiff) {
			s.WorstChar, s.WorstDiff = m.Name, m.Diff
		}
	}
	return s, found
}

// approvalTable is the Markdown the owner answers: each verified team's
// scores on both engines. A team moves to the engine only with the
// owner's yes for it (CLAUDE.md), recorded in Gscouter's approved list.
func approvalTable(m *Manifest) string {
	var ids []string
	for id, t := range m.Teams {
		if t.Verified {
			ids = append(ids, id)
		}
	}
	sort.Strings(ids)
	var b strings.Builder
	fmt.Fprintf(&b, "# Teams to approve\n\nEngine %s (module %s), Gscouter %s: %d of %d supported teams verified. Each verified team's library build (the score a result page starts from) and its one-pass page config, gcsim against the engine; ±95%% is the half-width of the difference's interval. A team moves to the engine only with the owner's yes.\n\n", short(m.Engine.Commit), short(m.Engine.WasmSHA256), short(m.Gscouter), m.Verified, m.Supported)
	b.WriteString("| Team | Build | Iterations | gcsim | engine | Difference | ±95% | Character most apart | Builds passed |\n|---|---|---|---|---|---|---|---|---|\n")
	for _, id := range ids {
		t := m.Teams[id]
		passed := fmt.Sprintf("library %s, variants %s, build space %s, pages %s", t.Builds["library"], t.Builds["variants"], t.Builds["buildspace"], t.Builds["pages"])
		for i, s := range t.Scores {
			name, info := id, passed
			if i > 0 {
				name, info = "", ""
			}
			worst := ""
			if s.WorstChar != "" {
				worst = fmt.Sprintf("%s %+.2f%%", s.WorstChar, 100*s.WorstDiff)
			}
			fmt.Fprintf(&b, "| %s | %s | %d | %.1f | %.1f | %+.2f%% | %.2f%% | %s | %s |\n", name, s.Build, s.Iterations, s.Gcsim, s.Gse, 100*s.Diff, 100*s.Half95, worst, info)
		}
	}
	return b.String()
}

func short(s string) string {
	if len(s) > 12 {
		return s[:12]
	}
	return s
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
	approval := flag.String("approval", "", "also write the owner's approval table (Markdown) to this file")
	flag.Parse()
	m, err := build(*wasm, *commit, flag.Args())
	if err == nil {
		var b []byte
		if b, err = json.MarshalIndent(m, "", "  "); err == nil {
			err = os.WriteFile(*out, append(b, '\n'), 0o644)
		}
	}
	if err == nil && *approval != "" {
		err = os.WriteFile(*approval, []byte(approvalTable(m)), 0o644)
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
			if v == "library" || v == "page:once" {
				if s, ok := scoreOf(r); ok {
					t.Scores = append(t.Scores, s)
				}
			}
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
		sort.SliceStable(t.Scores, func(i, j int) bool { return t.Scores[i].Build == "library" && t.Scores[j].Build != "library" })
		t.Verified = len(t.Why) == 0
		if t.Verified {
			m.Verified++
		}
		m.Teams[id] = t
	}
	return m, nil
}
