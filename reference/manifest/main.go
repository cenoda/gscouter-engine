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
	"slices"
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
	// Minor is a character below compare's -char-share of the team's
	// DPS: reported, not judged. Settled is a verdict decided by its
	// estimate at -max-iter, its interval still across the bound.
	Minor   bool `json:"minor"`
	Settled bool `json:"settled"`
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
	Engine   Engine `json:"engine"`
	Gscouter string `json:"gscouter_commit"`
	// Kinds are the kinds of build this release judged: library (with
	// the probes), which is the whole verification (the owner,
	// 2026-11-04), and any other kind a release also ran.
	Kinds []string        `json:"kinds"`
	Teams map[string]Team `json:"teams"`
	// Verified counts the teams verified; Supported those the engine
	// runs the library build of.
	Verified  int `json:"verified"`
	Supported int `json:"supported"`
	// Drift is the mean of the team's difference to gcsim over every
	// build compared, with its standard error: an error the whole engine
	// shares a little of, which no build's verdict shows.
	Drift Drift `json:"drift"`
	// DriftByKind is Drift over each kind of build alone.
	DriftByKind map[string]Drift `json:"drift_by_kind"`
	// GroupALimit is the bound on a team's difference for group A
	// (summary.go), recorded with the groups it drew.
	GroupALimit float64 `json:"group_a_limit"`
}

type Drift struct {
	Builds int     `json:"builds"`
	Mean   float64 `json:"mean"`
	SE     float64 `json:"se"`
}

// driftLimit is the drift above which approval.md warns: well below the
// 1% bound, and far above what thousands of builds' noise leaves.
const driftLimit = 0.002

type Engine struct {
	Commit     string `json:"commit"`
	WasmSHA256 string `json:"wasm_sha256"`
}

type Team struct {
	Verified bool `json:"verified"`
	// VerifiedKinds are the kinds a verified team passed: the release's
	// Kinds.
	VerifiedKinds []string `json:"verified_kinds,omitempty"`
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
	// Settled counts the team's builds with a settled verdict.
	Settled int `json:"settled,omitempty"`
	// Group is a verified team's group for the owner's answer: "A"
	// (approved at once) or "B" (one by one), with GroupWhy for B
	// (summary.go).
	Group    string   `json:"group,omitempty"`
	GroupWhy []string `json:"group_why,omitempty"`
}

// Score is one build's team DPS on gcsim and the engine, with the 95%
// half-width of their difference, and the judged character whose DPS
// differs most; the minor characters (below compare's -char-share, not
// judged) have their own, shown apart.
type Score struct {
	Build          string  `json:"build"`
	Iterations     int     `json:"iterations"`
	Gcsim          float64 `json:"gcsim"`
	Gse            float64 `json:"gse"`
	Diff           float64 `json:"diff"`
	Half95         float64 `json:"half95"`
	WorstChar      string  `json:"worst_char,omitempty"`
	WorstDiff      float64 `json:"worst_diff,omitempty"`
	WorstMinorChar string  `json:"worst_minor_char,omitempty"`
	WorstMinorDiff float64 `json:"worst_minor_diff,omitempty"`
	// Settled is set when the team's or a judged character's verdict
	// was settled.
	Settled bool `json:"settled,omitempty"`
}

// scoreOf reads a result's team metric and its worst characters.
func scoreOf(r result) (Score, bool) {
	s := Score{Build: r.Variant, Iterations: r.Iter}
	found := false
	for _, m := range r.Metrics {
		if m.Settled && !m.Minor {
			s.Settled = true
		}
		switch {
		case m.Name == "team":
			s.Gcsim, s.Gse, s.Diff, s.Half95 = m.Gcsim, m.Gse, m.Diff, 1.96*m.DiffSE
			found = true
		case m.Minor:
			if s.WorstMinorChar == "" || math.Abs(m.Diff) > math.Abs(s.WorstMinorDiff) {
				s.WorstMinorChar, s.WorstMinorDiff = m.Name, m.Diff
			}
		case s.WorstChar == "" || math.Abs(m.Diff) > math.Abs(s.WorstDiff):
			s.WorstChar, s.WorstDiff = m.Name, m.Diff
		}
	}
	return s, found
}

// settled tells whether a result's team or a judged character's verdict
// was settled.
func settled(r result) bool {
	for _, m := range r.Metrics {
		if m.Settled && !m.Minor {
			return true
		}
	}
	return false
}

func driftOf(xs []float64) Drift {
	d := Drift{Builds: len(xs)}
	if len(xs) == 0 {
		return d
	}
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
	approval := flag.String("approval", "", "also write the owner's approval table (Markdown), every verified team, to this file")
	summary := flag.String("summary", "", "also write the page the owner answers (Markdown): counts, drift, group B in full")
	candidates := flag.String("candidates", "", "also write group A's team ids to this file, for approved.txt once the owner answers")
	kindsFlag := flag.String("kinds", "library", "the kinds of build the release ran, each needed for a team's verdict: library, the whole verification (the owner, 2026-11-04), and any of variants, buildspace and pages it also ran")
	approvedPath := flag.String("approved", "", "Gscouter's approved list (internal/engine/approved.txt): its teams are carried forward or listed as dropped, and left out of the groups to answer; none or a missing file for none known")
	flag.Parse()
	need, err := parseKinds(*kindsFlag)
	var m *Manifest
	if err == nil {
		m, err = build(*wasm, *commit, need, flag.Args())
	}
	var approved []approvedTeam
	if err == nil && *approvedPath != "" {
		var raw []byte
		if raw, err = os.ReadFile(*approvedPath); errors.Is(err, os.ErrNotExist) {
			err = nil
		} else if err == nil {
			approved, err = parseApproved(string(raw))
			if err == nil && approved == nil {
				approved = []approvedTeam{}
			}
		}
	}
	if err == nil {
		var b []byte
		if b, err = json.MarshalIndent(m, "", "  "); err == nil {
			err = os.WriteFile(*out, append(b, '\n'), 0o644)
		}
	}
	if err == nil && *approval != "" {
		err = os.WriteFile(*approval, []byte(approvalTable(m)), 0o644)
	}
	if err == nil && *summary != "" {
		err = os.WriteFile(*summary, []byte(approvalSummary(m, approved)), 0o644)
	}
	if err == nil && *candidates != "" {
		err = os.WriteFile(*candidates, []byte(candidateList(m, approved)), 0o644)
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, "manifest:", err)
		os.Exit(1)
	}
	fmt.Printf("%d of %d supported teams verified\n", m.Verified, m.Supported)
}

// parseKinds reads -kinds: a comma-separated subset of kinds, in kinds'
// order.
func parseKinds(s string) ([]string, error) {
	want := map[string]bool{}
	for _, k := range strings.Split(s, ",") {
		if k = strings.TrimSpace(k); k == "" {
			continue
		}
		if !slices.Contains(kinds, k) {
			return nil, fmt.Errorf("-kinds: %q is not one of %s", k, strings.Join(kinds, ", "))
		}
		want[k] = true
	}
	var out []string
	for _, k := range kinds {
		if want[k] {
			out = append(out, k)
		}
	}
	if len(out) == 0 {
		return nil, errors.New("-kinds: none given")
	}
	return out, nil
}

func build(wasm, commit string, need []string, paths []string) (*Manifest, error) {
	if wasm == "" || commit == "" || len(paths) == 0 {
		return nil, errors.New("want -wasm, -commit and the reports")
	}
	b, err := os.ReadFile(wasm)
	if err != nil {
		return nil, err
	}
	sum := sha256.Sum256(b)
	m := &Manifest{Engine: Engine{Commit: commit, WasmSHA256: hex.EncodeToString(sum[:])}, Kinds: need, Teams: map[string]Team{}}
	needed := map[string]bool{}
	for _, k := range need {
		needed[k] = true
	}

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
	for _, k := range need {
		if !ran[k] {
			missing = append(missing, k)
		}
	}
	if len(missing) > 0 {
		return nil, fmt.Errorf("no report ran %s: a manifest needs one complete run", strings.Join(missing, ", "))
	}

	var diffs []float64
	byKind := map[string][]float64{}
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
			if k := kindOf(r); k != "probes" && !needed[k] {
				continue // a kind this release does not judge
			}
			if r.Verdict != "idle" && len(r.Metrics) > 0 && r.Metrics[0].Name == "team" && !math.IsInf(r.Metrics[0].Diff, 0) && !math.IsNaN(r.Metrics[0].Diff) {
				diffs = append(diffs, r.Metrics[0].Diff)
				byKind[kindOf(r)] = append(byKind[kindOf(r)], r.Metrics[0].Diff)
			}
			if kindOf(r) != "probes" && settled(r) {
				t.Settled++
			}
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
			case "idle":
				total[k]++
				t.Why = append(t.Why, v+": idle (the rotation does nothing in the scored window in either engine; fix it in Gscouter's library)")
				continue
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
		for _, k := range need {
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
			t.VerifiedKinds = need
			t.Group, t.GroupWhy = group(t, needed["pages"])
		}
		m.Teams[id] = t
	}
	m.Drift = driftOf(diffs)
	m.DriftByKind = map[string]Drift{}
	for k, xs := range byKind {
		m.DriftByKind[k] = driftOf(xs)
	}
	m.GroupALimit = groupALimit
	return m, nil
}
