# gscouter-engine

A Rust port of gcsim, written only for Gscouter (https://github.com/cenoda/Gscouter). The goal is Gscouter's scores within 1% of gcsim, much faster than gcsim. Bit-exact agreement is not a goal. `README.md` has the current results and how the engine works. `docs/roadmap.md` has the phases and where things stand.

## Rules

- Do not modify Gscouter or the gcsim fork. Clone them, read them and run them; all work and pushes go to this repo.
- The reference is Gscouter's fork of gcsim at the commit Gscouter pins. `reference/go.mod` fixes it (`cenoda/gcsim@eb21884`). Port from that source, not from memory of gcsim.
- Accuracy bar: team and per-character DPS within 1% of gcsim, decided by `reference/compare`, never loosened. Report waits beside the DPS.
- Anything the engine lacks is an error that names it (`gse --check`). The engine never silently does something different.
- A change that should not move scores must keep them to the last digit. At 1,000 iterations:
  - `configs/team-looped.txt` gives 72,266.73.
  - `configs/hutao-solo-looped.txt` gives 24,607.03.
  - The same seed must give the same result, so keep the order of random draws.
- Speed is the point of the engine. Keep the hot path free of allocation and dynamic dispatch, and measure. Native, one thread, 500 iterations of `team-looped.txt` takes about 1.47 s.
- Write documents, comments and commit messages in English. Talk to the owner in Korean.
- The default branch is `main`. The owner has had pushes go straight to `main`.

## Working on it

- Commands are in `README.md`, under "Running": build, the engine, the gcsim harness, the comparison runner, `gendata` and `dump`.
- Order of work for a new character, weapon or set:
  1. Read its gcsim package.
  2. Take its tables from `engine/src/gcsimdata` (generated; do not hand-edit). Write its logic in `engine/src/chars/` or the weapon or set modules, on the hooks in `engine/src/content.rs`.
  3. Run `compare` on every library team that becomes supported.
- What blocks the most library teams is the census in `compare`'s report. Use it to pick what to port next.
- gcsim behaviour already matched on purpose; keep it matched:
  - Order of work within a frame.
  - Mod expiry: a mod counts in stat sums while its expiry is `> F`, and reads as active while `>= F`.
  - Hitlag extends mods and queues.
  - Gauges are kept per source character.
  - In damage mode, a kill returns before the element attaches.
  - gcsl evaluates both sides of `&&` and `||`, ignores `return` inside loops, and lexes `-` as an identifier character.
- After each phase, update the measurements and status in `README.md` and `docs/roadmap.md`.
