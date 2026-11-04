# Phase 4: how the owner approves teams

> Status: decided by the owner on 2026-11-04 (§5; verification is the library build, §5.6); the groups are written by `reference/manifest` (§4) · 2026-11-04

A library team moves to the engine in Gscouter only with the owner's yes for that team (`CLAUDE.md`), recorded in Gscouter's `internal/engine/approved.txt` with the date of the answer (Gscouter `docs/engine.md` §2). A release (`compare.yml` with `release=true` on `main`) writes `manifest.json` and `approval.md`. `approval.md` has a row per verified team for its library build (and one for its one-pass page config, `page:once`, when a release also ran the pages), with gcsim's and the engine's team DPS, the difference with its 95% half-width, and the character furthest apart, plus the drift over every build compared.

With about 3,000 verified teams that table is thousands of rows, too long for anyone to read and answer team by team. This document sets out how it is put to the owner without loosening the bound: every team offered has already passed `compare` at 1%. The groups below only decide how closely the owner looks at each team.

## 1. What a verified team already means

Verification is the library build and the probes, within 1% of gcsim, and nothing more (the owner, 2026-11-04, §5.6). The manifest marks a team verified when the census counts it supported and its library build passes: team DPS and every character with at least 10% of the team's DPS within 1% of gcsim. The probes check content (each constellation, refinement and set count that library builds do not carry) and run with the library builds. The drift over every build compared must stay within ±0.2%. A team that failed, stayed inconclusive, errored or was idle is not offered. The owner is not asked to judge the 1% bound again, only whether a score that moves by less than 1% (roadmap §4.1) is acceptable for each team.

What a verified and approved team runs on the engine is any build a player enters, as long as the engine has its content (`gse --check`; Gscouter sends a config the engine refuses to gcsim). Variants, build space and pages are not part of the verdict. A release may still run them (`release_kinds`), and a team then needs them too.

## 2. The answer: one batch, then the exceptions

**Gate, before anything is offered.** The drift is within ±0.2%. If it is not, nothing is offered: the whole engine leans, and the cause is found first (`approval.md` already warns).

**Group A: approve at once.** A verified team is in group A when:

- the library build's team difference is within ±0.5% (point estimate), and so is the one-pass page config's when the release ran the pages,
- no build of the team the release judged has a *settled* verdict (decided by its estimate at 32,000 iterations with the interval still across the bound), and
- every judged character (at least 10% of the team's DPS) is within ±1% by its estimate.

The owner answers group A with one yes, naming the release (engine commit and module SHA-256). That yes is recorded as one line per team in `approved.txt`, each with the date of the answer, under a comment that names the release and this rule. The owner sees the summary below and can open the full table, but is not asked to read it row by row.

**Group B: listed one by one.** Every other verified team: a difference between 0.5% and 1%, a settled verdict, a judged character outside 1% by its estimate, or, when the release ran the pages, no one-pass score. How many is known only from the release (a team whose energy waits spiral is often settled somewhere in its builds); listed sorted by the largest difference first, each with both scores, the character furthest apart and the builds that passed. The owner approves each team, approves a list, or leaves teams on gcsim. A team left out stays on gcsim and can be offered again by a later release.

**Not offered.** The unverified teams, counted by reason (failed, inconclusive, errored, idle, a library build the engine cannot run, no result), so that the owner sees what is left and why.

The 0.5% line is half the bound, which is about where the site's own noise lies (50 iterations, about 0.7% standard error). A team in group A moves by less than a reload of its own page does today. The line only sends teams to closer review. It never lets a team through that `compare` failed.

## 3. What the owner is shown

One page in place of the full table:

1. The release: engine commit, module SHA-256, Gscouter commit.
2. Counts: the approvals given before that still hold and those this release drops (listed with why, §5.3), then supported, verified, group A and group B to answer (teams not approved before), unverified by reason.
3. The drift, with its interval, over every build and over each kind (library, variants, build space, pages).
4. The distribution of team differences across verified teams (library and one-pass separately): how many within 0.1%, 0.25%, 0.5% and 1%, the mean, and the largest five each way.
5. Group B's table, in full.
6. A link to the full `approval.md` (every verified team), and to `manifest.json`.

## 4. What `reference/manifest` writes

Done on 2026-11-03. These change what the release reports, not any score:

- **The character furthest apart among judged characters only.** `scoreOf` takes the worst of every character, including those below 10% of the team's DPS, whose verdicts are reported but not counted (`Metric.Minor`). A support with 2% of the team's damage can show −4% and look like a failure. Show the worst judged character, and the worst minor one apart from it.
- **Mark settled verdicts** (`Metric.Settled`): `Score.Settled`, and `Team.Settled` counts the team's settled builds. Each verified team has `group` and `group_why` in `manifest.json`, and the manifest records `group_a_limit` and the drift of each kind (`drift_by_kind`).
- **Write the summary page** (`-summary approval-summary.md`, also the job summary, which `approval.md` at thousands of rows would overflow: GitHub keeps 1 MiB of it) and keep `approval.md` as the full table, sorted by group and then by largest difference.
- **Write group A's candidate lines** (`-candidates approved-candidates.txt`: team ids only, no dates). Gscouter's maintainers add them to `approved.txt` with the date of the owner's answer, and only after it.

## 5. The owner's decisions (2026-11-04)

The owner answered the questions this section asked, on 2026-11-04 (KST):

1. **A batch answer: yes.** One yes for group A, given on a release's summary page, is the owner's answer for each of its teams. It is recorded as one line per team in Gscouter's `approved.txt`, dated the day of the answer, under a comment that names the release (`pinengine -approve-a <date>`, run only after the answer).
2. **The group A line: 0.5%**, as proposed (`groupALimit`).
3. **Approvals carry forward until a problem appears.** A team approved from group A stays approved across later releases without being asked again, as long as it stays verified and in group A. A problem is a release in which the team is not verified (it failed, stayed inconclusive, errored, was deferred, idle or unsupported) or has left group A (a difference above 0.5%, a settled build, a judged character above 1%). From that release on the team runs on gcsim, and it comes back to the owner before it runs on the engine again, whatever a later release says. A team the owner approves one by one from group B is approved for that release only (`release=<module>` on its line), since it was never in group A. That last rule is this document's reading of the answer, not the owner's own words. How it works:
   - Gscouter's `engine.Dropped` decides whether an approval holds for a release. The server runs a team whose approval does not hold on gcsim.
   - Pinning a release (`pinengine`) takes the dropped approvals off the list, each with a comment saying why, so that a later release cannot bring them back on its own.
   - The release's summary page reads Gscouter main's `approved.txt` (`manifest -approved`). It counts the approvals that still hold, lists the ones the release drops and why, and leaves the teams already approved out of the groups to answer.
   - A team whose library files changed since the release is not dropped: it waits on gcsim until a release verifies its new files (Gscouter `docs/engine.md` §3).
4. **The `/updates` entry: required.** Gscouter's `/updates` gets an entry when teams move to the engine. It is drafted on Gscouter's `claude/engine-integration` and goes out with the merge the owner approves, then one entry for each release that moves teams.
5. **Fillers: keep the rotation gcsim picks.** When a team moves to the engine, its result page keeps the rotation and filler candidate that gcsim picks today, unless the engine's pick is clearly better (outside the 1% tie window). How Gscouter does it, and how often the two engines' picks differ on the page configs, is worked out in Gscouter (`docs/engine.md`).
6. **Verification is the library build** (2026-11-04, later the same day). A team is verified when its library build and the probes pass compare (within 1%), and nothing more: no variants, build space or pages, before or after a release, and no runs that demote teams. Support means any build a player enters for content the engine has. A library-only release (`release_kinds=library`, the default) is the full release; its manifest records the kinds it judged (`kinds`, each verified team's `verified_kinds`), and groups teams on the library build alone. The groups, carry-forward and drops of §5.1 to §5.3 hold as written: an approval drops when a later release does not verify the team or puts it in group B. Further releases add teams as their ports land.
