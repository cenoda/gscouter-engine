# Roadmap

> Status: Phase 0 done, Phase 1 next · Last updated: 2026-10-30

What it takes for this engine to score Gscouter's results. More characters alone is not enough: the engine also needs every weapon and set a player can bring, the rest of gcsim's systems, a way into Gscouter's code, and rules for scores that move.

## 1. Why characters alone are not enough

Measured on Gscouter's library (4,573 teams, October 2026) and on gcsim at `cenoda/gcsim@eb21884`.

- **Weapons and sets, all of them.** A library team has fixed builds, but a score runs the player's showcase build, which can carry any weapon and any set. One that is missing and the score cannot run. gcsim has 261 weapons and 55 sets.
- **Characters have a long tail.** The library's teams use 168 member keys (each Traveler element counts once). Taking characters in the order that completes the most teams:

  | Characters ported | Teams the engine could score |
  |---|---|
  | 20 | 302 (7%) |
  | 40 | 1,170 (26%) |
  | 60 | 2,217 (48%) |
  | 100 | 3,939 (86%) |
  | 168 | 4,570 (100%) |

- **Content is most of the work.** gcsim's characters, weapons and sets are about 180,000 lines of Go, its core (`pkg`) about 100,000. This engine is 7,300 lines, core included.
- **Gscouter uses more of gcsim than a DPS number.**
  - Fillers (`internal/score/score.go`) wrap gcsim's action evaluator, subscribe to `OnActionExec` and `OnTick`, and mark points in a run with `delay(987654)`.
  - Timelines read each character's `DamageEvents` and `ActionEvents` and the 0.5 s damage buckets.
  - `internal/score/chasca.go` reads a character's state.
- **Rotations use features the engine lacks.** Among library configs (rotation or source), `pick_up_crystallize` appears in 511 teams, `set_player_pos` in 45, `rand` and `randnorm` in several hundred, and `hurt` in 64.

## 2. Phases

### Phase 0: prototype (done)

Hu Tao / Xingqiu / Zhongli / Yelan and the gcsl parser, within 0.03% of gcsim; about 8× faster native and about 30× faster in WebAssembly (`README.md`).

### Phase 1: foundations for porting at scale

What has to be in place before dozens of characters, so that they are not rewritten later.

- **Comparison runner.** Run each library team's looped config under gcsim and under the engine, and compare team DPS, per-character DPS, waits and per-ability damage against the 1% bound. Run it in CI.
  - Library builds alone are not enough: also compare builds with their stats moved (ER, crit, ATK). Players' builds often lack ER, and the waits that follow decide fillers.
- **Generated data.** Take the talent multipliers, frames, hitlag and ICD data from gcsim's generated files and catalog through `reference/gendata`, so that porting a character is only its logic.
- **Core structure.** Characters are dispatched by `match` today. Make characters, weapons and sets attach to events and mods in one general way, which 168 characters and 316 weapons and sets need.

### Phase 2: the rest of the engine

- **Reactions.** Overloaded, superconduct, electro-charged, swirl, frozen and shatter, burning, bloom, hyperbloom and burgeon, quicken, aggravate and spread, and the lunar reactions.
- **Systems.** Nightsoul, healing and HP changes (Furina's fanfare follows them), shields, gadgets (dendro cores, crystallize shards and `pick_up_crystallize`), positions (`set_player_pos`), stamina, `rand` and `randnorm`.

### Phase 3: content (most of the work)

- **Weapons (261) and sets (55).** Small one by one, but all are needed. Port the ones Enka showcases carry most first.
- **Characters**, in the order of the table in §1: 40 cover a quarter of the library, 100 cover 86%.
- A character is done when every library team with it passes the comparison runner.

### Phase 4: into Gscouter

- **Mixed engines.** A team runs on the engine when every member, weapon and set is implemented and the team passes the comparison runner, and on gcsim otherwise. This lets the engine ship before Phase 3 ends, so Phase 4 can start partway through it.
- **Browser.** Only the helper workers that run simulation chunks switch to the engine's WebAssembly. The lead worker, which runs the fillers and renders, stays in Go.
- **Server.** Go calls the engine through a cgo static library or through the same WebAssembly on wazero. Results stay identical between browser and server (iteration `i` uses seed `base + i`), as they are now.
- **Filler hooks.** Wrapping the evaluator, detecting waits, markers and action events.
- **Results.** The part of gcsim's `stats.Result` Gscouter reads: DPS per character, damage buckets, damage and action events, failures and warnings.
- **Cache key.** Includes the engine and its version.

### Phase 5: upkeep

- **Following gcsim.** Every gcsim update (game patches, fixes) is ported too, then the comparison runner reruns.
- **New characters.** They go into both Gscouter's gcsim fork and the engine. That double cost is new.
- **Source validation** (`cmd/validate`) stays on gcsim. It runs each entry under its source's conditions (several targets, other enemies, `hurt`), which scoring does not need.

## 3. Decisions for the owner

1. **Scores that move.** Gscouter promises verified scores unchanged to the last bit across fork updates. The engine matches gcsim statistically, not bit for bit, so a team that moves to the engine changes its score by up to about 1%. Allow it, with an entry in `/updates`?
2. **Fillers can flip.** A filler is the best of several runs. When two candidates are within 1%, the two engines can pick different ones: the score barely moves, but the rotation the result shows does.
3. **Two engines to keep.** Over time this is the largest cost.

## 4. What it gains

- **Faster result pages.** A result page takes 10–24 s today, most of it simulation (Gscouter `docs/speed.md`). Most of that would go, leaving download and worker start.
- **More simulation in the same time.** Hundreds of iterations instead of 50, or a wider filler search.
- **Faster library validation.** It takes about 40 minutes today. At the same ratio it would take minutes. This is estimated, not measured.

## 5. Recommended order

Phases 1 and 2, then in Phase 3 the top 20–40 characters and the common weapons and sets. Then add Phase 4's mixed engines and measure the gain on real pages before porting the rest.
