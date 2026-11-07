package main

// What the owner is shown to approve teams (docs/phase4-approval.md):
// the verified teams in two groups, A to approve at once and B one by
// one, a one-page summary, the full table, and group A's ids.

import (
	"fmt"
	"math"
	"slices"
	"sort"
	"strings"
)

// groupALimit bounds the team's difference, library build and one-pass
// page config alike, for group A: half of compare's 1% bound. Every
// verified team already passed at 1%; this only decides which teams the
// owner sees one by one. It is the owner's to change.
const groupALimit = 0.005

// judgedLimit is compare's bound on a judged character's difference.
const judgedLimit = 0.01

// group puts a verified team in A or B, with why for B. pages is set
// when the release judged the page configs: a library-only release has
// no one-pass score to need.
func group(t Team, pages bool) (string, []string) {
	var why []string
	has := map[string]bool{}
	for _, s := range t.Scores {
		has[s.Build] = true
		if math.Abs(s.Diff) > groupALimit {
			why = append(why, fmt.Sprintf("%s %+.2f%%", s.Build, 100*s.Diff))
		}
		if s.WorstChar != "" && math.Abs(s.WorstDiff) > judgedLimit {
			why = append(why, fmt.Sprintf("%s: %s %+.2f%%", s.Build, s.WorstChar, 100*s.WorstDiff))
		}
	}
	wantScores := []string{"library"}
	if pages {
		wantScores = append(wantScores, "page:once")
	}
	for _, b := range wantScores {
		if !has[b] {
			why = append(why, "no "+b+" score")
		}
	}
	if t.Settled > 0 {
		why = append(why, fmt.Sprintf("%d settled", t.Settled))
	}
	if len(why) > 0 {
		return "B", why
	}
	return "A", nil
}

// largest is a team's largest team difference over its scores.
func largest(t Team) float64 {
	x := 0.0
	for _, s := range t.Scores {
		x = math.Max(x, math.Abs(s.Diff))
	}
	return x
}

// verifiedIDs are the verified teams of group g ("" for both), A before
// B, then the largest difference first.
func verifiedIDs(m *Manifest, g string) []string {
	var ids []string
	for id, t := range m.Teams {
		if t.Verified && (g == "" || t.Group == g) {
			ids = append(ids, id)
		}
	}
	sort.Slice(ids, func(i, j int) bool {
		a, b := m.Teams[ids[i]], m.Teams[ids[j]]
		if a.Group != b.Group {
			return a.Group < b.Group
		}
		if la, lb := largest(a), largest(b); la != lb {
			return la > lb
		}
		return ids[i] < ids[j]
	})
	return ids
}

func header(m *Manifest) string {
	return fmt.Sprintf("Engine %s (module %s), Gscouter %s.", short(m.Engine.Commit), short(m.Engine.WasmSHA256), short(m.Gscouter))
}

// driftLine is the drift with its interval, warning above driftLimit.
func driftLine(d Drift) string {
	s := fmt.Sprintf("%+.3f%% ± %.3f%% (95%%) over %d builds", 100*d.Mean, 196*d.SE, d.Builds)
	if math.Abs(d.Mean) > driftLimit {
		s += fmt.Sprintf(" **above %.1f%%**", 100*driftLimit)
	}
	return s
}

// table writes the score rows of teams ids.
func table(b *strings.Builder, m *Manifest, ids []string, withWhy bool) {
	b.WriteString("| Team | Group | Build | Iterations | gcsim | engine | Difference | ±95% | Judged character most apart | Minor character most apart | Builds passed |\n|---|---|---|---|---|---|---|---|---|---|---|\n")
	for _, id := range ids {
		t := m.Teams[id]
		info := fmt.Sprintf("library %s, variants %s, build space %s, pages %s", t.Builds["library"], t.Builds["variants"], t.Builds["buildspace"], t.Builds["pages"])
		g := t.Group
		if withWhy && len(t.GroupWhy) > 0 {
			g += ": " + strings.Join(t.GroupWhy, "; ")
		}
		for i, s := range t.Scores {
			name, gr, in := id, g, info
			if i > 0 {
				name, gr, in = "", "", ""
			}
			worst, minor := "", ""
			if s.WorstChar != "" {
				worst = fmt.Sprintf("%s %+.2f%%", s.WorstChar, 100*s.WorstDiff)
			}
			if s.WorstMinorChar != "" {
				minor = fmt.Sprintf("%s %+.2f%%", s.WorstMinorChar, 100*s.WorstMinorDiff)
			}
			build := s.Build
			if s.Settled {
				build += " (settled)"
			}
			fmt.Fprintf(b, "| %s | %s | %s | %d | %.1f | %.1f | %+.2f%% | %.2f%% | %s | %s | %s |\n", name, gr, build, s.Iterations, s.Gcsim, s.Gse, 100*s.Diff, 100*s.Half95, worst, minor, in)
		}
	}
}

// approvalTable is every verified team's scores on both engines, group A
// then B, the largest difference first.
func approvalTable(m *Manifest) string {
	var b strings.Builder
	fmt.Fprintf(&b, "# Teams to approve\n\n%s %d of %d supported teams verified. Each verified team's library build (the score a result page starts from) and its one-pass page config, gcsim against the engine; ±95%% is the half-width of the difference's interval. Group A is approved at once, group B one by one (approval-summary.md). A team moves to the engine only with the owner's yes.\n\n", header(m), m.Verified, m.Supported)
	if d := m.Drift; d.Builds > 0 {
		fmt.Fprintf(&b, "Drift: %s.", driftLine(d))
		if math.Abs(d.Mean) > driftLimit {
			b.WriteString(" **The engine as a whole leans; find why before approving.**")
		}
		b.WriteString("\n\n")
	}
	table(&b, m, verifiedIDs(m, ""), false)
	return b.String()
}

// reasonOf is the kind of a line of Team.Why: the verdict, or "no <kind>
// result".
func reasonOf(why string) string {
	if strings.HasPrefix(why, "no ") {
		return why
	}
	if i := strings.LastIndex(why, ": "); i >= 0 {
		why = why[i+2:]
	}
	if f := strings.Fields(why); len(f) > 0 {
		return f[0]
	}
	return why
}

// approvedTeam is a line of Gscouter's approved list
// (internal/engine/approved.txt): a team id, the date of the owner's
// answer, and for a team approved from group B, release=<first 16 hex
// digits of the module's SHA-256>, followed, when Gscouter's pinengine
// carried the approval to that module, by carried=<old module>:identical:
// <date of the pin> (Gscouter's engine.Carry).
type approvedTeam struct {
	team, date, release, carried string
}

// parseApproved reads Gscouter's approved list.
func parseApproved(text string) ([]approvedTeam, error) {
	var out []approvedTeam
	for n, line := range strings.Split(text, "\n") {
		f := strings.Fields(line)
		if len(f) == 0 || strings.HasPrefix(f[0], "#") {
			continue
		}
		if len(f) < 2 || len(f) > 4 {
			return nil, fmt.Errorf("approved list, line %d: %q", n+1, line)
		}
		a := approvedTeam{team: f[0], date: f[1]}
		if len(f) >= 3 {
			r, ok := strings.CutPrefix(f[2], "release=")
			if !ok || len(r) != 16 {
				return nil, fmt.Errorf("approved list, line %d: %q", n+1, line)
			}
			a.release = r
		}
		if len(f) == 4 {
			c, ok := strings.CutPrefix(f[3], "carried=")
			if p := strings.Split(c, ":"); !ok || len(p) != 3 || len(p[0]) != 16 || p[1] != "identical" {
				return nil, fmt.Errorf("approved list, line %d: %q", n+1, line)
			}
			a.carried = c
		}
		out = append(out, a)
	}
	return out, nil
}

// dropped says why an approval does not hold for m's release, as
// Gscouter's engine.Dropped does: an approval holds while its team is
// verified and in group A, one from group B for its release only.
func dropped(m *Manifest, a approvedTeam) string {
	t, ok := m.Teams[a.team]
	switch {
	case !ok:
		return "not supported"
	case !t.Verified:
		return "not verified: " + strings.Join(t.Why, "; ")
	case a.release != "":
		if !strings.HasPrefix(m.Engine.WasmSHA256, a.release) {
			return "approved from group B for module " + a.release + " only; in group " + t.Group + " " + strings.Join(t.GroupWhy, "; ")
		}
	case t.Group != "A":
		return "left group A: " + strings.Join(t.GroupWhy, "; ")
	}
	return ""
}

// approvalSummary is the page the owner answers. approved is Gscouter's
// approved list, nil when not known: its teams that still hold run on
// this release without a new answer, and the others are listed.
func approvalSummary(m *Manifest, approved []approvedTeam) string {
	var b strings.Builder
	listed := map[string]bool{}
	var carried int
	var drops []string
	for _, ap := range approved {
		listed[ap.team] = true
		if w := dropped(m, ap); w != "" {
			drops = append(drops, fmt.Sprintf("| %s | %s | %s |", ap.team, ap.date, w))
		} else {
			carried++
		}
	}
	var a, bs []string
	for _, id := range verifiedIDs(m, "A") {
		if !listed[id] {
			a = append(a, id)
		}
	}
	for _, id := range verifiedIDs(m, "B") {
		if !listed[id] {
			bs = append(bs, id)
		}
	}
	fmt.Fprintf(&b, "# Approving teams for the engine\n\n%s\n\n", header(m))
	if approved != nil {
		fmt.Fprintf(&b, "- Approved before: %d. Still holding (verified, in group A, or approved from group B for this module), so they stay on the engine with no new answer: %d. **Dropped: %d** (below): back on gcsim, each back to the owner.\n", len(approved), carried, len(drops))
	}
	fmt.Fprintf(&b, "- Supported: %d. Verified: %d. **Group A to answer: %d** (approved at once). **Group B to answer: %d** (one by one, below). Unverified: %d.", m.Supported, m.Verified, len(a), len(bs), m.Supported-m.Verified)
	if approved != nil {
		b.WriteString(" Groups count only the teams not approved before.")
	}
	b.WriteString("\n")
	pagesRun := slices.Contains(m.Kinds, "pages")
	fmt.Fprintf(&b, "- Kinds judged: %s.", strings.Join(m.Kinds, ", "))
	b.WriteString(" Verification is the library build and the probes within 1% of gcsim (the owner, 2026-11-04); a verified, approved team runs on the engine for any build a player enters whose content the engine has.")
	b.WriteString("\n")
	if pagesRun {
		fmt.Fprintf(&b, "- Group A: both the library build's and the one-pass page config's team DPS within ±%.2f%% of gcsim, no build with a settled verdict, every judged character within ±%.0f%%. Every verified team passed compare at 1%%; the groups only decide how closely each is read.\n", 100*groupALimit, 100*judgedLimit)
	} else {
		fmt.Fprintf(&b, "- Group A: the library build's team DPS within ±%.2f%% of gcsim, no build with a settled verdict, every judged character within ±%.0f%%. No one-pass page score: the page configs were not run in this release. Every verified team passed compare at 1%%; the groups only decide how closely each is read.\n", 100*groupALimit, 100*judgedLimit)
	}
	if m.Drift.Builds > 0 {
		fmt.Fprintf(&b, "- Drift: %s.", driftLine(m.Drift))
		if math.Abs(m.Drift.Mean) > driftLimit {
			b.WriteString(" **The engine as a whole leans: offer no team until it is found.**")
		}
		b.WriteString("\n")
	}
	var kinds []string
	for k := range m.DriftByKind {
		kinds = append(kinds, k)
	}
	sort.Strings(kinds)
	for _, k := range kinds {
		fmt.Fprintf(&b, "  - %s: %s\n", k, driftLine(m.DriftByKind[k]))
	}

	// Unverified teams by reason; a team can have several.
	reasons := map[string]int{}
	for _, t := range m.Teams {
		if t.Verified {
			continue
		}
		seen := map[string]bool{}
		for _, w := range t.Why {
			r := reasonOf(w)
			if !seen[r] {
				seen[r] = true
				reasons[r]++
			}
		}
	}
	if len(reasons) > 0 {
		var rs []string
		for r := range reasons {
			rs = append(rs, r)
		}
		sort.Slice(rs, func(i, j int) bool {
			if reasons[rs[i]] != reasons[rs[j]] {
				return reasons[rs[i]] > reasons[rs[j]]
			}
			return rs[i] < rs[j]
		})
		b.WriteString("\n## Unverified teams by reason\n\nA team with several reasons counts under each. The reasons per team are in manifest.json (`why`).\n\n| Reason | Teams |\n|---|---|\n")
		for _, r := range rs {
			fmt.Fprintf(&b, "| %s | %d |\n", r, reasons[r])
		}
	}

	// How the verified teams' differences spread.
	b.WriteString("\n## Differences of the verified teams\n\n| Build | Teams | within 0.1% | 0.25% | 0.5% | 1% | above 1% | Mean | Lowest | Highest |\n|---|---|---|---|---|---|---|---|---|---|\n")
	for _, build := range []string{"library", "page:once"} {
		type td struct {
			id string
			d  float64
		}
		var xs []td
		for _, id := range verifiedIDs(m, "") {
			for _, s := range m.Teams[id].Scores {
				if s.Build == build {
					xs = append(xs, td{id, s.Diff})
				}
			}
		}
		if len(xs) == 0 {
			continue
		}
		bounds := []float64{0.001, 0.0025, 0.005, 0.01, math.Inf(1)}
		counts := make([]int, len(bounds))
		mean := 0.0
		for _, x := range xs {
			mean += x.d
			for i, bd := range bounds {
				if math.Abs(x.d) <= bd {
					counts[i]++
					break
				}
			}
		}
		mean /= float64(len(xs))
		sort.Slice(xs, func(i, j int) bool { return xs[i].d < xs[j].d })
		ends := func(list []td) string {
			var p []string
			for _, x := range list {
				p = append(p, fmt.Sprintf("%s %+.2f%%", x.id, 100*x.d))
			}
			return strings.Join(p, "<br>")
		}
		n := min(5, len(xs))
		var high []td
		for i := len(xs) - 1; i >= len(xs)-n; i-- {
			high = append(high, xs[i])
		}
		fmt.Fprintf(&b, "| %s | %d | %d | %d | %d | %d | %d | %+.3f%% | %s | %s |\n", build, len(xs), counts[0], counts[1], counts[2], counts[3], counts[4], 100*mean, ends(xs[:n]), ends(high))
	}
	b.WriteString("\nEach column counts the teams above the bound of the column before it, up to its own.\n")

	if len(drops) > 0 {
		fmt.Fprintf(&b, "\n## Approvals this release drops: %d\n\nEach runs on gcsim from this release on, and needs the owner's yes again before it runs on the engine (Gscouter's cmd/pinengine takes it off the approved list when pinning).\n\n| Team | Approved | Why |\n|---|---|---|\n", len(drops))
		sort.Strings(drops)
		b.WriteString(strings.Join(drops, "\n") + "\n")
	}

	fmt.Fprintf(&b, "\n## Group B: %d teams, one by one\n\nLargest difference first, with why each is in group B.\n\n", len(bs))
	if len(bs) > 0 {
		table(&b, m, bs, true)
	}
	b.WriteString("\nEvery verified team, group A included, is in approval.md; group A's ids are in approved-candidates.txt, and everything per team in manifest.json, all in this artifact.\n")
	return b.String()
}

// candidateList is group A's ids not on the approved list, one per
// line, for Gscouter's approved.txt once the owner answers: they carry
// no date, since only the owner's answer gives one.
func candidateList(m *Manifest, approved []approvedTeam) string {
	listed := map[string]bool{}
	for _, a := range approved {
		listed[a.team] = true
	}
	var b strings.Builder
	fmt.Fprintf(&b, "# Group A of %s\n# Team DPS within ±%.2f%% of gcsim on the library build (and the one-pass\n# page config when the release ran the pages), no settled verdict (docs/phase4-approval.md).\n# NOT APPROVED. Only the owner approves: add these to Gscouter's\n# internal/engine/approved.txt with the date of the owner's answer, after it.\n", header(m), 100*groupALimit)
	var ids []string
	for _, id := range verifiedIDs(m, "A") {
		if !listed[id] {
			ids = append(ids, id)
		}
	}
	sort.Strings(ids)
	for _, id := range ids {
		b.WriteString(id + "\n")
	}
	return b.String()
}
