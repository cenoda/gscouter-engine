# Verification

> Status: In use (rules of the owner, 2026-11-02 and 2026-11-04) · Last updated: 2026-11-04

How the engine is checked against gcsim: what `reference/compare` decides and how, the probes, CI and its caches, and the release that Gscouter pins. How a team then moves to the engine in Gscouter is `docs/phase4-approval.md` and Gscouter's `docs/engine.md`.

## 1. What is verified

- **The reference** is Gscouter's fork of gcsim at the commit `reference/go.mod` fixes (`cenoda/gcsim@eb21884`), run on the looped config Gscouter assembles for each library team (`cmd/gscouter -dump`).
- **The bound**: team DPS, and the DPS of each character with at least 10% of the team's (`-char-share`), within ±1% of gcsim. It is never loosened. A character below 10% is shown, not judged: what it does reaches the team's DPS.
- **What is compared** (owner, 2026-11-04): each library team's library build, and the probes (§3). Nothing more. The variants (stats moved), the build-space variants (one member's constellation, refinement or weapon changed) and the page configs are no longer verified and have no schedule; their code stays for manual runs (§5).
- **What support means**: any build a player enters (any constellation, weapon, refinement, set, stats) for content the engine has. Content the engine lacks is an error naming it (`gse --check`), and Gscouter runs such a config on gcsim.
- **The drift**: the mean difference over every build compared, in each report and in the release's `approval.md`. It must stay within ±0.2%: an error the whole engine shares a little of, which no single verdict shows.
- **Accuracy is Gscouter's, not finer** (owner, 2026-11-02). The site scores 50 iterations, about 0.7% standard error. Do not spend runner time on differences below that noise.

## 2. How `compare` decides

- A build runs 4,000 iterations (`-iter`; about 0.1% on the team's difference). One that fails or is inconclusive grows 2 to 4 times a step, up to 32,000 (`-max-iter`), since a team whose energy waits spiral is heavy-tailed. An interval still across the bound there is decided by its estimate (`-settle`, marked settled).
  - Before 2026-11-02 every character's standard error had to reach 0.3% and an inconclusive build grew to 512,000 iterations or more. `-target-se 0.003 -max-iter 512000 -settle=false -char-share 0` gives that back.
  - `-max-iter` bounds gcsim's iterations, not the engine's. With `-settle=false`, a build still inconclusive grows on as far as the cache holds gcsim's results for its config, so only the engine runs past `-max-iter`.
- It passes a build when the 95% interval of each judged difference lies within ±1%, and exits with 1 when a comparison fails or stays inconclusive.
- **both-error**: a build both engines refuse to run is agreement, not a difference. Each engine draws its own fights, so a rotation that errors in only a few of them (a condition read the frame before the state it tests ends: Mavuika's recast in `mavuika-citlali-xilonen-bennett` errors in 8 of 16,000 gcsim fights) counts as both-error when the engine, on seeds 1 to `-max-iter`, hits an error with the same message. The report gives both messages and their seeds; otherwise it stays an error.
- **idle**: a build whose team DPS is below 10 in both engines does nothing in the scored window in either (Skirk-led rotations that never clear `First_Rot_Nuke` wait the whole window for a burst). It passes, stays out of the drift, is listed in the report's "Idle" section, and keeps its team unverified until the rotation is fixed in Gscouter's library.
- DPS under 1e-6 on both sides is equal (gcsim leaves 2e-12 where the engine has 0).
- The report gives, beside the verdicts, waits by reason and hits and damage by ability, as Markdown and JSON. Look at those before trusting a quick check.

Selecting what to run:

- `-only library,variants,buildspace,pages,probes` picks the kinds; `-shard k/n` the k-th of n parts of the teams (the census still covers every team); `-teams id,...` some teams; `-builds team/build,...` the builds named as the report names them (`team/library`, `team/probe:kuki-c6`, `team/er-0`, `klee-fischl-xingqiu-bennett/cons:xingqiu=C0`), of any kind, naming any it cannot find.
- `-dry-run` lists the builds without comparing them; `-configs <dir>` writes each selected build's dumped config to `<dir>/<team>-<build>.txt`, for `gse --check` and `go run . -config ... -abil` by hand.
- `-census` classifies every team (supported, or blocked by what) and runs nothing. Its list of blockers was the order of porting.
- `-sample n` runs at most n variants, build-space builds and page configs per team (by a hash of team and build), 4 in CI.
- `-deadline <duration>` stops starting and growing builds after that long and reports the rest as deferred (no verdict, not a failure), so a CI job saves its report and gcsim's chunks before the runner's limit.
- `-gcsim-only` runs gcsim alone into the cache, on teams the engine does not support too; `-seeds lo:hi` runs exactly iterations lo to hi (multiples of 500) of each selected build.

## 3. Probes

A library build checks only the constellations, weapons, refinements and sets it carries (Lisa's are 19 of 21 at C6). What no library build reaches is checked by a probe: a library team with some lines changed. Every constellation of a ported character, refinements 1 and 5 of a weapon and two and four pieces of a set each have one; there are 918 (2026-11-04). Code no comparison reaches is unverified, whatever its source: say so, and add the probe that reaches it. What the probes of the last weapons and sets cannot reach (kills, enemy HP and attacks, range, stamina and others) is listed in `docs/roadmap.md` §3.

A probe is `reference/probes/<name>.txt`, reported as `probe:<name>`:

- Comments start with `#`; the first is its description. One line `team <id>` names the library team.
- Build lines: a character's `char` or `add weapon` line replaces its own line of the team's `build.txt`; its `add set` or `add stats` lines replace all of its set or stats lines.
- Rotation hunks: one or more `rotation- <line>` lines followed by zero or more `rotation+ <line>` lines. The `-` lines must match consecutive lines of the team's `rotation.gcsl` (leading and trailing space aside) in exactly one place, and the `+` lines take their place, indented as the first of them. Hunks apply in order. They reach actions no library rotation uses (Wriothesley's fifth normal attack, Zhongli's charged attack).
- A build line with nothing to replace, or a hunk that matches no place or several, stops the runner with an error naming the probe; a probe never compares the unchanged build.

A probe's team need not be supported, only the probe's build: a new character can be checked on its own library team with the content the engine lacks swapped out.

```
go run ./compare -gscouter ../../Gscouter -teams <id> -only probes -dry-run -configs /tmp/probes
```

## 4. CI

The verdict is CI's (`.github/workflows/compare.yml`), with Gscouter at a fixed commit. Every push to `claude/**` and `main` compares the library builds and probes in 32 parallel parts.

- **gcsim's cache.** gcsim's results are cached by config and chunk of 500 seeds (`reference/harness/cache.go`; `-cache <dir>`, `~/.cache/gscouter-engine/gcsim` by default, empty for none). gcsim is fixed and a seed always gives the same fight, so only the engine runs again after an engine change. Each job restores the cache and uploads what it added; a last job merges them, one at a time per branch. A branch's cache is its own plus `main`'s. Raise `version` in `cache.go` when the harness changes what it measures; a new gcsim (`go.sum`) starts afresh by itself.
  - gcsim's results for every library build and probe are in `main`'s cache, computed by batch servers (`reference/warm-host.sh`, `reference/warm-daemon.sh`, now stopped) and imported with `import_release`. A new probe or changed build runs gcsim in CI.
  - A session's container starts with an empty cache, so a full `compare` there runs gcsim from nothing. Do not run one locally to decide a verdict.
- **Reused verdicts.** Every run with a cache stores each build's verdict (`verdicts/`) with a fingerprint: the hash of the engine's whole output on its first 500 seeds, on 4 threads. On a work branch (`-reuse`), a build whose fingerprint has not moved takes the stored verdict, so a push that changes one character runs in full only the builds whose fights it moves (`reference/compare/verdicts.go`). `main` and releases run every build in full. Settled and inconclusive verdicts are not stored; raise `verdictVersion` when compare decides differently on the same results. A change that moves no bit of those 500 fights and only later ones shows on `main`'s run.
- **The engine's own regression** (pilot, 2026-11-05): `.github/workflows/regression.yml` runs every library build and probe on the engine alone against a committed baseline, beside this gate, which it does not change (`docs/regression.md`).
- **Manual runs** (`workflow_dispatch` inputs; every whitespace character is dropped, so a pasted list may end in a line break):
  - `parts`, `buildspace`, `pages`: the variants (`0-39`), build-space (`0-79`) and page (`0-79`) parts.
  - `warm`: gcsim alone into the cache for some teams, so that a port's push runs only the engine.
  - `rerun`, `rerun_max_iter` (2,048,000), `rerun_shards`: one job per named build, to look closer at a settled verdict; with shards above 1, gcsim's seeds are split over jobs first.
  - `import_run`, `import_release`: merge another run's gcsim chunks, or the assets of a `gcsim-cache-<date>` prerelease, into this branch's cache.
  - `release`, `release_kinds`, `release_teams`: a release (§6).

## 5. Variants, build space and pages (not verified)

Kept for manual runs; no schedule since 2026-11-04.

- **Variants** (`-only variants`): the library build with its stats moved (ER from artifacts 0, half, +30%; crit ±; ATK, HP, EM), and forced variants whose bursts are used without readiness checks, so that energy waits are compared too.
- **Build space** (`-only buildspace`, `reference/compare/buildspace.go`): one axis of one member changed: each other constellation 0 to 6 (the char line's `cons`, so gcsim adds C3's and C5's talent levels), refinement 1 and 5, and other weapons of the class (`-buildspace-weapons`, 2 per member by default, chosen by a hash; 0 for all). A build the engine cannot run counts as unsupported with what it lacks.
- **Pages** (`-only pages`, `reference/compare/pages.go`): every rotation and filler candidate a result page's search (Gscouter's `steady.Best`) tries, and the one-pass config of its pick, for the library build and for no ER from artifacts. `gse-pages` (`pages.go.in`, built inside Gscouter's module with `go build -overlay`) runs that search on gcsim and prints the configs.

Their last complete runs, and the builds that needed millions of iterations, are in `docs/history.md`.

## 6. Releases

A manual run of `compare.yml` on `main` with `release=true` runs the library builds and probes at one commit (`release_kinds`, `library` by default; `release_teams` for some teams, ids or a file such as `reference/release-batch-1.txt`), builds the WebAssembly module and writes, through `reference/manifest`:

- `manifest.json`: the teams Gscouter may run on that module, named by its SHA-256. A team is verified when the census counts it supported and its library build passes (with every build of any other kind the release ran). Anything else keeps it unverified, and the manifest says why. It records the kinds judged (`kinds`, each team's `verified_kinds`), each team's group, and the drift by kind.
- `approval.md`: every verified team's library build, gcsim against the engine, with the judged character furthest apart and the minor one apart from it, sorted by group.
- `approval-summary.md` (also the job summary): the page the owner answers. Groups A and B, the approvals that carry forward or drop (it reads Gscouter main's `approved.txt`), unverified teams by reason, the drift, how the differences spread.
- `approved-candidates.txt`: group A's ids without dates, which go into Gscouter's list only after the owner's answer.

The module, the manifest and the reports are the artifact `engine-release`. The groups and the owner's decisions are in `docs/phase4-approval.md`; pinning a release in Gscouter is Gscouter's `docs/engine.md` §3.
