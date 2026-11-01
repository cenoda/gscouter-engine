package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestProbeRotation(t *testing.T) {
	rot := "active a;\nfor let x=0; x<4; x=x+1 {\n  a skill,\n     attack:3, charge,\n     attack:3, charge;\n  b burst;\n  b burst;\n}\n"
	cases := []struct {
		name, probe, want, err string
	}{
		{
			name:  "replace",
			probe: "team t\nrotation- a skill,\nrotation- attack:3, charge,\nrotation+ a skill,\nrotation+ attack:5, charge,\n",
			want:  "active a;\nfor let x=0; x<4; x=x+1 {\n  a skill,\n  attack:5, charge,\n     attack:3, charge;\n  b burst;\n  b burst;\n}\n",
		},
		{
			name:  "two hunks and a deletion",
			probe: "team t\nrotation- active a;\nrotation+ active b;\nrotation- attack:3, charge;\nrotation- b burst;\n",
			want:  "active b;\nfor let x=0; x<4; x=x+1 {\n  a skill,\n     attack:3, charge,\n  b burst;\n}\n",
		},
		{name: "no match", probe: "team t\nrotation- a attack:5;\nrotation+ a attack;\n", err: "matches 0 places"},
		{name: "two matches", probe: "team t\nrotation- b burst;\nrotation+ b skill;\n", err: "matches 2 places"},
		{name: "partial line", probe: "team t\nrotation- a skill\nrotation+ a burst\n", err: "matches 0 places"},
	}
	for _, c := range cases {
		pr, err := parseProbe(c.name, c.probe)
		if err != nil {
			t.Fatalf("%s: %v", c.name, err)
		}
		got, err := pr.applyRotation(rot)
		switch {
		case c.err != "" && (err == nil || !strings.Contains(err.Error(), c.err)):
			t.Errorf("%s: error %v, want %q", c.name, err, c.err)
		case c.err == "" && err != nil:
			t.Errorf("%s: %v", c.name, err)
		case c.err == "" && got != c.want:
			t.Errorf("%s:\n%s\nwant:\n%s", c.name, got, c.want)
		}
	}
	for _, bad := range []string{
		"team t\nrotation+ a attack;\n",
		"rotation- a;\nrotation+ b;\n",
		"team t\n",
		"team t\nrotation a;\n",
	} {
		if _, err := parseProbe("bad", bad); err == nil {
			t.Errorf("%q: no error", bad)
		}
	}
}

// Every probe in the repository parses.
func TestProbesParse(t *testing.T) {
	ents, err := os.ReadDir("../probes")
	if err != nil {
		t.Fatal(err)
	}
	for _, e := range ents {
		if !strings.HasSuffix(e.Name(), ".txt") {
			continue
		}
		b, err := os.ReadFile(filepath.Join("../probes", e.Name()))
		if err != nil {
			t.Fatal(err)
		}
		if _, err := parseProbe(strings.TrimSuffix(e.Name(), ".txt"), string(b)); err != nil {
			t.Error(err)
		}
	}
}
