package main

import (
	"slices"
	"testing"
)

var testResolver = resolver{
	chars:   map[string]string{"arle": "arlecchino", "arlecchino": "arlecchino", "bennett": "bennett", "tartaglia": "tartaglia"},
	weapons: map[string]string{"fjords": "balladofthefjords"},
	sets:    map[string]string{"gt": "gildeddreams"},
}

func TestMembersAndReaches(t *testing.T) {
	build := "arle char lvl=90/90 cons=2 talent=9,9,9;\narle add weapon=\"fjords\" refine=5 lvl=90/90;\narle add set =\"gt\" count=4;\nbennett char lvl=90/90 cons=6;\nbennett add weapon= \"x\" lvl=90/90 +params=[stacks=5];\n"
	ms, err := testResolver.members(build)
	if err != nil {
		t.Fatal(err)
	}
	got := append(ms[0].reaches(), ms[1].reaches()...)
	want := []string{
		"char:arlecchino:c1", "char:arlecchino:c2", "weapon:balladofthefjords:r5", "set:gildeddreams:2", "set:gildeddreams:4",
		"char:bennett:c1", "char:bennett:c2", "char:bennett:c3", "char:bennett:c4", "char:bennett:c5", "char:bennett:c6", "weapon:x:r1",
	}
	if !slices.Equal(got, want) {
		t.Errorf("reaches %v, want %v", got, want)
	}
}

func TestAddedActions(t *testing.T) {
	rot := "while 1 {\n  arlecchino skill;\n  tartaglia walk, burst,\n    skill;\n}\n"
	cases := []struct {
		probe string
		want  []string
	}{
		{"team t\nrotation- arlecchino skill;\nrotation+ arlecchino skill;\nrotation+ if .arlecchino.energy >= 60 { arle burst; }\n", []string{"arlecchino burst"}},
		{"team t\nrotation- skill;\nrotation+ skill[hold=1, x=2], attack:3;\n", []string{"tartaglia skill[hold=1,x=2]", "tartaglia attack:3"}},
		{"team t\nrotation- tartaglia walk, burst,\nrotation+ tartaglia burst, walk,\n", nil},
	}
	for _, c := range cases {
		pr, err := parseProbe("p", c.probe)
		if err != nil {
			t.Fatal(err)
		}
		got, err := testResolver.addedActions(pr, rot)
		if err != nil {
			t.Fatal(err)
		}
		if !slices.Equal(got, c.want) {
			t.Errorf("%q: %v, want %v", c.probe, got, c.want)
		}
	}
	pr, _ := parseProbe("p", "team t\nrotation- burst,\nrotation+ x\n")
	if _, err := testResolver.addedActions(pr, rot+rot); err == nil {
		t.Error("a hunk matching two places is not an error")
	}
}

func TestApplyNamesChanged(t *testing.T) {
	pr, err := parseProbe("p", "team t\narle add set=\"gt\" count=2;\narle add set=\"x\" count=2;\n")
	if err != nil {
		t.Fatal(err)
	}
	out, changed, err := pr.apply("arle char cons=0;\narle add set =\"gt\" count=4;\nbennett char cons=6;\n")
	if err != nil {
		t.Fatal(err)
	}
	if !slices.Equal(changed, []string{"arle"}) || out != "arle char cons=0;\narle add set=\"gt\" count=2;\narle add set=\"x\" count=2;\nbennett char cons=6;\n" {
		t.Errorf("apply: %q %v", out, changed)
	}
	pr, _ = parseProbe("p", "team t\nfoo add stats hp=1;\n")
	if _, _, err := pr.apply("arle char cons=0;\n"); err == nil {
		t.Error("a line with nothing to replace is not an error")
	}
}
