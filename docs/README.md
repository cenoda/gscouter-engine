# Documents

> Last updated: 2026-11-04

What each document is for. The conventions are Gscouter's (its `docs/README.md`): English, present tense for how things are, dated measurements, records next to their subject, and a header with a status line.

| Document | What it holds |
|---|---|
| `README.md` | What the engine is, its status, what it does not do, how it works, the commands |
| `CLAUDE.md` | The rules for working on it: scope, accuracy, the reference scores, branches and merges |
| `docs/roadmap.md` | Where it stands, what done means, the phases and what remains |
| `docs/verification.md` | How it is checked against gcsim: `compare`'s rules, probes, CI and its caches, releases |
| `docs/porting.md` | How to port from gcsim, the gcsim behaviour matched on purpose, and the pitfalls found so far |
| `docs/phase4-approval.md` | How the owner approves teams for Gscouter: groups A and B, carry-forward, the owner's decisions |
| `docs/independence.md` | The plan to stop depending on gcsim (Phase 6): why, what stays, where data comes from, the steps, what remains |
| `docs/independence-roadmap.md` | Phase 6 execution order, milestone deliverables, completion evidence and unresolved decisions |
| `docs/history.md` | Records of Phases 0 to 3: results tables, runs and measurements as they were taken |
| `docs/rotation-search.md` | A pointer: rotation search moved to gscouter-optimizer |
| `reference/audits/` | Audits of where characters read the player's position, for `MOVES_WITH_PLAYER` |

Gscouter's side of the engine (which teams run on it, the approved list, pinning a release, the server's runtime) is Gscouter's `docs/engine.md`.

## Keeping them current

- A change updates the document it changes, in the same commit: a new port updates `README.md`'s status only when it moves the counts there, a new rule goes in `CLAUDE.md` only when it is a rule, and a pitfall goes in `docs/porting.md`.
- After each release, update `README.md`'s status, `docs/roadmap.md` §1 and Gscouter's `docs/engine.md` together, with the same numbers.
- Records (a run's results, a trace, a measurement no longer current) go to `docs/history.md`, not into the status.
