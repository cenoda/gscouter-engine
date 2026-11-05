// Command parsecheck checks the engine's parse API (gse --parse, engine
// src/parse.rs) against gcsim's parser at the pinned commit: every config
// given is parsed by both, and every field Gscouter's input path reads
// (party order, levels, ascension, constellations, talents, weapons,
// refinements, sets, stats, params, the active character, targets, energy,
// options and gcsim's non-fatal Errors) is compared. Malformed fixtures
// (malformed/*.txt, each with a first line "# expect: <class>" or "#
// expect: ok [error classes]") also check the engine's error class. A
// fixture expecting "unsupported" declares a form gcsim reads and the
// engine refuses by name; it is reported on stderr, not as a difference.
//
//	go run ./parsecheck -gse ../engine/target/release/gse [-name build.txt] <file or dir>...
//
// A directory is walked for *.txt (only files called -name, if given). It
// prints one line per difference and a summary, and exits with 1 if any.
package main

import (
	"bufio"
	"encoding/json"
	"flag"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"sort"
	"strings"
	"sync"

	"github.com/genshinsim/gcsim/pkg/catalog"
	"github.com/genshinsim/gcsim/pkg/core/attributes"
	"github.com/genshinsim/gcsim/pkg/core/info"
	"github.com/genshinsim/gcsim/pkg/core/keys"
	"github.com/genshinsim/gcsim/pkg/core/player/character"
	"github.com/genshinsim/gcsim/pkg/gcs/ast"
	"github.com/genshinsim/gcsim/pkg/gcs/parser"
)

// The engine's JSON (parse_version 1).
type engineWeapon struct {
	Key      string         `json:"key"`
	Level    int            `json:"level"`
	MaxLevel int            `json:"max_level"`
	Refine   int            `json:"refine"`
	Params   map[string]int `json:"params"`
}

type engineSet struct {
	Key    string         `json:"key"`
	Count  int            `json:"count"`
	Params map[string]int `json:"params"`
}

type engineChar struct {
	Key       string             `json:"key"`
	Level     int                `json:"level"`
	MaxLevel  int                `json:"max_level"`
	Ascension *int               `json:"ascension"`
	Cons      int                `json:"cons"`
	Talents   [3]int             `json:"talents"`
	Params    map[string]int     `json:"params"`
	Weapon    *engineWeapon      `json:"weapon"`
	Sets      []engineSet        `json:"sets"`
	Stats     map[string]float64 `json:"stats"`
}

type engineTarget struct {
	Level  int                `json:"level"`
	Resist map[string]float64 `json:"resist"`
	HP     float64            `json:"hp"`
	Pos    [2]float64         `json:"pos"`
	Radius float64            `json:"radius"`
}

type engineParse struct {
	ParseVersion int            `json:"parse_version"`
	OK           bool           `json:"ok"`
	Characters   []engineChar   `json:"characters"`
	Active       *string        `json:"active"`
	Targets      []engineTarget `json:"targets"`
	Energy       *struct {
		Mode   string `json:"mode"`
		Start  int    `json:"start"`
		End    int    `json:"end"`
		Amount int    `json:"amount"`
	} `json:"energy"`
	Options struct {
		Iterations        int     `json:"iterations"`
		Duration          float64 `json:"duration"`
		SwapDelay         int     `json:"swap_delay"`
		Hitlag            bool    `json:"hitlag"`
		Defhalt           bool    `json:"defhalt"`
		IgnoreBurstEnergy bool    `json:"ignore_burst_energy"`
		DamageMode        bool    `json:"damage_mode"`
	} `json:"options"`
	Hurt   bool `json:"hurt"`
	Errors []struct {
		Class   string `json:"class"`
		Message string `json:"message"`
	} `json:"errors"`
	Error *struct {
		Class   string `json:"class"`
		Line    *int   `json:"line"`
		Message string `json:"message"`
	} `json:"error"`
}

// gcsim's element names by attributes.Element, as the engine writes them.
var elements = []string{"none", "pyro", "hydro", "dendro", "electro", "cryo", "anemo", "geo", "physical", "frozen", "quicken"}

func main() {
	gse := flag.String("gse", "../engine/target/release/gse", "the engine's command")
	name := flag.String("name", "", "when walking a directory, only files of this name")
	flag.Parse()
	var files []string
	for _, a := range flag.Args() {
		st, err := os.Stat(a)
		if err != nil {
			fail(err)
		}
		if !st.IsDir() {
			files = append(files, a)
			continue
		}
		err = filepath.WalkDir(a, func(p string, d fs.DirEntry, err error) error {
			if err != nil {
				return err
			}
			if !d.IsDir() && strings.HasSuffix(p, ".txt") && (*name == "" || d.Name() == *name) {
				files = append(files, p)
			}
			return nil
		})
		if err != nil {
			fail(err)
		}
	}
	sort.Strings(files)

	type outcome struct {
		diffs  []string
		failed bool // both refused the config, or a declared difference
	}
	res := make([]outcome, len(files))
	var wg sync.WaitGroup
	sem := make(chan struct{}, 8)
	for i, f := range files {
		wg.Add(1)
		sem <- struct{}{}
		go func() {
			defer wg.Done()
			defer func() { <-sem }()
			d, failed := check(*gse, f)
			res[i] = outcome{d, failed}
		}()
	}
	wg.Wait()
	n, bad, refused := 0, 0, 0
	w := bufio.NewWriter(os.Stdout)
	for i, r := range res {
		n++
		if r.failed {
			refused++
		}
		if len(r.diffs) > 0 {
			bad++
			for _, d := range r.diffs {
				fmt.Fprintf(w, "%s: %s\n", files[i], d)
			}
		}
	}
	fmt.Fprintf(w, "%d configs: %d agree, %d differ (%d refused by both or declared)\n", n, n-bad, bad, refused)
	w.Flush()
	if bad > 0 {
		os.Exit(1)
	}
}

func fail(err error) {
	fmt.Fprintln(os.Stderr, err)
	os.Exit(2)
}

// expectation reads a fixture's "# expect:" line: the error class, or "ok"
// and the classes of the non-fatal errors.
func expectation(src string) (class string, errs []string, ok bool) {
	first, _, _ := strings.Cut(src, "\n")
	rest, found := strings.CutPrefix(strings.TrimSpace(first), "# expect:")
	if !found {
		return "", nil, false
	}
	f := strings.Fields(rest)
	if len(f) == 0 {
		return "", nil, false
	}
	if len(f) > 1 {
		errs = strings.Split(f[1], ",")
	}
	return f[0], errs, true
}

func check(gse, file string) (diffs []string, bothFailed bool) {
	add := func(format string, a ...any) { diffs = append(diffs, fmt.Sprintf(format, a...)) }
	b, err := os.ReadFile(file)
	if err != nil {
		return []string{err.Error()}, false
	}
	src := string(b)
	out, _ := exec.Command(gse, "--parse", file).Output()
	var e engineParse
	if err := json.Unmarshal(out, &e); err != nil {
		return []string{fmt.Sprintf("engine output is not JSON: %v: %.200s", err, out)}, false
	}
	if e.ParseVersion != 1 {
		add("parse_version %d", e.ParseVersion)
	}
	g, gerr := gcsimParse(src)

	class, errs, expect := expectation(src)
	if expect {
		got := "ok"
		if !e.OK && e.Error != nil {
			got = e.Error.Class
		}
		if got != class {
			add("expected %s, engine gives %s", class, got)
		}
		if e.OK {
			var classes []string
			for _, x := range e.Errors {
				classes = append(classes, x.Class)
			}
			if !slices.Equal(classes, errs) {
				add("expected errors %v, engine gives %v", errs, classes)
			}
		}
	}

	switch {
	case gerr != nil && !e.OK:
		return diffs, true
	case gerr != nil:
		add("gcsim refuses (%v), engine parses", gerr)
		return diffs, false
	case !e.OK && expect && class == "unsupported" && e.Error != nil && e.Error.Class == "unsupported":
		// a form gcsim reads and the engine names as unsupported: a
		// declared difference, as a fixture says
		fmt.Fprintf(os.Stderr, "%s: declared: gcsim parses, the engine refuses (%s)\n", file, e.Error.Message)
		return diffs, true
	case !e.OK:
		msg := "?"
		if e.Error != nil {
			msg = e.Error.Class + ": " + e.Error.Message
		}
		add("engine refuses (%s), gcsim parses", msg)
		return diffs, false
	}

	// non-fatal errors, by message
	var em []string
	for _, x := range e.Errors {
		em = append(em, x.Message)
	}
	if !slices.Equal(em, g.ErrorMsgs) && !(len(em) == 0 && len(g.ErrorMsgs) == 0) {
		add("errors: engine %q, gcsim %q", em, g.ErrorMsgs)
	}

	if len(e.Characters) != len(g.Characters) {
		add("characters: engine %d, gcsim %d", len(e.Characters), len(g.Characters))
	} else {
		for i := range g.Characters {
			compareChar(add, i, &e.Characters[i], &g.Characters[i])
		}
	}

	active := ""
	if e.Active != nil {
		active = *e.Active
	}
	gactive := ""
	if g.InitialChar != keys.NoChar {
		gactive = g.InitialChar.String()
	}
	if active != gactive {
		add("active: engine %q, gcsim %q", active, gactive)
	}

	if len(e.Targets) != len(g.Targets) {
		add("targets: engine %d, gcsim %d", len(e.Targets), len(g.Targets))
	} else {
		for i, gt := range g.Targets {
			et := e.Targets[i]
			if et.Level != gt.Level || et.HP != gt.HP || et.Pos[0] != gt.Pos.X || et.Pos[1] != gt.Pos.Y || et.Radius != gt.Pos.R {
				add("target %d: engine lvl %d hp %v pos %v r %v, gcsim lvl %d hp %v pos %v,%v r %v", i, et.Level, et.HP, et.Pos, et.Radius, gt.Level, gt.HP, gt.Pos.X, gt.Pos.Y, gt.Pos.R)
			}
			gr := map[string]float64{}
			for el, v := range gt.Resist {
				if v != 0 {
					gr[elements[el]] = v
				}
			}
			if !mapsEqual(et.Resist, gr) {
				add("target %d resist: engine %v, gcsim %v", i, et.Resist, gr)
			}
		}
	}

	ge := g.EnergySettings
	if (e.Energy != nil) != ge.Active {
		add("energy: engine %v, gcsim active %v", e.Energy != nil, ge.Active)
	} else if e.Energy != nil {
		mode := "every"
		if ge.Once {
			mode = "once"
		}
		if e.Energy.Mode != mode || e.Energy.Start != ge.Start || e.Energy.End != ge.End || e.Energy.Amount != ge.Amount {
			add("energy: engine %+v, gcsim %+v", *e.Energy, ge)
		}
	}
	if e.Hurt != g.HurtSettings.Active {
		add("hurt: engine %v, gcsim %v", e.Hurt, g.HurtSettings.Active)
	}
	o, s := e.Options, g.Settings
	if o.Iterations != s.Iterations || o.Duration != s.Duration || o.SwapDelay != s.Delays.Swap || o.Hitlag != s.EnableHitlag || o.Defhalt != s.DefHalt || o.IgnoreBurstEnergy != s.IgnoreBurstEnergy || o.DamageMode != s.DamageMode {
		add("options: engine %+v, gcsim iter %d dur %v swap %d hitlag %v defhalt %v ibe %v dmgmode %v", o, s.Iterations, s.Duration, s.Delays.Swap, s.EnableHitlag, s.DefHalt, s.IgnoreBurstEnergy, s.DamageMode)
	}
	return diffs, false
}

// gcsimParse is gcsim's parse, with a panic as an error.
func gcsimParse(src string) (res *info.ActionList, err error) {
	defer func() {
		if r := recover(); r != nil {
			err = fmt.Errorf("gcsim's parser panicked: %v", r)
		}
	}()
	res, _, err = parser.New(ast.NewFile(), src).Parse()
	return res, err
}

func compareChar(add func(string, ...any), i int, e *engineChar, g *info.CharacterProfile) {
	key := g.Base.Key.String()
	if e.Key != key {
		add("character %d: engine %s, gcsim %s", i, e.Key, key)
		return
	}
	if e.Level != g.Base.Level || e.MaxLevel != g.Base.MaxLevel || e.Cons != g.Base.Cons {
		add("%s: engine lvl %d/%d c%d, gcsim lvl %d/%d c%d", key, e.Level, e.MaxLevel, e.Cons, g.Base.Level, g.Base.MaxLevel, g.Base.Cons)
	}
	if e.Talents != [3]int{g.Talents.Attack, g.Talents.Skill, g.Talents.Burst} {
		add("%s talents: engine %v, gcsim %v", key, e.Talents, g.Talents)
	}
	if d := catalog.CharacterMap[g.Base.Key]; d != nil {
		asc := character.AvatarAsc(g.Base.MaxLevel, d)
		if e.Ascension == nil || *e.Ascension != asc {
			add("%s ascension: engine %v, gcsim %d", key, e.Ascension, asc)
		}
	} else if e.Ascension != nil {
		add("%s ascension: engine %d, gcsim has no data", key, *e.Ascension)
	}
	if !intMapsEqual(e.Params, g.Params) {
		add("%s params: engine %v, gcsim %v", key, e.Params, g.Params)
	}
	if g.Weapon.Key == keys.NoWeapon {
		if e.Weapon != nil {
			add("%s weapon: engine %s, gcsim none", key, e.Weapon.Key)
		}
	} else if e.Weapon == nil {
		add("%s weapon: engine none, gcsim %s", key, g.Weapon.Key)
	} else {
		w, gw := e.Weapon, g.Weapon
		if w.Key != gw.Key.String() || w.Level != gw.Level || w.MaxLevel != gw.MaxLevel || w.Refine != gw.Refine {
			add("%s weapon: engine %s %d/%d r%d, gcsim %s %d/%d r%d", key, w.Key, w.Level, w.MaxLevel, w.Refine, gw.Key, gw.Level, gw.MaxLevel, gw.Refine)
		}
		if !intMapsEqual(w.Params, gw.Params) {
			add("%s weapon params: engine %v, gcsim %v", key, w.Params, gw.Params)
		}
	}
	es := map[string]int{}
	for _, s := range e.Sets {
		es[s.Key] = s.Count
		var gp map[string]int
		for k, v := range g.SetParams {
			if k.String() == s.Key {
				gp = v
			}
		}
		if !intMapsEqual(s.Params, gp) {
			add("%s set %s params: engine %v, gcsim %v", key, s.Key, s.Params, gp)
		}
	}
	gs := map[string]int{}
	for k, v := range g.Sets {
		gs[k.String()] = v
	}
	if !intMapsEqual(es, gs) {
		add("%s sets: engine %v, gcsim %v", key, es, gs)
	}
	gst := map[string]float64{}
	for j, v := range g.Stats {
		if v != 0 && j != int(attributes.NoStat) {
			gst[attributes.Stat(j).String()] = v
		}
	}
	if !mapsEqual(e.Stats, gst) {
		add("%s stats: engine %v, gcsim %v", key, e.Stats, gst)
	}
}

func mapsEqual(a, b map[string]float64) bool {
	if len(a) != len(b) {
		return false
	}
	for k, v := range a {
		if w, ok := b[k]; !ok || w != v {
			return false
		}
	}
	return true
}

func intMapsEqual(a, b map[string]int) bool {
	if len(a) != len(b) {
		return false
	}
	for k, v := range a {
		if w, ok := b[k]; !ok || w != v {
			return false
		}
	}
	return true
}
