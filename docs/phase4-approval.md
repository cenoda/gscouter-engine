# Phase 4: how the owner approves teams

> Status: proposal, waiting for the owner; the groups are written by `reference/manifest` (§4) · 2026-11-03

A library team moves to the engine in Gscouter only with the owner's yes for that team (`CLAUDE.md`), recorded in Gscouter's `internal/engine/approved.txt` with the date of the answer (Gscouter `docs/engine.md` §2). A release (`compare.yml` with `release=true` on `main`) writes `manifest.json` and `approval.md`. `approval.md` has two rows per verified team, the library build and the one-pass page config (`page:once`), each with gcsim's and the engine's team DPS, the difference with its 95% half-width, and the character furthest apart, plus the drift over every build compared.

With about 3,000 verified teams that table is about 6,000 rows, too long for anyone to read and answer team by team. This document proposes how to put it to the owner without loosening the bound: every team offered has already passed `compare` at 1%. The groups below only decide how closely the owner looks at each team.

## 1. What a verified team already means

The manifest marks a team verified only when the census counts it supported and every build of it the engine runs passes: the library build and, sampled at most four per team (`-sample 4`), its variants, build-space builds and page configs (the filler and rotation candidates the page's search tries; the one-pass configs always). Each one passed on team DPS and on every character with at least 10% of the team's DPS, within 1% of gcsim. The drift over every build compared must stay within ±0.2%. A team that failed, stayed inconclusive, was deferred, errored or was idle is not offered at all. The owner is not asked to judge the 1% bound again. The owner decides whether a score that moves by less than 1% (roadmap §4.1) is acceptable for each team.

## 2. Proposed answer: one batch, then the exceptions

**Gate, before anything is offered.** The drift is within ±0.2%. If it is not, nothing is offered: the whole engine leans, and the cause is found first (`approval.md` already warns).

**Group A: approve at once.** A verified team is in group A when, for both its library build and its one-pass page config:

- the team's difference is within ±0.5% (point estimate),

and for the team as a whole:

- no build of the team (library, variants, build space, pages) has a *settled* verdict (decided by its estimate at 32,000 iterations with the interval still across the bound), and
- every judged character (at least 10% of the team's DPS) is within ±1% by its estimate.

The owner answers group A with one yes, naming the release (engine commit and module SHA-256). That yes is recorded as one line per team in `approved.txt`, each with the date of the answer, under a comment that names the release and this rule. The owner sees the summary below and can open the full table, but is not asked to read it row by row.

**Group B: listed one by one.** Every other verified team: a difference between 0.5% and 1% on either score, a settled verdict on any of its builds, a judged character outside 1% by its estimate, or no one-pass score (its one-pass config is one the engine cannot run). How many is known only from the release (a team whose energy waits spiral is often settled somewhere in its builds); listed sorted by the largest difference first, each with both scores, the character furthest apart and the builds that passed. The owner approves each team, approves a list, or leaves teams on gcsim. A team left out stays on gcsim and can be offered again by a later release.

**Not offered.** The unverified teams, counted by reason (failed, inconclusive, errored, deferred, idle, a library build or variant the engine cannot run, no result of a kind), so that the owner sees what is left and why.

The 0.5% line is half the bound, which is about where the site's own noise lies (50 iterations, about 0.7% standard error). A team in group A moves by less than a reload of its own page does today. The line only sends teams to closer review. It never lets a team through that `compare` failed.

## 3. What the owner is shown

One page in place of the 6,000 rows:

1. The release: engine commit, module SHA-256, Gscouter commit.
2. Counts: supported, verified, group A, group B, unverified by reason.
3. The drift, with its interval, over every build and over each kind (library, variants, build space, pages).
4. The distribution of team differences across verified teams (library and one-pass separately): how many within 0.1%, 0.25%, 0.5% and 1%, the mean, and the largest five each way.
5. Group B's table, in full.
6. A link to the full `approval.md` (every verified team), and to `manifest.json`.

## 4. What `reference/manifest` writes

Done on 2026-11-03. These change what the release reports, not any score:

- **The character furthest apart among judged characters only.** `scoreOf` takes the worst of every character, including those below 10% of the team's DPS, whose verdicts are reported but not counted (`Metric.Minor`). A support with 2% of the team's damage can show −4% and look like a failure. Show the worst judged character, and the worst minor one apart from it.
- **Mark settled verdicts** (`Metric.Settled`): `Score.Settled`, and `Team.Settled` counts the team's settled builds. Each verified team has `group` and `group_why` in `manifest.json`, and the manifest records `group_a_limit` and the drift of each kind (`drift_by_kind`).
- **Write the summary page** (`-summary approval-summary.md`, also the job summary, which `approval.md` at about 6,000 rows would overflow: GitHub keeps 1 MiB of it) and keep `approval.md` as the full table, sorted by group and then by largest difference.
- **Write group A's candidate lines** (`-candidates approved-candidates.txt`: team ids only, no dates). Gscouter's maintainers add them to `approved.txt` with the date of the owner's answer, and only after it.

## 5. Decisions for the owner

1. **A batch answer.** Does one yes for group A, under the rule in §2, count as the owner's answer for each of its teams, recorded per team with that date? Without it the owner answers about 3,000 teams one by one.
2. **The 0.5% line** for group A (`groupALimit` in `reference/manifest/summary.go`), or another line within the bound (for example 0.3% for a smaller batch and a longer group B).
3. **Later releases.** A new module changes the scores of the teams on it (Gscouter `docs/engine.md` §3), so pinning one is the owner's call. Proposed: a new release carries forward the approval of each team that is still verified and still in group A, and shows only the teams that left group A or are new. The other option is to ask for every team again at each release.
4. **The `/updates` entry** for scores that move (roadmap §4.1): one entry for the first batch, then one per release.
5. **Fillers that flip** (roadmap §4.2): a team whose filler candidates are within 1% of each other can show another rotation on the engine, with nearly the same score. Accept it, or keep such teams in group B (this needs the page comparison to report how close a page's best candidates are, which it does not yet).
