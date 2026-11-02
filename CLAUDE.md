# gscouter-engine

A Rust port of gcsim, written only for Gscouter (https://github.com/cenoda/Gscouter). The goal is Gscouter's scores within 1% of gcsim, much faster than gcsim. Bit-exact agreement is not a goal. `README.md` has the current results and how the engine works. `docs/roadmap.md` has the phases and where things stand.

## Rules

- Do not modify the gcsim fork. Clone it, read it and run it.
- Gscouter may be changed for Phase 4 (putting the engine into it), on its own `claude/...` branches, never pushed to its `main` without the owner (allowed by the owner on 2026-11-02). Follow Gscouter's own `CLAUDE.md` there. Everything else goes to this repo.
- A team moves to the engine only with the owner's answer for that team. Moving changes its score (within 1% of gcsim, not bit for bit), so before a team runs on the engine in Gscouter, show the owner its gcsim and engine scores and wait for a yes; a team without one stays on gcsim, whatever the manifest says. Keep the approved teams in a list in Gscouter that the code reads, with the date of each answer.
- The reference is Gscouter's fork of gcsim at the commit Gscouter pins. `reference/go.mod` fixes it (`cenoda/gcsim@eb21884`). Port from that source, not from memory of gcsim.
- Scope: the engine scores a player's showcase build (any constellation, weapon and refinement, set, talent levels, stats) on a library team's rotation. Library builds are test data, not the scope; `docs/roadmap.md` §2 says what done means.
- Accuracy bar: team DPS, and the DPS of each character with at least 10% of the team's, within 1% of gcsim, decided by `reference/compare`, never loosened. Report waits beside the DPS.
- Verification is at Gscouter's accuracy, not finer (owner, 2026-11-02): the site scores 50 iterations (about 0.7% standard error), so `compare` runs 4,000 iterations per build, up to 32,000 when a build fails or is inconclusive there, and decides an interval still across the bound by its estimate. Variants, build space and pages run a sample per team (`-sample 4` in CI). Errors that add up over a team's items show in its team DPS; errors the whole engine shares show in the drift (the mean difference over every build compared, in each report and in `approval.md`), which must stay within ±0.2%. Do not spend runner time on differences below the site's noise.
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
  1. Read its gcsim package, all of it.
  2. Take its tables from `engine/src/gcsimdata` (generated; do not hand-edit). Write its logic in `engine/src/chars/` or the weapon or set modules, on the hooks in `engine/src/content.rs`. Port all of it, not only what the library's builds and rotations use: a character C0 to C6 (`MAX_CONS = 6`) with every action and param gcsim's package has, a weapon refinements 1 to 5, a set two and four pieces. What is left out stays an error that names it.
  3. Run `compare` on every library team that becomes supported, and verify what those builds do not carry: each constellation, refinements 1 and 5, two and four pieces, on a supported team (the runner's build-space variants once they exist, `reference/probes` until then).
  4. It is done when all of that passes, not when its library teams do.
- What blocks the most library teams is the census in `compare`'s report. Use it to pick what to port next, but measure progress also by the share of real players' builds the engine runs and has verified. Systems (Phase 2) and content both go in parallel branches under the conditions below, merged and compared again together.
- gcsim behaviour already matched on purpose; keep it matched:
  - Order of work within a frame.
  - Mod expiry: a mod counts in stat sums while its expiry is `> F`, and reads as active while `>= F`, unless gcsim has dropped it: the snapshot's `Stats()`, `ApplyAttackMods` (not for reaction damage), `ReactBonus` and `HealBonus` delete the mods of their kind whose expiry is `<= F`, so a mod at its expiry frame is inactive once one of them ran on that frame (`Char::pruned`; Raiden's eye is read so). `Stat`, `SelectStat`, `TotalAtk` and `TotalDef` drop nothing.
  - Hitlag extends mods and queues.
  - Gauges are kept per source character.
  - In damage mode, a kill returns before the element attaches.
  - A wait counts whole under the reason its action first failed for, as gcsim's stats do.
  - gcsl evaluates both sides of `&&` and `||`, ignores `return` inside loops, and lexes `-` as an identifier character.
- Pitfalls found so far; check for them in new code:
  - A param, field or constellation the engine does not handle must be an error, not a default. Found silently wrong: constellations above what a character implements (now `MAX_CONS`), Yelan's `breakthrough`, `.char.infusion.<key>` (was always false).
  - Lists of reactions, swirls, attack tags or elements must cover every one the engine has (VV's swirl bonus missed cryo swirl; Raiden's Eye did not ignore burning damage). Grep for the existing variants, and gcsim's ported packages for the new ones, when adding one.
  - Content written before a system existed assumes it never happens: Sapwood Blade did nothing until dendro reactions, The Alley Flash ignored hits on the player. When adding a system, grep the modules' comments for "never", "none of which", "no fight", "does nothing".
  - Gadgets see an attack as it was before the enemy took it (gcsim hands each target its own copy, before ICD and reactions), and only through a hit pattern (`Pattern`). An attack without one that could reach a dendro core or Lynette's Bogglecat Box is an error; give patterns from gcsim's `combat.New...Hit` calls. A gadget that reacts to hits is added with `add_hittable_gadget` and handled in its owner's `gadget_hit`.
  - Geometry sits on exact boundaries: a dendro core is exactly the enemy's radius + 0.5 from its center, so a circle of radius 0.5 on the enemy touches it at exactly the sum of the radii. Keep coordinates in f64 and compute them as gcsim does (in f32, Fischl's A4 reached such cores half the time against gcsim's two thirds).
  - gcsim's ApplyAttackMods skips reaction damage (`AttackTag::is_reaction`); a crit-rate mod on a reaction makes it crit, and crits feed Favonius.
  - Absorb checks read the enemy, then gadgets, then the player, and only what gcsim's check pattern reaches (`Sim::absorb_check_in`; Sucrose's burst does not reach the player or Guoba).
  - gcsim has special cases away from the obvious code: burning's ICD is checked for every character, not only the attacker (pkg/enemy/attack.go). When per-ability damage differs with equal hits, trace one gcsim fight with its debug log (`simulation.NewCore(seed, true, cfg)`, then `c.Log.(*glog.Ctrl).Dump()`) beside the engine.
  - Stellar-Conduct and Polestar Fields are ported with Sandrone, and Radiance with them, but no supported library team has Sandrone, so CI compares none of it (only hand-written teams were checked). Stellar Swirl is still an error; Shenhe's C2 and Escoffier's C1 Stellar parts need it. Qiqi reads Radiance but does not create it.
  - The engine works in gcsim's world coordinates: the player has a position and a facing, and a hit lands on the enemy when its shape reaches it, as gcsim's AttackWillLand decides. Build patterns with the helpers that mirror gcsim's calls (`circle_on_player`, `box_on_player`, `fan_on_player`, `circle_on_target`), at the moment gcsim builds them. A character joins `MOVES_WITH_PLAYER` only once checked against gcsim with a moved player. Compare hits per ability.
  - The engine's modules are listed twice, in `engine/src/main.rs` and `engine/src/lib.rs` (the library Gscouter links): a new top-level module goes in both.
  - A library build checks only the constellations, weapons and refinements it carries (Lisa's are 19 of 21 at C6). Code no comparison reaches is unverified, whatever its source; say so in the report and add the build that reaches it.
  - Before trusting a quick check, look at per-ability hits and damage (`gse` output and the harness's `-abil`), and at waits.
  - A team whose bursts wait for energy can spiral (a wait takes the field time that makes the next burst's energy), so its DPS is heavy-tailed and 1,000 iterations can be 2% off (Klee / Fischl / Xingqiu / Bennett looked −1.78% and passes at 36,000). Let `compare` decide before hunting a cause.
- Verification runs in CI, and the work does not wait for it:
  - The verdict is CI's (`.github/workflows/compare.yml`, which runs on every push to `claude/**` and `main`). A session's container starts with an empty gcsim cache, so a full `compare` here runs gcsim again from nothing; CI restores main's cache and the branch's own, and runs only the engine for configs it has seen. Do not run a full `compare` locally to decide a verdict.
  - Before pushing, check locally what takes seconds: `cargo test`, the two scores above to the last digit, `gse --check` on the configs the change touches, and per-ability hits and damage on one dumped config against `go run . -config -abil` at 1,000 iterations. A screen for gross errors may use `compare -teams <id> -only library -cache ""`; its verdict is advisory, never the result.
  - At the start of a port, dispatch `compare.yml` on `main` with `warm` set to the teams the port will unblock (from the census in `compare`'s report, which lists the teams one or two items from support), so that gcsim runs while the port is written and the push runs only the engine.
  - After pushing, start the next item while CI runs, under these conditions, and come back to a branch when its run fails or stays inconclusive:
    - Every branch starts from `main`, never from a branch not yet verified.
    - At most five branches are unverified at once, so a misreading of gcsim repeats in at most five ports before a verdict shows it.
    - Every branch keeps the two scores above to the last digit before it is pushed, so a break in the core shows at once.
    - A system that changes an existing hook's signature or the order of work within a frame (events, tasks, the frame loop) goes alone: no other system runs beside it, and it is merged before the next one starts. Systems that only add hooks, events or state may run side by side.
    - A branch is merged into `main` as soon as its CI passes, one at a time, without waiting for the owner; the others are brought up to the new `main` before their own merge. The merge is compared again (gcsim is cached by then), and a branch counts as done only when that run passes too.
    - Phase 2 is done when, after its last merge, one run on `main` of the library builds, probes and the whole build space passes every team the engine runs.
- Workflow notes: `compare` runs 4,000 iterations, grows a build that fails or is inconclusive by 2 to 4 times a step up to 32,000, and settles what is still inconclusive there by its estimate (`-settle`; `-target-se` and `-settle=false` give the old growth). gcsim takes about 85% of the time. A manual run of `.github/workflows/compare.yml` (input `parts`: `library`, or `0-39` for every variant) runs in parallel jobs on any branch. gcsim's results are cached by config and seed chunk (`reference/harness/cache.go`), so a rerun after an engine change costs only the engine; raise its `version` when the harness changes what it measures. A branch's CI cache is its own plus `main`'s. Quick checks: dump the looped config with Gscouter's `cmd/gscouter -team <dir> -build <file> -dump`, then run `gse` and `go run . -config` at 1,000 iterations.
- After each phase, update the measurements and status in `README.md` and `docs/roadmap.md`.
