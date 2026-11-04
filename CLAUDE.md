# gscouter-engine

A Rust port of gcsim, written only for Gscouter (https://github.com/cenoda/Gscouter). The goal is Gscouter's scores within 1% of gcsim, much faster than gcsim. Bit-exact agreement is not a goal. `README.md` has the status and the commands; `docs/README.md` lists the other documents.

## Rules

- **Talk to the owner in Korean.** Write documents, comments and commit messages in English.
- **Do not modify the gcsim fork.** Clone it, read it and run it. The reference is the commit `reference/go.mod` fixes (`cenoda/gcsim@eb21884`): port from that source, not from memory of gcsim.
- **Gscouter** may be changed for the engine's integration on its own `claude/...` branches, never pushed to its `main` without the owner (owner, 2026-11-02). Follow Gscouter's own `CLAUDE.md` there. Everything else goes to this repo.
- **A team moves to the engine only with the owner's answer for that team.** Its score moves (within 1% of gcsim, not to the bit), so the owner sees its gcsim and engine scores and says yes first; a team without one stays on gcsim. The answers are in Gscouter's `internal/engine/approved.txt`, dated; how they are given is `docs/phase4-approval.md`.
- **Scope**: a player's showcase build (any constellation, weapon and refinement, set, talent levels, stats) on a library team's rotation. Library builds are test data, not the scope (`docs/roadmap.md` §2).
- **Accuracy bar**: team DPS, and the DPS of each character with at least 10% of the team's, within 1% of gcsim, decided by `reference/compare`, never loosened. Report waits beside the DPS. The drift (the mean difference over every build compared) must stay within ±0.2%.
- **Verification is at Gscouter's accuracy, not finer** (owner, 2026-11-02, 2026-11-04): each library build and the probes within 1% of gcsim, nothing more. Do not spend runner time on differences below the site's noise. Details: `docs/verification.md`.
- **Anything the engine lacks is an error that names it** (`gse --check`). The engine never silently does something different.
- **A change that should not move scores keeps them to the last digit.** At 1,000 iterations (`gse <config> 1000 150 4`):
  - `configs/team-looped.txt` gives 72,247.45.
  - `configs/hutao-solo-looped.txt` gives 24,589.24.
  - The same seed must give the same result, so keep the order of random draws.
- **Speed is the point.** Keep the hot path free of allocation and dynamic dispatch, and measure. Native, one thread, 500 iterations of `team-looped.txt` take about 1.3 s (2026-11-04).
- The default branch is `main`. The owner has had pushes go straight to `main`.

## Working on it

- Porting a character, weapon, set or system: `docs/porting.md` (order of work, gcsim behaviour matched on purpose, pitfalls). Read it before porting.
- Before pushing, check locally what takes seconds: `cargo test`, the two scores above to the last digit, `gse --check` on the configs the change touches, and per-ability hits and damage on one dumped config against `go run . -config ... -abil` at 1,000 iterations.
- **The verdict is CI's** (`.github/workflows/compare.yml`, on every push to `claude/**` and `main`). A session's container starts with an empty gcsim cache, so do not run a full `compare` locally to decide a verdict. A screen for gross errors may use `compare -teams <id> -only library -cache ""`; its verdict is advisory.
- **Branches**, while CI runs:
  - Every branch starts from `main`, never from a branch not yet verified.
  - At most seven branches are unverified at once (owner, 2026-11-02). A branch may carry several ports.
  - Every branch keeps the two scores above to the last digit before it is pushed.
  - A system that changes an existing hook's signature or the order of work within a frame (events, tasks, the frame loop) goes alone, and is merged before the next one starts. Systems that only add hooks, events or state may run side by side.
  - A branch is merged into `main` once its CI passes, without waiting for the owner. Several passed branches may be merged together and verified by one run on `main` (owner, 2026-11-04): one merger resolves the conflicts (content modules, the module lists in `lib.rs` and `main.rs`, probes), checks `cargo test` and the two scores, and pushes once; no push to `main` while that run goes. If it fails, revert the merge of the branch the failing teams' characters come from, alone, and fix that branch. A branch is done only when `main`'s run passes with it.
- A commit that changes only documents may skip CI with `[skip ci]` in its message.
- **After a release, or a change to what the engine has**, update `README.md`'s status, `docs/roadmap.md` §1 and Gscouter's `docs/engine.md` together (`docs/README.md`).
