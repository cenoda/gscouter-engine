package main

import (
	"os"
	"path/filepath"
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
		{"team": "a", "variant": "library", "verdict": "pass"},
		{"team": "b", "variant": "library", "verdict": "pass"},
		{"team": "a", "variant": "probe:x", "verdict": "fail"}], `+census+`}`)
	rest := write(t, dir, "rest.json", `{"gscouter_commit": "g", "only": ["variants", "buildspace", "pages"], "results": [
		{"team": "a", "variant": "er-0", "verdict": "pass"},
		{"team": "a", "variant": "cons:x=C2", "verdict": "pass", "build": {"axis": "cons"}},
		{"team": "a", "variant": "cons:x=C3", "verdict": "unsupported", "error": "unsupported: constellation 3 of x", "build": {"axis": "cons"}},
		{"team": "a", "variant": "page:once", "verdict": "pass"},
		{"team": "a", "variant": "page:looped:1.0", "verdict": "unsupported", "error": "unsupported: y"},
		{"team": "b", "variant": "er-0", "verdict": "pass"},
		{"team": "b", "variant": "page:once", "verdict": "inconclusive"}], `+census+`}`)
	m, err := build(wasm, "e", []string{lib, rest})
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
	if _, err := build(wasm, "e", []string{lib}); err == nil {
		t.Fatal("a manifest from the library run alone")
	}
	other := write(t, dir, "other.json", `{"gscouter_commit": "h", "only": ["pages"], "results": []}`)
	if _, err := build(wasm, "e", []string{lib, rest, other}); err == nil {
		t.Fatal("reports of two Gscouter commits")
	}
}
