package main

import (
	"strings"
	"testing"
)

const testBuild = `# Build from the source config.

hutao char lvl=90/90 cons=1 talent=10,10,10;
hutao add weapon="staffofhoma" refine=1 lvl=90/90;
hutao add set="shimenawasreminiscence" count=4;
hutao add stats hp=4780 cr=0.7;

xingqiu char lvl=90/90 cons=6 talent=9,9,10;
xingqiu add weapon="sacrificialsword" lvl=90/90 refine=3 +params=[stacks=5];
xingqiu add stats atk=311;
`

func TestBuildSpace(t *testing.T) {
	has := map[string]bool{"dragonsbane": true, "favoniuslance": true, "balladofthefjords": true, "favoniussword": true, "lionsroar": true, "harbingerofdawn": true}
	impl := func(w string) (bool, error) { return has[w], nil }
	bs, err := buildSpace("team", testBuild, 2, impl)
	if err != nil {
		t.Fatal(err)
	}
	byName := map[string]bsBuild{}
	count := map[string]int{}
	for _, b := range bs {
		byName[b.v.Name] = b
		count[b.v.change.Axis]++
		// one line changes, nothing else
		old, new := strings.Split(testBuild, "\n"), strings.Split(b.build, "\n")
		if len(old) != len(new) {
			t.Fatalf("%s: lines %d, want %d", b.v.Name, len(new), len(old))
		}
		diff := 0
		for i := range old {
			if old[i] != new[i] {
				diff++
			}
		}
		if diff != 1 {
			t.Errorf("%s: %d lines changed, want 1", b.v.Name, diff)
		}
	}
	// hutao C0, C2-C6 and xingqiu C0-C5; refine: hutao R5, xingqiu R1 and
	// R5; weapons: two per member
	if count["cons"] != 12 || count["refine"] != 3 || count["weapon"] != 4 {
		t.Errorf("counts %v", count)
	}
	line := func(name, prefix string) string {
		b, ok := byName[name]
		if !ok {
			t.Fatalf("no build %s", name)
		}
		for _, l := range strings.Split(b.build, "\n") {
			if strings.HasPrefix(l, prefix) {
				return l
			}
		}
		return ""
	}
	if l := line("cons:hutao=C6", "hutao char"); l != "hutao char lvl=90/90 cons=6 talent=10,10,10;" {
		t.Errorf("cons: %q", l)
	}
	if l := line("refine:xingqiu=R1", "xingqiu add weapon"); l != `xingqiu add weapon="sacrificialsword" lvl=90/90 refine=1 +params=[stacks=5];` {
		t.Errorf("refine: %q", l)
	}
	for _, b := range bs {
		if b.v.change.Axis != "weapon" {
			continue
		}
		if !has[b.v.change.To] {
			t.Errorf("%s: weapon the engine lacks", b.v.Name)
		}
		if b.v.change.Char == "xingqiu" {
			want := `xingqiu add weapon="` + b.v.change.To + `" refine=3 lvl=90/90;`
			if l := line(b.v.Name, "xingqiu add weapon"); l != want {
				t.Errorf("weapon: %q, want %q", l, want)
			}
			if !strings.Contains(b.v.Desc, "params dropped") {
				t.Errorf("%s: desc %q", b.v.Name, b.v.Desc)
			}
		}
		if b.v.change.Char == "hutao" && !strings.Contains("dragonsbane favoniuslance balladofthefjords", b.v.change.To) {
			t.Errorf("%s: not a polearm", b.v.Name)
		}
	}
	// the same team gives the same builds
	again, _ := buildSpace("team", testBuild, 2, impl)
	for i := range bs {
		if again[i].v.Name != bs[i].v.Name {
			t.Fatalf("not deterministic: %s against %s", again[i].v.Name, bs[i].v.Name)
		}
	}
	all, _ := buildSpace("team", testBuild, 0, impl)
	n := 0
	for _, b := range all {
		if b.v.change.Axis == "weapon" {
			n++
		}
	}
	if n != 6 {
		t.Errorf("-buildspace-weapons 0: %d weapon builds, want 6", n)
	}
}

func TestBuildSpaceDefaults(t *testing.T) {
	// no cons= and no refine=: C0 and R1, written out when changed
	b := "bennett char lvl=90/90 talent=9,9,9;\nbennett add weapon=\"favoniussword\" lvl=90/90;\n"
	bs, err := buildSpace("t", b, 0, func(string) (bool, error) { return false, nil })
	if err != nil {
		t.Fatal(err)
	}
	if len(bs) != 7 {
		t.Fatalf("%d builds, want 6 cons and R5", len(bs))
	}
	if !strings.Contains(bs[0].build, "talent=9,9,9 cons=1;") {
		t.Errorf("cons added: %q", bs[0].build)
	}
	if !strings.Contains(bs[6].build, `lvl=90/90 refine=5;`) {
		t.Errorf("refine added: %q", bs[6].build)
	}
}
