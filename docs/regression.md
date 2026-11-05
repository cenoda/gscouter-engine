# Library regression

> Status: Pilot (Phase 6 milestone 3a), running in CI beside the active gate, which stays `compare` against gcsim (`docs/verification.md`). Its failure does not yet decide a verdict on its own · Last updated: 2026-11-05

The engine's own regression: every library build and every probe on fixed configs and fixed seeds, run by the engine alone, against expected outputs committed in this repository. It detects change, not truth (`docs/independence.md` step 3): a change meant to keep scores keeps every line, and a change meant to move them carries a reviewed baseline diff.

## 1. What it holds

`regression/`:

- `inputs.tar.xz`: the looped config of each library build and probe, as `compare -only library,probes -dry-run -configs` writes them (`configs/<team>-library.txt`, `configs/<team>-probe-<name>.txt`), and `gscouter-commit`, the Gscouter commit they were assembled at (the one `compare.yml` pins). 5,488 configs (4,570 library builds, 918 probes), 22 MB as files, 399 KB compressed (2026-11-05).
- `baseline.txt`: one line per build, sorted, after a header recording the format, the engine commit the baseline was made from (`+changes`: the working tree's engine differed from that commit, as when the baseline is committed with the change), the Gscouter commit, the hash of the inputs, and the iterations, warmup, threads and first seed. Each line has the hash of the engine's whole output (`sha`: the JSON `gse` prints, its timing left out, with exit status and error text; 128 bits), team DPS and each character's DPS as `gse` prints them (4 decimals), the waits by character and reason, the frames, and the error of a build the engine refuses. 1.9 MB (2026-11-05).
- `src/`: the runner `regress` (crate `gse-regress`, no dependencies): `run`, `diff`, `merge`.
- `update.sh`: regenerates the inputs or the baseline.

The runs are 50 iterations from seed 1, warmup 150 s, one thread per build (the engine's sums depend on how fights are split between threads; one thread is the native order). Determinism makes 50 enough: any change to the code those 2,744,000 fights reach moves the hash. The accuracy of a score is the gate's business, not this one's (owner, 2026-11-02).

## 2. Why the inputs are committed

The runner needs neither Go nor gcsim: it unpacks the configs and runs `gse`. Assembling the configs is Gscouter's (`cmd/gscouter -dump`) and `compare`'s (probes), both Go; committing their output at the pinned commit takes that out of the runner at 399 KB per version, where porting the assembly to the engine would be a second implementation of Gscouter's library format to keep in agreement. The CI job `inputs` still runs `compare -dry-run` (Go, no gcsim run) to check that the committed inputs are current. It is removed, and `update.sh inputs` replaced, once Gscouter itself writes the configs the engine is to score (step 5, when the site runs without gcsim) or the library moves into a format the engine reads.

The inputs change when `GSCOUTER_COMMIT` moves, a probe is added or changed, or a team becomes supported. Each regeneration adds about 0.4 MB to the history (xz does not delta). Should that grow too large, the alternative is the configs as one plain text file, which git deltas between versions.

## 3. Commands

```
regression/update.sh inputs      # Go, and Gscouter at the pinned commit in ../Gscouter ($GSCOUTER)
regression/update.sh baseline    # the engine alone: about 7 minutes on 4 cores
```

`baseline` builds the engine and the runner, runs every build, writes `regression/baseline.txt`, and prints the diff against the previous baseline (kept in `regression/target/baseline-diff.md`). The diff lists the builds that changed, largest team move first: team DPS and each character's (`(minor)` below 10% of the team's), waits, errors, and builds whose hash alone moved (finer than the printed digits, or abilities' hits and damage only); then new and missing builds, and the mean and largest team move.

By hand:

```
regress run --gse engine/target/release/gse --configs <dir> [--iter 50] [--seed 1] [--warmup 150] [--jobs N] [--shard k/n] -o results.txt
regress diff regression/baseline.txt results.txt [--shard k/n]     # exits 1 on any difference
regress merge results-*.txt -o baseline.txt                          # shards into one file
```

A shard is chosen by a hash of the build's id, so it does not depend on the other builds.

## 4. CI

`.github/workflows/regression.yml`, on every push to `claude/**` and `main`, beside `compare.yml`:

- `build`: the engine and the runner (with its tests), once.
- `run`: 8 shards, each running its builds and comparing them with the baseline; the diff goes to the job summary. It fails on any difference, on inputs whose hash is not the baseline's, and on a baseline made with other iterations or seeds.
- `results`: the shards' results merged into one file (artifact `regression-baseline`), the baseline as CI's machines give it.
- `inputs`: the committed inputs are what `compare -dry-run` assembles at the pinned Gscouter commit, and the baseline was made at that commit.

Measured wall times are in §6.

## 5. Review rule

- **A change meant to keep scores** leaves `baseline.txt` as it is, and CI's `run` passes. The two reference scores of `CLAUDE.md` stay as they are.
- **A change meant to move scores** commits the regenerated `baseline.txt` in the same commit as the change, with the diff of `update.sh baseline` (or its summary and the largest moves, when long) in the commit message or a report beside it, and an explanation: which mechanic changed, why, and the evidence. A reviewer checks that every build that moved is one the change reaches, and that nothing else moved. Never regenerate the baseline to make CI pass without that explanation.
- **New inputs** (a pin move, a new probe, a newly supported team) regenerate the inputs and then the baseline; the diff should show only new, missing or changed builds of the configs that changed.
- **A baseline taken from CI** (the `regression-baseline` artifact) is for when the local machine gives other bits than CI's; say so in the commit. Both have agreed so far (§6).
- While this is a pilot, `compare` against gcsim stays the verdict; a score-moving change still needs its library builds within 1% of gcsim and the owner's answer for its teams (`docs/phase4-approval.md`).

## 6. Measurements

- 2026-11-05, local container (4 cores): all 5,488 builds in 408 s (26 CPU-minutes); 666 builds of one shard on 3 jobs gave the same lines as the full run on 4.
