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
  - Polestar Fields and Stellar-Conduct come from content not ported (Qiqi, Sandrone, Odette, the cryo Traveler and others); Wriothesley's and Yae Miko's Radiance assume they never happen. Port Radiance with the first of them.
  - Most hits are taken to land, but some abilities move their hitbox away (Diluc's burst): compare hits per ability, and use `Sim::box_hits` where gcsim's geometry decides.
  - Before trusting a quick check, look at per-ability hits and damage (`gse` output and the harness's `-abil`), and at waits.
  - A team whose bursts wait for energy can spiral (a wait takes the field time that makes the next burst's energy), so its DPS is heavy-tailed and 1,000 iterations can be 2% off (Klee / Fischl / Xingqiu / Bennett looked −1.78% and passes at 36,000). Let `compare` decide before hunting a cause.
- Workflow notes: `compare` grows iterations until every character's standard error is 0.3%, so a full run of 100 teams' library builds takes hours here; build experiments into another `--target-dir` while it runs. CI is faster: a manual run of `.github/workflows/compare.yml` (input `parts`: `library`, or `0-39` for every variant) runs in parallel jobs on any branch. gcsim's results are cached by config and seed chunk (`reference/harness/cache.go`), so a rerun after an engine change costs only the engine; raise its `version` when the harness changes what it measures. Quick checks: dump the looped config with Gscouter's `cmd/gscouter -team <dir> -build <file> -dump`, then run `gse` and `go run . -config` at 1,000 iterations.
- After each phase, update the measurements and status in `README.md` and `docs/roadmap.md`.
