// Command coverage writes the probes' coverage map and checks it against
// what the engine has (docs/probe-coverage.md).
//
// Usage, from reference/:
//
//	go run ./coverage -gscouter ../../Gscouter            # writes probes/coverage.json
//	go run ./coverage -gscouter ../../Gscouter -check     # fails if it is stale or a gap is unexplained
//
// It reads every probe (probes/*.txt, the format of docs/verification.md
// §3, read as compare reads it), every library team's build.txt and
// rotation.gcsl in a Gscouter checkout (the commit compare.yml pins), the
// release manifest of that checkout (which library teams are verified) and
// the engine's key catalog (gse --catalog: what is implemented, and the
// names that resolve to each key). No gcsim is run.
//
// The items are every implemented character at C1 to C6, every
// implemented weapon at R1 and R5 and every implemented set at 2 and 4
// pieces. A build reaches a character's Cn when its cons is n or more, a
// weapon's Rn when its refine is n, and a set's n pieces when its count is
// n or more. An item is covered by the library when a verified team's
// library build reaches it, by a probe when a probe's build reaches it
// through a line the probe changes, and otherwise is a gap: explained when
// coverage/gaps.txt names it with a reason, unexplained if not. gaps.txt
// also lists the parts of covered items no probe reaches ("partial").
package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"slices"
	"sort"
	"strconv"
	"strings"
)

var (
	gscouterDir  = flag.String("gscouter", "", "Gscouter checkout at the commit compare.yml pins (its teams/ and internal/engine/release/manifest.json)")
	gsePath      = flag.String("gse", "../engine/target/release/gse", "the engine's executable (for gse --catalog)")
	catalogPath  = flag.String("catalog", "", "read the catalog from this file (gse --catalog's output) instead of running gse")
	probesDir    = flag.String("probes", "probes", "directory of probes")
	gapsPath     = flag.String("gaps", "coverage/gaps.txt", "known gaps, with their reasons")
	outPath      = flag.String("out", "probes/coverage.json", "the coverage map")
	workflowPath = flag.String("workflow", "../.github/workflows/compare.yml", "the workflow whose GSCOUTER_COMMIT the checkout must be at")
	check        = flag.Bool("check", false, "write nothing; fail if -out differs from what this run makes, a probe it lists is gone, or a gap is unexplained")
)

func fail(err error) {
	fmt.Fprintln(os.Stderr, "coverage:", err)
	os.Exit(1)
}

// ---- the catalog ----

type catalogEntry struct {
	Key         string   `json:"key"`
	Aliases     []string `json:"aliases"`
	Implemented bool     `json:"implemented"`
}

type catalog struct {
	Characters []catalogEntry `json:"characters"`
	Weapons    []catalogEntry `json:"weapons"`
	Sets       []catalogEntry `json:"sets"`
}

// names maps every name gcsl accepts to its key.
func names(es []catalogEntry) map[string]string {
	m := map[string]string{}
	for _, e := range es {
		m[e.Key] = e.Key
		for _, a := range e.Aliases {
			m[a] = e.Key
		}
	}
	return m
}

func readCatalog() catalog {
	var b []byte
	var err error
	if *catalogPath != "" {
		b, err = os.ReadFile(*catalogPath)
	} else {
		b, err = exec.Command(*gsePath, "--catalog").Output()
	}
	if err != nil {
		fail(fmt.Errorf("catalog: %v", err))
	}
	var c catalog
	if err := json.Unmarshal(b, &c); err != nil {
		fail(fmt.Errorf("catalog: %v", err))
	}
	return c
}

// ---- builds ----

// member is what a build gives one character.
type member struct {
	Char   string `json:"char"`
	Cons   int    `json:"cons"`
	Weapon string `json:"weapon"`
	Refine int    `json:"refine"`
	Sets   []pc   `json:"sets"`
}

type pc struct {
	Set   string `json:"set"`
	Count int    `json:"count"`
}

var (
	buildLine  = regexp.MustCompile(`^\s*(\S+)\s+(char|add\s+weapon|add\s+set|add\s+stats)\b(.*)$`)
	actionName = regexp.MustCompile(`^[a-z_]+(\[[^\]]*\])?(:\d+)?$`)
	param      = regexp.MustCompile(`(\w+)=("[^"]*"|[^\s;]+)`)
)

type resolver struct {
	chars, weapons, sets map[string]string
}

// members reads a build: its characters in order, as gcsim reads them
// (a name gcsl resolves to a key; a key the catalog lacks stays as written).
func (r resolver) members(build string) ([]member, error) {
	var out []member
	idx := map[string]int{}
	for _, l := range strings.Split(build, "\n") {
		t := strings.TrimSpace(l)
		if t == "" || strings.HasPrefix(t, "#") {
			continue
		}
		m := buildLine.FindStringSubmatch(t)
		if m == nil {
			continue
		}
		name := m[1]
		if k, ok := r.chars[name]; ok {
			name = k
		}
		i, ok := idx[name]
		if !ok {
			i = len(out)
			idx[name] = i
			out = append(out, member{Char: name})
		}
		ps := map[string]string{}
		for _, p := range param.FindAllStringSubmatch(t[len(m[1]):], -1) {
			ps[p[1]] = strings.Trim(p[2], `"`)
		}
		atoi := func(k string) int {
			n, _ := strconv.Atoi(ps[k])
			return n
		}
		switch strings.Join(strings.Fields(m[2]), " ") {
		case "char":
			out[i].Cons = atoi("cons")
		case "add weapon":
			w := ps["weapon"]
			if k, ok := r.weapons[w]; ok {
				w = k
			}
			out[i].Weapon, out[i].Refine = w, atoi("refine")
			if out[i].Refine == 0 {
				out[i].Refine = 1
			}
		case "add set":
			s := ps["set"]
			if k, ok := r.sets[s]; ok {
				s = k
			}
			out[i].Sets = append(out[i].Sets, pc{s, atoi("count")})
		}
	}
	return out, nil
}

// reaches lists the items a member reaches (see the package comment).
func (m member) reaches() []string {
	var out []string
	for c := 1; c <= min(m.Cons, 6); c++ {
		out = append(out, fmt.Sprintf("char:%s:c%d", m.Char, c))
	}
	if m.Weapon != "" && (m.Refine == 1 || m.Refine == 5) {
		out = append(out, fmt.Sprintf("weapon:%s:r%d", m.Weapon, m.Refine))
	}
	for _, s := range m.Sets {
		for _, n := range []int{2, 4} {
			if s.Count >= n {
				out = append(out, fmt.Sprintf("set:%s:%d", s.Set, n))
			}
		}
	}
	return out
}

// ---- probes (read as reference/compare's parseProbe and apply do) ----

type hunk struct{ old, new []string }

type probe struct {
	name, desc, team string
	lines            []string
	hunks            []hunk
}

func parseProbe(name, text string) (probe, error) {
	pr := probe{name: name}
	inNew := false
	for _, l := range strings.Split(text, "\n") {
		t := strings.TrimSpace(l)
		switch {
		case t == "":
		case strings.HasPrefix(t, "#"):
			if pr.desc == "" {
				pr.desc = strings.TrimSpace(strings.TrimPrefix(t, "#"))
			}
		case strings.HasPrefix(t, "team "):
			pr.team = strings.TrimSpace(strings.TrimPrefix(t, "team "))
		case t == "rotation-" || strings.HasPrefix(t, "rotation- "):
			if len(pr.hunks) == 0 || inNew {
				pr.hunks = append(pr.hunks, hunk{})
				inNew = false
			}
			h := &pr.hunks[len(pr.hunks)-1]
			h.old = append(h.old, strings.TrimSpace(strings.TrimPrefix(t, "rotation-")))
		case t == "rotation+" || strings.HasPrefix(t, "rotation+ "):
			if len(pr.hunks) == 0 {
				return pr, fmt.Errorf("probe %s: %q has no rotation- line before it", name, t)
			}
			h := &pr.hunks[len(pr.hunks)-1]
			h.new = append(h.new, strings.TrimSpace(strings.TrimPrefix(t, "rotation+")))
			inNew = true
		case buildLine.MatchString(t):
			pr.lines = append(pr.lines, t)
		default:
			return pr, fmt.Errorf("probe %s: cannot read %q", name, t)
		}
	}
	if pr.team == "" {
		return pr, fmt.Errorf("probe %s: no team line", name)
	}
	if len(pr.lines) == 0 && len(pr.hunks) == 0 {
		return pr, fmt.Errorf("probe %s: changes nothing", name)
	}
	return pr, nil
}

func lineKey(l string) (string, bool) {
	m := buildLine.FindStringSubmatch(l)
	if m == nil {
		return "", false
	}
	return m[1] + " " + strings.Join(strings.Fields(m[2]), " "), true
}

// apply replaces the build's lines as compare does, and names the
// characters (as written) whose lines it replaced.
func (pr probe) apply(build string) (string, []string, error) {
	repl := map[string][]string{}
	for _, l := range pr.lines {
		k, _ := lineKey(l)
		repl[k] = append(repl[k], l)
	}
	var out, changed []string
	done := map[string]bool{}
	for _, l := range strings.Split(build, "\n") {
		k, ok := lineKey(l)
		if !ok || repl[k] == nil {
			out = append(out, l)
			continue
		}
		if !done[k] {
			out = append(out, repl[k]...)
			done[k] = true
			changed = append(changed, strings.Fields(k)[0])
		}
	}
	for k := range repl {
		if !done[k] {
			return "", nil, fmt.Errorf("probe %s: the build has no line %q to replace", pr.name, k)
		}
	}
	return strings.Join(out, "\n"), changed, nil
}

// addedActions applies the probe's hunks to a rotation, as compare does
// (a hunk must match exactly one place), and returns the actions the
// hunks add: each hunk's new lines' actions less its old lines', read in
// the context of the rotation line the hunk continues.
func (r resolver) addedActions(pr probe, rot string) ([]string, error) {
	lines := strings.Split(rot, "\n")
	var added []string
	for i, h := range pr.hunks {
		var at []int
		for s := 0; s+len(h.old) <= len(lines); s++ {
			ok := true
			for j, o := range h.old {
				if strings.TrimSpace(lines[s+j]) != o {
					ok = false
					break
				}
			}
			if ok {
				at = append(at, s)
			}
		}
		if len(at) != 1 {
			return nil, fmt.Errorf("probe %s: rotation hunk %d matches %d places, not one", pr.name, i+1, len(at))
		}
		s := at[0]
		ctx := r.context(lines[:s])
		left := map[string]int{}
		for _, a := range r.actions(ctx, h.old) {
			left[a]++
		}
		for _, a := range r.actions(ctx, h.new) {
			if left[a] > 0 {
				left[a]--
				continue
			}
			added = append(added, a)
		}
		lines = slices.Concat(lines[:s], h.new, lines[s+len(h.old):])
	}
	return added, nil
}

// context is the character a line after these continues the action list
// of: the last statement's character when it ends with a comma.
func (r resolver) context(before []string) string {
	cur := ""
	for _, l := range before {
		cur = r.scan(cur, l, nil)
	}
	return cur
}

// actions reads the character actions of gcsl lines, "<char> a, b:3,
// c[p=1];" and the continuation lines after a trailing comma, starting
// in the action list of cur ("" for none). Control lines are skipped;
// the statements inside a one-line block ("if c { a burst; }") are read.
func (r resolver) actions(cur string, lines []string) []string {
	var out []string
	for _, l := range lines {
		cur = r.scan(cur, l, &out)
	}
	return out
}

func (r resolver) scan(cur, l string, out *[]string) string {
	t := strings.TrimSpace(l)
	for _, c := range []string{"#", "//"} {
		if i := strings.Index(t, c); i >= 0 {
			t = strings.TrimSpace(t[:i])
		}
	}
	if i, j := strings.Index(t, "{"), strings.LastIndex(t, "}"); i >= 0 && j > i {
		t = strings.TrimSpace(t[i+1 : j])
		cur = ""
	} else if strings.ContainsAny(t, "{}") {
		return ""
	}
	if t == "" {
		return cur
	}
	for _, st := range strings.SplitAfter(t, ";") {
		st = strings.TrimSpace(st)
		if st == "" {
			continue
		}
		f := strings.Fields(st)
		body := st
		if k, ok := r.chars[strings.Trim(f[0], ",;")]; ok {
			cur = k
			body = strings.TrimSpace(st[len(f[0]):])
		} else if cur == "" {
			continue
		}
		for _, a := range splitActions(body) {
			a = strings.Join(strings.Fields(strings.TrimSuffix(strings.TrimSpace(a), ";")), "")
			if a == "" || !actionName.MatchString(a) {
				continue
			}
			if out != nil {
				*out = append(*out, cur+" "+a)
			}
		}
		if strings.HasSuffix(st, ";") {
			cur = ""
		}
	}
	return cur
}

// splitActions splits an action list at the commas outside brackets
// (skill[hold=1,x=2], attack:3).
func splitActions(s string) []string {
	var out []string
	depth, start := 0, 0
	for i, c := range s {
		switch c {
		case '[':
			depth++
		case ']':
			depth--
		case ',':
			if depth == 0 {
				out = append(out, s[start:i])
				start = i + 1
			}
		}
	}
	return append(out, s[start:])
}

// ---- gaps ----

type gap struct {
	Kind   string `json:"kind"` // unreached or partial
	Reason string `json:"reason"`
	Note   string `json:"note"`
}

// readGaps reads lines "<item> <unreached|partial> <reason> <note...>".
func readGaps(path string) (map[string][]gap, error) {
	b, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	out := map[string][]gap{}
	for i, l := range strings.Split(string(b), "\n") {
		t := strings.TrimSpace(l)
		if t == "" || strings.HasPrefix(t, "#") {
			continue
		}
		f := strings.Fields(t)
		if len(f) < 4 || (f[1] != "unreached" && f[1] != "partial") {
			return nil, fmt.Errorf("%s:%d: want \"<item> unreached|partial <reason> <note>\"", path, i+1)
		}
		note := strings.TrimSpace(t[strings.Index(t, f[2])+len(f[2]):])
		out[f[0]] = append(out[f[0]], gap{f[1], f[2], note})
	}
	return out, nil
}

// ---- the map ----

type probeOut struct {
	Name         string   `json:"name"`
	Team         string   `json:"team"`
	Description  string   `json:"description"`
	TeamVerified bool     `json:"team_verified"`
	Changed      []member `json:"changed"`
	Hunks        int      `json:"rotation_hunks,omitempty"`
	Actions      []string `json:"actions,omitempty"`
	Reaches      []string `json:"reaches"`
}

type itemOut struct {
	ID      string `json:"id"`
	Status  string `json:"status"` // library, probe, gap (a documented reason), gap-new (listed, no reason yet), unexplained
	Library int    `json:"library_builds"`
	// library builds of teams the release does not verify: compared, but
	// not counted as coverage
	LibraryUnverified int      `json:"library_builds_unverified,omitempty"`
	Probes            []string `json:"probes,omitempty"`
	Gaps              []gap    `json:"gaps,omitempty"`
}

type counts struct {
	Probes             int `json:"probes"`
	ProbesTeamVerified int `json:"probes_on_verified_teams"`
	Teams              int `json:"library_teams"`
	TeamsVerified      int `json:"library_teams_verified"`
	Items              int `json:"items"`
	Library            int `json:"covered_by_library"`
	Probe              int `json:"covered_by_probe_only"`
	Explained          int `json:"gaps_explained"`
	New                int `json:"gaps_new"`
	Unexplained        int `json:"gaps_unexplained"`
	Partial            int `json:"covered_with_partial_gaps"`
}

type coverageMap struct {
	Comment        string     `json:"comment"`
	GscouterCommit string     `json:"gscouter_commit"`
	ReleaseEngine  string     `json:"release_engine_commit"`
	Counts         counts     `json:"counts"`
	Probes         []probeOut `json:"probes"`
	Items          []itemOut  `json:"items"`
}

func pinnedCommit() string {
	b, err := os.ReadFile(*workflowPath)
	if err != nil {
		fail(err)
	}
	m := regexp.MustCompile(`(?m)^\s*GSCOUTER_COMMIT:\s*(\S+)`).FindSubmatch(b)
	if m == nil {
		fail(fmt.Errorf("%s: no GSCOUTER_COMMIT", *workflowPath))
	}
	return string(m[1])
}

func build() coverageMap {
	if *gscouterDir == "" {
		fail(fmt.Errorf("-gscouter is required"))
	}
	pinned := pinnedCommit()
	head, err := exec.Command("git", "-C", *gscouterDir, "rev-parse", "HEAD").Output()
	if err != nil {
		fail(fmt.Errorf("git rev-parse in %s: %v", *gscouterDir, err))
	}
	if h := strings.TrimSpace(string(head)); h != pinned {
		fail(fmt.Errorf("%s is at %s, compare.yml pins %s", *gscouterDir, h, pinned))
	}
	cat := readCatalog()
	r := resolver{names(cat.Characters), names(cat.Weapons), names(cat.Sets)}

	var man struct {
		Engine struct {
			Commit string `json:"commit"`
		} `json:"engine"`
		Teams map[string]struct {
			Verified bool `json:"verified"`
		} `json:"teams"`
	}
	mb, err := os.ReadFile(filepath.Join(*gscouterDir, "internal", "engine", "release", "manifest.json"))
	if err != nil {
		fail(err)
	}
	if err := json.Unmarshal(mb, &man); err != nil {
		fail(fmt.Errorf("manifest: %v", err))
	}

	gaps, err := readGaps(*gapsPath)
	if err != nil {
		fail(err)
	}

	cm := coverageMap{
		Comment:        "Written by reference/coverage (docs/probe-coverage.md). Do not edit; regenerate.",
		GscouterCommit: pinned,
		ReleaseEngine:  man.Engine.Commit,
	}
	libCount, libUnverified := map[string]int{}, map[string]int{}
	byProbe := map[string][]string{}

	teamsDir := filepath.Join(*gscouterDir, "teams")
	ents, err := os.ReadDir(teamsDir)
	if err != nil {
		fail(err)
	}
	builds := map[string]string{}
	for _, e := range ents {
		if !e.IsDir() {
			continue
		}
		b, err := os.ReadFile(filepath.Join(teamsDir, e.Name(), "build.txt"))
		if err != nil {
			continue
		}
		builds[e.Name()] = string(b)
		cm.Counts.Teams++
		count := libUnverified
		if man.Teams[e.Name()].Verified {
			cm.Counts.TeamsVerified++
			count = libCount
		}
		ms, _ := r.members(string(b))
		seen := map[string]bool{}
		for _, m := range ms {
			for _, it := range m.reaches() {
				if !seen[it] {
					seen[it] = true
					count[it]++
				}
			}
		}
	}

	pents, err := os.ReadDir(*probesDir)
	if err != nil {
		fail(err)
	}
	for _, e := range pents {
		if e.IsDir() || !strings.HasSuffix(e.Name(), ".txt") {
			continue
		}
		b, err := os.ReadFile(filepath.Join(*probesDir, e.Name()))
		if err != nil {
			fail(err)
		}
		pr, err := parseProbe(strings.TrimSuffix(e.Name(), ".txt"), string(b))
		if err != nil {
			fail(err)
		}
		base, ok := builds[pr.team]
		if !ok {
			fail(fmt.Errorf("probe %s: no library team %s", pr.name, pr.team))
		}
		nb, changedNames, err := pr.apply(base)
		if err != nil {
			fail(err)
		}
		ms, _ := r.members(nb)
		changed := map[string]bool{}
		for _, n := range changedNames {
			if k, ok := r.chars[n]; ok {
				n = k
			}
			changed[n] = true
		}
		po := probeOut{Name: pr.name, Team: pr.team, Description: pr.desc, TeamVerified: man.Teams[pr.team].Verified}
		po.Hunks = len(pr.hunks)
		if len(pr.hunks) > 0 {
			rot, err := os.ReadFile(filepath.Join(teamsDir, pr.team, "rotation.gcsl"))
			if err != nil {
				fail(err)
			}
			added, err := r.addedActions(pr, string(rot))
			if err != nil {
				fail(err)
			}
			seenA := map[string]bool{}
			for _, a := range added {
				if !seenA[a] {
					seenA[a] = true
					po.Actions = append(po.Actions, a)
				}
				changed[strings.Fields(a)[0]] = true
			}
			sort.Strings(po.Actions)
		}
		seen := map[string]bool{}
		for _, m := range ms {
			if !changed[m.Char] {
				continue
			}
			if m.Sets == nil {
				m.Sets = []pc{}
			}
			po.Changed = append(po.Changed, m)
			for _, it := range m.reaches() {
				if !seen[it] {
					seen[it] = true
					po.Reaches = append(po.Reaches, it)
					byProbe[it] = append(byProbe[it], pr.name)
				}
			}
		}
		if po.Reaches == nil {
			po.Reaches = []string{}
		}
		cm.Probes = append(cm.Probes, po)
		cm.Counts.Probes++
		if po.TeamVerified {
			cm.Counts.ProbesTeamVerified++
		}
	}

	var ids []string
	for _, c := range cat.Characters {
		if c.Implemented {
			for n := 1; n <= 6; n++ {
				ids = append(ids, fmt.Sprintf("char:%s:c%d", c.Key, n))
			}
		}
	}
	for _, w := range cat.Weapons {
		if w.Implemented {
			ids = append(ids, "weapon:"+w.Key+":r1", "weapon:"+w.Key+":r5")
		}
	}
	for _, s := range cat.Sets {
		if s.Implemented {
			ids = append(ids, "set:"+s.Key+":2", "set:"+s.Key+":4")
		}
	}
	known := map[string]bool{}
	for _, id := range ids {
		known[id] = true
		it := itemOut{ID: id, Library: libCount[id], LibraryUnverified: libUnverified[id], Probes: byProbe[id], Gaps: gaps[id]}
		sort.Strings(it.Probes)
		var unreached, isNew bool
		for _, g := range it.Gaps {
			unreached = unreached || g.Kind == "unreached"
			isNew = isNew || (g.Kind == "unreached" && g.Reason == "new")
		}
		switch {
		case it.Library > 0:
			it.Status = "library"
			cm.Counts.Library++
		case len(it.Probes) > 0:
			it.Status = "probe"
			cm.Counts.Probe++
		case isNew:
			it.Status = "gap-new"
			cm.Counts.New++
		case unreached:
			it.Status = "gap"
			cm.Counts.Explained++
		default:
			it.Status = "unexplained"
			cm.Counts.Unexplained++
		}
		if it.Status != "gap" && it.Status != "gap-new" && unreached {
			fail(fmt.Errorf("%s: gaps.txt calls it unreached, but it is covered (%s); remove or change the entry", id, it.Status))
		}
		if (it.Status == "library" || it.Status == "probe") && len(it.Gaps) > 0 {
			cm.Counts.Partial++
		}
		cm.Items = append(cm.Items, it)
	}
	cm.Counts.Items = len(ids)
	var stray []string
	for id := range gaps {
		if !known[id] {
			stray = append(stray, id)
		}
	}
	if len(stray) > 0 {
		sort.Strings(stray)
		fail(fmt.Errorf("gaps.txt names items the engine does not have: %s", strings.Join(stray, ", ")))
	}
	return cm
}

func encode(cm coverageMap) []byte {
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetIndent("", " ")
	enc.SetEscapeHTML(false)
	if err := enc.Encode(cm); err != nil {
		fail(err)
	}
	return buf.Bytes()
}

func main() {
	flag.Parse()
	cm := build()
	out := encode(cm)
	c := cm.Counts
	summary := fmt.Sprintf("%d probes (%d on verified teams); %d library teams (%d verified); %d items: %d by the library, %d by probes only, %d gaps with a documented reason, %d new gaps listed without one, %d unexplained; %d covered items with partial gaps",
		c.Probes, c.ProbesTeamVerified, c.Teams, c.TeamsVerified, c.Items, c.Library, c.Probe, c.Explained, c.New, c.Unexplained, c.Partial)
	var unexplained []string
	for _, it := range cm.Items {
		if it.Status == "unexplained" {
			unexplained = append(unexplained, it.ID)
		}
	}
	if !*check {
		if err := os.WriteFile(*outPath, out, 0o644); err != nil {
			fail(err)
		}
		fmt.Println(summary)
		for _, id := range unexplained {
			fmt.Println("unexplained:", id)
		}
		return
	}
	var problems []string
	old, err := os.ReadFile(*outPath)
	if err != nil {
		fail(err)
	}
	var prev coverageMap
	if err := json.Unmarshal(old, &prev); err != nil {
		fail(fmt.Errorf("%s: %v", *outPath, err))
	}
	now := map[string]bool{}
	for _, p := range cm.Probes {
		now[p.Name] = true
	}
	for _, p := range prev.Probes {
		if !now[p.Name] {
			problems = append(problems, "probe removed: "+p.Name)
		}
	}
	for _, id := range unexplained {
		problems = append(problems, "unexplained gap: "+id+" (add a probe, or its reason to coverage/gaps.txt)")
	}
	if !bytes.Equal(old, out) {
		problems = append(problems, *outPath+" is stale: run go run ./coverage -gscouter <checkout> and commit it")
	}
	fmt.Println(summary)
	if len(problems) > 0 {
		for _, p := range problems {
			fmt.Fprintln(os.Stderr, p)
		}
		os.Exit(1)
	}
}
