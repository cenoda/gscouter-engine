package main

// Build-space variants: builds of a supported team's library build that
// change one axis of one member, so that what a player's showcase build
// can carry is compared, not only what the library's builds happen to:
//
//   - cons: each constellation 0 to 6 other than the build's (the char
//     line's cons=; gcsim adds the +3 talent levels at C3 and C5 itself),
//   - refine: refinement 1 and 5, whichever the build does not carry,
//   - weapon: another weapon of the same class that the engine implements,
//     at the build's refinement and level. -buildspace-weapons bounds how
//     many per member: the candidates are ranked by a hash of team, member
//     and weapon (rendezvous hashing), so that a team's choice is stable,
//     changes only where a newly ported weapon ranks first, and across the
//     teams that carry a class every weapon of it is drawn.
//
// A build-space build the engine cannot run (gse --check names what it
// lacks, such as a constellation above what a character implements) is not
// run and not dropped: it is a result with the verdict "unsupported" and
// the names, counted by axis and by reason in the report, the start of the
// measure of progress over players' builds (docs/roadmap.md §2).

import (
	"fmt"
	"hash/fnv"
	"math"
	"regexp"
	"slices"
	"sort"
	"strconv"
	"strings"

	"github.com/genshinsim/gcsim/pkg/catalog"
	"github.com/genshinsim/gcsim/pkg/shortcut"
)

// BuildChange is the one axis of one member a build-space build changes.
type BuildChange struct {
	Axis string `json:"axis"` // cons, refine, weapon
	Char string `json:"char"`
	From string `json:"from"`
	To   string `json:"to"`
}

var axes = []string{"cons", "refine", "weapon"}

var (
	charLine   = regexp.MustCompile(`^\s*(\S+)\s+char\b`)
	consField  = regexp.MustCompile(`\bcons\s*=\s*(\d+)`)
	weaponLine = regexp.MustCompile(`^(\s*)(\S+)\s+add\s+weapon\s*=\s*"([^"]+)"(.*)$`)
	refineFld  = regexp.MustCompile(`\brefine\s*=\s*(\d+)`)
	lvlField   = regexp.MustCompile(`\blvl\s*=\s*([0-9]+/[0-9]+)`)
)

// member is a character of a library build: its line numbers and what the
// axes start from.
type member struct {
	name     string
	charIdx  int
	cons     int
	weapIdx  int
	weapon   string
	refine   int
	lvl      string
	weapLine string
}

func members(build string) ([]member, error) {
	lines := strings.Split(build, "\n")
	var ms []member
	at := map[string]int{}
	for i, l := range lines {
		t := strings.TrimSpace(l)
		if t == "" || strings.HasPrefix(t, "#") {
			continue
		}
		if m := charLine.FindStringSubmatch(l); m != nil {
			c := 0
			if f := consField.FindStringSubmatch(l); f != nil {
				c, _ = strconv.Atoi(f[1])
			}
			at[m[1]] = len(ms)
			ms = append(ms, member{name: m[1], charIdx: i, cons: c, weapIdx: -1})
			continue
		}
		if m := weaponLine.FindStringSubmatch(l); m != nil {
			k, ok := at[m[2]]
			if !ok {
				return nil, fmt.Errorf("weapon line before %s's char line: %q", m[2], l)
			}
			r := 1
			if f := refineFld.FindStringSubmatch(m[4]); f != nil {
				r, _ = strconv.Atoi(f[1])
			}
			lvl := "90/90"
			if f := lvlField.FindStringSubmatch(m[4]); f != nil {
				lvl = f[1]
			}
			ms[k].weapIdx, ms[k].weapon, ms[k].refine, ms[k].lvl, ms[k].weapLine = i, m[3], r, lvl, l
		}
	}
	for _, m := range ms {
		if m.weapIdx < 0 {
			return nil, fmt.Errorf("%s has no weapon line", m.name)
		}
	}
	return ms, nil
}

// bsBuild is a build-space build: its variant and its build file's text.
type bsBuild struct {
	v     variant
	build string
}

func setLine(build string, i int, l string) string {
	lines := strings.Split(build, "\n")
	lines[i] = l
	return strings.Join(lines, "\n")
}

func withCons(l string, c int) string {
	if consField.MatchString(l) {
		return consField.ReplaceAllString(l, "cons="+strconv.Itoa(c))
	}
	// default C0: add the field before the line's end
	l = strings.TrimRight(l, " ;")
	return l + " cons=" + strconv.Itoa(c) + ";"
}

func withRefine(l string, r int) string {
	if refineFld.MatchString(l) {
		return refineFld.ReplaceAllString(l, "refine="+strconv.Itoa(r))
	}
	l = strings.TrimRight(l, " ;")
	return l + " refine=" + strconv.Itoa(r) + ";"
}

// weaponClass is the class of a weapon by the name gcsl takes, and its
// rarity.
func weaponClass(name string) (int32, uint32, bool) {
	k, ok := shortcut.WeaponNameToKey[name]
	if !ok {
		return 0, 0, false
	}
	d, ok := catalog.WeaponMap[k]
	if !ok {
		return 0, 0, false
	}
	return int32(d.WeaponClass), d.Rarity, true
}

// classWeapons is every weapon key in gcsim's catalog of a class, sorted.
func classWeapons(class int32) []string {
	var out []string
	for k, d := range catalog.WeaponMap {
		if int32(d.WeaponClass) == class {
			out = append(out, k.String())
		}
	}
	sort.Strings(out)
	return out
}

func rank(team, char, weapon string) uint64 {
	h := fnv.New64a()
	fmt.Fprintf(h, "%s/%s/%s", team, char, weapon)
	return h.Sum64()
}

// buildSpace is the team's build-space builds. implemented tells whether
// the engine has a weapon.
func buildSpace(id, base string, nWeapons int, implemented func(string) (bool, error)) ([]bsBuild, error) {
	ms, err := members(base)
	if err != nil {
		return nil, fmt.Errorf("%s: %w", id, err)
	}
	var out []bsBuild
	add := func(ch BuildChange, desc, build string) {
		name := fmt.Sprintf("%s:%s=%s", ch.Axis, ch.Char, ch.To)
		c := ch
		out = append(out, bsBuild{v: variant{Name: name, Desc: desc, change: &c}, build: build})
	}
	lines := strings.Split(base, "\n")
	for _, m := range ms {
		for c := 0; c <= 6; c++ {
			if c == m.cons {
				continue
			}
			ch := BuildChange{"cons", m.name, "C" + strconv.Itoa(m.cons), "C" + strconv.Itoa(c)}
			add(ch, fmt.Sprintf("%s C%d instead of C%d", m.name, c, m.cons), setLine(base, m.charIdx, withCons(lines[m.charIdx], c)))
		}
		for _, r := range []int{1, 5} {
			if r == m.refine {
				continue
			}
			ch := BuildChange{"refine", m.name, "R" + strconv.Itoa(m.refine), "R" + strconv.Itoa(r)}
			add(ch, fmt.Sprintf("%s's %s R%d instead of R%d", m.name, m.weapon, r, m.refine), setLine(base, m.weapIdx, withRefine(lines[m.weapIdx], r)))
		}
		class, _, ok := weaponClass(m.weapon)
		if !ok {
			return nil, fmt.Errorf("%s: %s's weapon %q is not in gcsim's catalog", id, m.name, m.weapon)
		}
		cur := m.weapon
		if k, ok := shortcut.WeaponNameToKey[m.weapon]; ok {
			cur = k.String()
		}
		var cands []string
		for _, w := range classWeapons(class) {
			if w == cur {
				continue
			}
			ok, err := implemented(w)
			if err != nil {
				return nil, err
			}
			if ok {
				cands = append(cands, w)
			}
		}
		slices.SortFunc(cands, func(a, b string) int {
			ra, rb := rank(id, m.name, a), rank(id, m.name, b)
			switch {
			case ra < rb:
				return -1
			case ra > rb:
				return 1
			}
			return strings.Compare(a, b)
		})
		if nWeapons > 0 && len(cands) > nWeapons {
			cands = cands[:nWeapons]
		}
		for _, w := range cands {
			lvl := m.lvl
			if _, rar, _ := weaponClass(w); rar <= 2 {
				// one- and two-star weapons stop at 70
				lvl = "70/70"
			}
			// the old weapon's params (+params=[stacks=5]) are its own
			l := fmt.Sprintf("%s add weapon=%q refine=%d lvl=%s;", m.name, w, m.refine, lvl)
			ch := BuildChange{"weapon", m.name, cur, w}
			desc := fmt.Sprintf("%s with %s R%d instead of %s", m.name, w, m.refine, cur)
			if strings.Contains(m.weapLine, "+params") {
				desc += " (its params dropped)"
			}
			add(ch, desc, setLine(base, m.weapIdx, l))
		}
	}
	return out, nil
}

// weaponSupport tells whether the engine has a weapon, by asking gse
// --check about a supported config with one weapon line replaced; the
// answer does not depend on the team, so it is kept per weapon.
type weaponSupport struct {
	gse, tmp string
	known    map[string]bool
}

func (s *weaponSupport) implemented(cfg, line, char, w string) (bool, error) {
	if ok, seen := s.known[w]; seen {
		return ok, nil
	}
	if !strings.Contains(cfg, line) {
		return false, fmt.Errorf("weapon line %q not in the dumped config", line)
	}
	_, rar, _ := weaponClass(w)
	lvl := "90/90"
	if rar <= 2 {
		lvl = "70/70"
	}
	probe := strings.Replace(cfg, line, fmt.Sprintf("%s add weapon=%q refine=1 lvl=%s;", char, w, lvl), 1)
	miss, err := check(s.gse, s.tmp, "weapon-"+w, probe)
	if err != nil {
		return false, err
	}
	ok := !slices.Contains(miss, "unsupported: weapon "+w)
	s.known[w] = ok
	return ok, nil
}

// AxisCount is how the build-space builds of one axis went.
type AxisCount struct {
	Axis         string `json:"axis"`
	Builds       int    `json:"builds"`
	Pass         int    `json:"pass"`
	Fail         int    `json:"fail"`
	Inconclusive int    `json:"inconclusive"`
	Error        int    `json:"error"`
	Unsupported  int    `json:"unsupported"`
	// Planned is what -dry-run would run.
	Planned int `json:"planned,omitempty"`
}

// Reason is a thing the engine lacks and the build-space builds it stops.
type Reason struct {
	Name   string         `json:"name"`
	Builds int            `json:"builds"`
	ByAxis map[string]int `json:"by_axis"`
}

type BuildSpace struct {
	Axes    []AxisCount `json:"axes"`
	Reasons []Reason    `json:"unsupported_reasons"`
	// Weapons is, for each weapon the weapon axis drew, how many builds.
	Weapons map[string]int `json:"weapons"`
}

func buildSpaceSummary(rs []Result) *BuildSpace {
	counts := map[string]*AxisCount{}
	for _, a := range axes {
		counts[a] = &AxisCount{Axis: a}
	}
	reasons := map[string]*Reason{}
	bs := &BuildSpace{Weapons: map[string]int{}}
	any := false
	for _, r := range rs {
		if r.Build == nil {
			continue
		}
		any = true
		c := counts[r.Build.Axis]
		c.Builds++
		switch r.Verdict {
		case "pass":
			c.Pass++
		case "fail":
			c.Fail++
		case "inconclusive":
			c.Inconclusive++
		case "dry-run":
			c.Planned++
		case "unsupported":
			c.Unsupported++
			for _, u := range strings.Split(strings.TrimPrefix(r.Error, "unsupported: "), "; ") {
				u = strings.TrimPrefix(u, "unsupported: ")
				re := reasons[u]
				if re == nil {
					re = &Reason{Name: u, ByAxis: map[string]int{}}
					reasons[u] = re
				}
				re.Builds++
				re.ByAxis[r.Build.Axis]++
			}
		default:
			c.Error++
		}
		if r.Build.Axis == "weapon" {
			bs.Weapons[r.Build.To]++
		}
	}
	if !any {
		return nil
	}
	for _, a := range axes {
		bs.Axes = append(bs.Axes, *counts[a])
	}
	for _, re := range reasons {
		bs.Reasons = append(bs.Reasons, *re)
	}
	sort.Slice(bs.Reasons, func(i, j int) bool {
		a, b := bs.Reasons[i], bs.Reasons[j]
		if a.Builds != b.Builds {
			return a.Builds > b.Builds
		}
		return a.Name < b.Name
	})
	return bs
}

func buildSpaceMarkdown(w func(string, ...any), rep *Report) {
	bs := rep.BuildSpace
	if bs == nil {
		return
	}
	w("\n## Build space\n\nBuilds that change one axis of one member of a supported team's library build: constellation 0 to 6, refinement 1 and 5, and up to %d other weapons of the class the engine implements (0: all). A build the engine cannot run is not run and counts as unsupported, with what it lacks.\n\n", rep.BuildSpaceWeapons)
	tot := AxisCount{Axis: "all"}
	for _, a := range bs.Axes {
		tot.Builds += a.Builds
		tot.Pass += a.Pass
		tot.Fail += a.Fail
		tot.Inconclusive += a.Inconclusive
		tot.Error += a.Error
		tot.Unsupported += a.Unsupported
		tot.Planned += a.Planned
	}
	if tot.Planned > 0 {
		w("| Axis | Builds | Pass | Fail | Inconclusive | Error | Unsupported | Not run (-dry-run) |\n|---|---|---|---|---|---|---|---|\n")
	} else {
		w("| Axis | Builds | Pass | Fail | Inconclusive | Error | Unsupported |\n|---|---|---|---|---|---|---|\n")
	}
	for _, a := range append(slices.Clone(bs.Axes), tot) {
		w("| %s | %d | %d | %d | %d | %d | %d |", a.Axis, a.Builds, a.Pass, a.Fail, a.Inconclusive, a.Error, a.Unsupported)
		if tot.Planned > 0 {
			w(" %d |", a.Planned)
		}
		w("\n")
	}
	if run := tot.Builds - tot.Unsupported; tot.Builds > 0 {
		w("\nThe engine runs %d of %d builds (%.1f%%) and %d pass.", run, tot.Builds, 100*float64(run)/float64(tot.Builds), tot.Pass)
		if tot.Planned > 0 {
			w(" -dry-run: %d of them were listed, not run.", tot.Planned)
		}
		w("\n")
	}
	var bad []Result
	for _, r := range rep.Results {
		if r.Build != nil && r.Verdict != "pass" && r.Verdict != "unsupported" && r.Verdict != "dry-run" {
			bad = append(bad, r)
		}
	}
	if len(bad) > 0 {
		w("\nNot passing:\n\n| Team | Build | Verdict | Iterations | Worst | Difference | ±95%% |\n|---|---|---|---|---|---|---|\n")
		for _, r := range bad {
			worst := Metric{}
			for _, m := range r.Metrics {
				if math.Abs(m.Diff)+z*m.DiffSE > math.Abs(worst.Diff)+z*worst.DiffSE {
					worst = m
				}
			}
			if worst.Name == "" {
				w("| %s | %s | **%s** | %d | %s | | |\n", r.Team, r.Variant, r.Verdict, r.Iter, r.Error)
				continue
			}
			w("| %s | %s | **%s** | %d | %s | %s | %.2f%% |\n", r.Team, r.Variant, r.Verdict, r.Iter, worst.Name, pct(worst.Diff), 100*z*worst.DiffSE)
		}
	}
	if len(bs.Reasons) > 0 {
		w("\nUnsupported, by what the engine lacks:\n\n| Lacks | Builds | cons | refine | weapon |\n|---|---|---|---|---|\n")
		for _, re := range bs.Reasons {
			w("| %s | %d | %d | %d | %d |\n", re.Name, re.Builds, re.ByAxis["cons"], re.ByAxis["refine"], re.ByAxis["weapon"])
		}
	}
	if len(bs.Weapons) > 0 {
		var ws []string
		for _, k := range slices.Sorted(func(yield func(string) bool) {
			for k := range bs.Weapons {
				if !yield(k) {
					return
				}
			}
		}) {
			ws = append(ws, fmt.Sprintf("%s %d", k, bs.Weapons[k]))
		}
		w("\nWeapons drawn by the weapon axis (builds): %s.\n", strings.Join(ws, ", "))
	}
}
