# Verification transition: proposal for step 3b

> Status: Proposal for the owner (Phase 6 milestone 3b, `docs/independence-roadmap.md`). Nothing here is active: `CLAUDE.md`, `docs/verification.md` and `docs/phase4-approval.md` still hold, and "within 1% of gcsim" stays the gate until the owner approves this and the replacement runs · Last updated: 2026-11-05

What replaces mandatory agreement with gcsim, built on what milestone 3a put in CI, and the decisions it needs from the owner (§6).

## 1. What 3a gives

| Part | Where | What it decides |
|---|---|---|
| Library regression | `regression/`, `docs/regression.md` | Whether any library build or probe moved by one bit, and if so which, by how much |
| Probe coverage map | `reference/coverage`, `docs/probe-coverage.md` | That every constellation, refinement and set count the engine has is reached, or is a gap with a reason |
| Mechanic tests | `engine/tests/mechanics.rs`, `docs/mechanics-tests.md` | Whether a mechanic agrees with public evidence, within the evidence's rounding |
| Arlecchino investigation | `docs/arlecchino/report.md` | The workflow on a reported discrepancy: no damage error reproduced; the site's "about 20%" came from a looped rotation, and the note was removed |
| `compare` against gcsim | `reference/compare` | Today's gate; afterwards a diagnostic |

## 2. The proposed gate

A change to the engine passes when, in CI:

1. **Tests**: `cargo test` (with the mechanic tests), the content validator, the parse check, native/Wasm parity.
2. **Regression**: no build moves, or every build that moves is in the reviewed baseline diff committed with the change (§3).
3. **Coverage**: the map is current; nothing unexplained.
4. **gcsim, as an aid**: `compare` still runs on the library builds and probes. A difference above 1% is no longer a failure by itself, but it must be listed in the change's record (intentional, with its evidence) or it blocks the change as unexplained. A change meant to keep scores keeps them to the bit (regression), so for such a change gcsim cannot move either.

The two reference scores stay to the last digit unless an approved correction moves them, and the drift rule (±0.2%) is replaced by the regression: a change either moves nothing or names what it moves.

## 3. A change that moves scores

Only one mechanic at a time (step 6), with:

- the mechanic, the evidence and its kind (official, measured, stated, community), and the mechanic test that now agrees with it;
- the regenerated baseline and its diff (which teams and characters moved, by how much, largest first), checked to move only what the mechanic reaches;
- an affected-team report (as `docs/arlecchino/report.md` §6 does): teams moved, by how much, which change rank or recommendation;
- gcsim's difference before and after, recorded, not judged;
- the owner's approval of the correction, then a release, and a `/updates` entry.

The baseline is never regenerated to make CI pass; an unexplained move is a bug.

## 4. Releases and approval

Proposed in place of group A and B against gcsim (`docs/phase4-approval.md` §2):

- **The manifest binds** the module SHA-256, the baseline version (its hash), the inputs' hash and the Gscouter commit. Module identity and native/Wasm parity stay as they are.
- **Teams whose regression lines are identical** to the release the owner last approved carry forward without being asked (the same rule as group B's carry-forward of 2026-11-05, extended to every team).
- **Teams moved by an approved correction** are approved with the correction: the owner answers the correction once, with its affected-team report, not team by team.
- **Newly supported teams** (a new port, existing content): verified as now, against gcsim, while gcsim has that content; the owner's batch yes as today.
- **New content gcsim lacks** (new characters, last, engine only): the provisional-result policy of `docs/independence.md` §7.2 (1% per hit against public numbers, 5% provisional DPS target, estimate sensitivity), labelled on the site.
- **The engine becomes the default** for every supported team once this gate is active; a team stays on gcsim only when the engine refuses its config (`gse --check`), until step 5.
- **Rollback**: Gscouter keeps the previous pinned module; a release whose correction proves wrong is reverted by pinning the previous module, and the correction goes back to step 3.

## 5. The pilot correction 3b still needs

The roadmap requires "an approved pilot demonstrating a justified correction" before 3b, and another one if Arlecchino's investigation stayed unresolved. It found no damage error, so it cannot be that pilot. The candidate with the best evidence so far is **Xiangling's Pyronado spin count at C0**: the engine (as gcsim) gives 9 hits in 10 s, KQM's guide states 8 standing still and a frame-count sheet measured 8 cycles (`docs/pilot-xiangling/hidden.md` §2, §3 B: about −3.4% team and −12% Xiangling DPS on C0 builds, none at C4+). It is recorded as a known difference in the mechanic tests. A Test Run recording of one C0 Pyronado (counting the spin hits on a standing target) would settle it.

## 6. For the owner to decide

1. The gate of §2: regression, coverage and tests decide; gcsim differences are recorded and explained, not failures.
2. Score-moving changes as §3: one mechanic, evidence, affected-team report, owner's approval per correction.
3. Approval as §4: carry-forward for unchanged teams, approval per correction, the engine as the default.
4. The pilot correction of §5 (Xiangling C0 spin count), and whether a Test Run recording should settle it first.
5. Whether the regression job becomes required for merging now, during the pilot (it costs about 6 minutes per push).

Until these are answered and the replacement has run on `main`, nothing changes: `CLAUDE.md`'s 1% rule, the drift rule and `docs/phase4-approval.md` stay in force.
