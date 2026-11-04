package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func write(t *testing.T, dir, name, text string) string {
	p := filepath.Join(dir, name)
	if err := os.WriteFile(p, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
	return p
}

func TestBuild(t *testing.T) {
	dir := t.TempDir()
	wasm := write(t, dir, "m.wasm", "module")
	census := `"census": {"status": [{"id": "a", "status": "supported"}, {"id": "b", "status": "supported"}, {"id": "c", "status": "unsupported"}]}`
	lib := write(t, dir, "lib.json", `{"gscouter_commit": "g", "only": ["library", "probes"], "results": [
		{"team": "a", "variant": "library", "verdict": "pass", "iterations": 1000, "metrics": [{"name": "team", "gcsim": 200, "gse": 199, "diff": -0.005, "diff_se": 0.002}]},
		{"team": "b", "variant": "library", "verdict": "pass"},
		{"team": "a", "variant": "probe:x", "verdict": "fail"}], `+census+`}`)
	rest := write(t, dir, "rest.json", `{"gscouter_commit": "g", "only": ["variants", "buildspace", "pages"], "results": [
		{"team": "a", "variant": "er-0", "verdict": "pass"},
		{"team": "a", "variant": "cons:x=C2", "verdict": "pass", "build": {"axis": "cons"}},
		{"team": "a", "variant": "cons:x=C3", "verdict": "unsupported", "error": "unsupported: constellation 3 of x", "build": {"axis": "cons"}},
		{"team": "a", "variant": "page:once", "verdict": "pass", "iterations": 2000, "metrics": [{"name": "team", "gcsim": 100, "gse": 100.5, "diff": 0.005, "diff_se": 0.001}, {"name": "x", "gcsim": 10, "gse": 9.9, "diff": -0.01, "diff_se": 0.004}]},
		{"team": "a", "variant": "page:looped:1.0", "verdict": "unsupported", "error": "unsupported: y"},
		{"team": "b", "variant": "er-0", "verdict": "pass"},
		{"team": "b", "variant": "page:once", "verdict": "inconclusive"}], `+census+`}`)
	m, err := build(wasm, "e", kinds, []string{lib, rest})
	if err != nil {
		t.Fatal(err)
	}
	if m.Supported != 2 || m.Verified != 1 {
		t.Fatalf("supported %d verified %d", m.Supported, m.Verified)
	}
	a := m.Teams["a"]
	if !a.Verified || len(a.Unsupported) != 2 || a.Builds["pages"] != "1/1" || a.Builds["buildspace"] != "1/1" {
		t.Fatalf("a: %+v", a)
	}
	b := m.Teams["b"]
	if b.Verified || len(b.Why) != 1 || b.Why[0] != "page:once: inconclusive" {
		t.Fatalf("b: %+v", b)
	}
	if len(a.Scores) != 2 || a.Scores[0].Build != "library" || a.Scores[1].WorstChar != "x" || a.Scores[1].Half95 != 1.96*0.001 {
		t.Fatalf("a's scores: %+v", a.Scores)
	}
	table := approvalTable(m)
	if a.Group != "A" {
		t.Errorf("a in group %s: %v", a.Group, a.GroupWhy)
	}
	for _, want := range []string{"1 of 2 supported teams verified", "| a | A | library | 1000 | 200.0 | 199.0 | -0.50% | 0.39% |", "|  |  | page:once | 2000 | 100.0 | 100.5 | +0.50% | 0.20% | x -1.00% |"} {
		if !strings.Contains(table, want) {
			t.Errorf("approval table lacks %q:\n%s", want, table)
		}
	}
	if strings.Contains(table, "| b |") {
		t.Error("an unverified team in the approval table")
	}
	if _, err := build(wasm, "e", kinds, []string{lib}); err == nil {
		t.Fatal("a manifest from the library run alone")
	}
	other := write(t, dir, "other.json", `{"gscouter_commit": "h", "only": ["pages"], "results": []}`)
	if _, err := build(wasm, "e", kinds, []string{lib, rest, other}); err == nil {
		t.Fatal("reports of two Gscouter commits")
	}
}

// Minor characters are shown apart from the judged ones; settled
// verdicts and differences above groupALimit put a team in group B.
func TestGroups(t *testing.T) {
	dir := t.TempDir()
	wasm := write(t, dir, "m.wasm", "module")
	census := `"census": {"status": [{"id": "a", "status": "supported"}, {"id": "b", "status": "supported"}, {"id": "c", "status": "supported"}, {"id": "d", "status": "supported"}]}`
	once := func(team string, diff float64) string {
		return fmt.Sprintf(`{"team": %q, "variant": "page:once", "verdict": "pass", "iterations": 4000, "metrics": [{"name": "team", "gcsim": 100, "gse": %v, "diff": %v, "diff_se": 0.001}]}`, team, 100*(1+diff), diff)
	}
	lib := write(t, dir, "lib.json", `{"gscouter_commit": "g", "only": ["library", "variants", "buildspace", "pages"], "results": [
		{"team": "a", "variant": "library", "verdict": "pass", "iterations": 4000, "metrics": [{"name": "team", "gcsim": 200, "gse": 200.2, "diff": 0.001, "diff_se": 0.001}, {"name": "x", "gcsim": 150, "gse": 150.3, "diff": 0.002, "diff_se": 0.002}, {"name": "y", "gcsim": 4, "gse": 3.8, "diff": -0.05, "diff_se": 0.02, "minor": true, "settled": true}]},
		{"team": "a", "variant": "er-0", "verdict": "pass"},
		`+once("a", 0.002)+`,
		{"team": "b", "variant": "library", "verdict": "pass", "iterations": 4000, "metrics": [{"name": "team", "gcsim": 200, "gse": 201.6, "diff": 0.008, "diff_se": 0.001}]},
		{"team": "b", "variant": "er-0", "verdict": "pass"},
		`+once("b", 0.001)+`,
		{"team": "c", "variant": "library", "verdict": "pass", "iterations": 32000, "metrics": [{"name": "team", "gcsim": 200, "gse": 200, "diff": 0, "diff_se": 0.001}]},
		{"team": "c", "variant": "er-0", "verdict": "pass", "metrics": [{"name": "team", "diff": 0.004, "diff_se": 0.003, "settled": true}]},
		`+once("c", 0.001)+`,
		{"team": "d", "variant": "library", "verdict": "fail"}], `+census+`}`)
	m, err := build(wasm, "e", kinds, []string{lib})
	if err != nil {
		t.Fatal(err)
	}
	a := m.Teams["a"]
	if a.Group != "A" || a.Settled != 0 || a.Scores[0].WorstChar != "x" || a.Scores[0].WorstMinorChar != "y" || a.Scores[0].Settled {
		t.Errorf("a: %+v", a)
	}
	if b := m.Teams["b"]; b.Group != "B" || len(b.GroupWhy) != 1 || b.GroupWhy[0] != "library +0.80%" {
		t.Errorf("b: %+v", b)
	}
	if c := m.Teams["c"]; c.Group != "B" || c.Settled != 1 {
		t.Errorf("c: %+v", c)
	}
	if d := m.Teams["d"]; d.Group != "" {
		t.Errorf("d, unverified, in group %s", d.Group)
	}
	sum := approvalSummary(m, nil)
	for _, want := range []string{"Group A to answer: 1**", "Group B to answer: 2**", "| fail | 1 |", "| b | B: library +0.80% |", "| c | B: 1 settled |"} {
		if !strings.Contains(sum, want) {
			t.Errorf("summary lacks %q:\n%s", want, sum)
		}
	}
	if strings.Contains(sum, "| a |") {
		t.Error("a group A team in the summary's group B table")
	}
	full := approvalTable(m)
	if ia, ib := strings.Index(full, "| a |"), strings.Index(full, "| b |"); ia < 0 || ib < ia || !strings.Contains(full, "x +0.20% | y -5.00% |") {
		t.Errorf("approval.md: group A first:\n%s", full)
	}
	cand := candidateList(m, nil)
	if !strings.HasSuffix(cand, "\na\n") || strings.Contains(cand, "\nb\n") || !strings.Contains(cand, "NOT APPROVED") {
		t.Errorf("candidates:\n%s", cand)
	}
	if m.DriftByKind["library"].Builds != 3 || m.GroupALimit != groupALimit {
		t.Errorf("drift by kind %+v, limit %v", m.DriftByKind, m.GroupALimit)
	}
}

// Teams approved before are carried forward while they hold, listed as
// dropped when not, and left out of the groups to answer.
func TestCarryForward(t *testing.T) {
	m := &Manifest{Engine: Engine{WasmSHA256: "fedcba9876543210" + strings.Repeat("0", 48)}, Verified: 3, Supported: 4, Teams: map[string]Team{
		"kept":  {Verified: true, Group: "A"},
		"left":  {Verified: true, Group: "B", GroupWhy: []string{"1 settled"}},
		"b-now": {Verified: true, Group: "B"},
		"gone":  {Why: []string{"library: fail"}},
		"new":   {Verified: true, Group: "A"},
	}}
	approved, err := parseApproved("# x\nkept 2026-11-04\nleft 2026-11-04\nb-now 2026-11-04 release=fedcba9876543210\ngone 2026-11-04\n")
	if err != nil {
		t.Fatal(err)
	}
	sum := approvalSummary(m, approved)
	for _, want := range []string{"Approved before: 4. Still holding", "with no new answer: 2. **Dropped: 2**", "**Group A to answer: 1**", "**Group B to answer: 0**", "| left | 2026-11-04 | left group A: 1 settled |", "| gone | 2026-11-04 | not verified: library: fail |"} {
		if !strings.Contains(sum, want) {
			t.Errorf("summary lacks %q:\n%s", want, sum)
		}
	}
	if cand := candidateList(m, approved); !strings.HasSuffix(cand, "\nnew\n") || strings.Contains(cand, "kept") {
		t.Errorf("candidates:\n%s", cand)
	}
}

// A library-only release verifies a team on its library build alone, and
// groups it without a one-pass score.
func TestLibraryOnly(t *testing.T) {
	dir := t.TempDir()
	wasm := write(t, dir, "m.wasm", "module")
	census := `"census": {"status": [{"id": "a", "status": "supported"}, {"id": "b", "status": "supported"}]}`
	lib := write(t, dir, "lib.json", `{"gscouter_commit": "g", "only": ["library", "probes"], "results": [
		{"team": "a", "variant": "library", "verdict": "pass", "iterations": 4000, "metrics": [{"name": "team", "gcsim": 200, "gse": 200.4, "diff": 0.002, "diff_se": 0.001}]},
		{"team": "a", "variant": "probe:x", "verdict": "fail"},
		{"team": "b", "variant": "library", "verdict": "fail", "metrics": [{"name": "team", "diff": 0.02, "diff_se": 0.002}]}], `+census+`}`)
	if _, err := build(wasm, "e", kinds, []string{lib}); err == nil {
		t.Fatal("a manifest of every kind from the library run alone")
	}
	need, err := parseKinds("library")
	if err != nil {
		t.Fatal(err)
	}
	m, err := build(wasm, "e", need, []string{lib})
	if err != nil {
		t.Fatal(err)
	}
	a := m.Teams["a"]
	if !a.Verified || a.Group != "A" || len(a.VerifiedKinds) != 1 || a.VerifiedKinds[0] != "library" || len(a.Builds) != 1 {
		t.Errorf("a: %+v", a)
	}
	if m.Teams["b"].Verified || m.Verified != 1 || len(m.Kinds) != 1 {
		t.Errorf("b: %+v, verified %d, kinds %v", m.Teams["b"], m.Verified, m.Kinds)
	}
	sum := approvalSummary(m, nil)
	for _, want := range []string{"Kinds judged: library. Verification is the library build and the probes", "No one-pass page score", "**Group A to answer: 1**"} {
		if !strings.Contains(sum, want) {
			t.Errorf("summary lacks %q:\n%s", want, sum)
		}
	}
	if _, err := parseKinds("library,nope"); err == nil {
		t.Error("an unknown kind")
	}
}
