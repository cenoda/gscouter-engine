# Roadmap

> Status: Phases 0 and 1 done, Phase 2 under way · Last updated: 2026-10-30

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

### Phase 1: foundations for porting at scale (done)

What had to be in place before dozens of characters, so that they are not rewritten later (`README.md` has the details and the full tables).

- **Comparison runner** (`reference/compare`, in CI). For each library team it assembles Gscouter's looped config, asks the engine what it lacks, and runs the supported ones under both engines: the library build and nine builds with their stats moved, among them ER-starved builds whose bursts wait for energy. Team and per-character DPS pass when the 95% interval of the difference lies within ±1%; iterations grow until each engine's standard error is at most 0.3% and further, up to 512,000, while a verdict is inconclusive. Waits by reason and per-ability hits and damage are reported beside them, as a Markdown report and JSON.
  - Result: both supported teams and all their builds pass, 20 of 20; the largest team difference is −0.15%, the largest character difference −0.38% ± 0.55 (Yelan, crit +20%).
- **Generated data** (`reference/gendata`). Every character's, weapon's and set's package variables and constants as gcsim computes them (talent multipliers, frames, hitmarks, hitlag, mod keys), every ICD tag and group, base stats and what each character's `NewChar` and `Init` set: all 168 member keys of the library, 252 weapons, 55 sets. The four ported characters take their tables from it; their scores did not move (72,266.73 and 24,607.03 at 1,000 iterations).
- **Core structure** (`engine/src/content.rs`). Characters, weapons and sets are modules on hooks (actions, tasks, attack callbacks, mod amounts, attack mods, events in gcsim's subscription order, frames, conditions), dispatched statically through macro-generated `match`es. The restructure gave byte-identical output and the same speed (+0.8%, 1.47 s against 1.46 s for 500 iterations on one thread).
- **Check of the structure**: Bennett, with Deathmatch, Favonius Warbow, The Alley Flash, Harbinger of Dawn and Instructor, makes `hutao-yelan-bennett-xingqiu` a supported library team (team −0.03%). The core gained only general systems for it: weapon infusion, reaction events, fight-wide statuses, pyro resonance.
- **Progress measure**: the runner's census. 2 of 4,570 teams are supported. The largest blockers: Noblesse Oblige (2,087 teams), Golden Troupe (1,852), Favonius Sword (1,845), Scroll of the Hero of Cinder City (1,493), Favonius Lance (1,376), Furina (1,324), Viridescent Venerer (1,290), Fischl (1,001); electro resonance 961, cryo 774; `pick_up_crystallize` 511.

### Phase 2: the rest of the engine

Progress so far, taken in the order that makes the most library teams supported (`README.md` has the results):

- **16 of 4,570 teams supported** (2 at the start), every build of each passing the comparison runner.
- **Electro, overload and electro-charged** (with Fischl), each reaction emitting its event as gcsim's `Reactable.React` does; reaction damage and `REACTION_LVL_BASE` from gcsim's combat package.
- **HP changes** (with Furina): `Player.Heal` and `Player.Drain` with `OnHeal` and `OnPlayerHPDrain`, overheal, and incoming healing bonus mods. HP debt is still missing.
- **Swirl, the player's aura and electro resonance** (with Kazuha and Raiden): swirl's hit on its own enemy, reaction bonus mods, Bennett's self infusion read by absorb checks; OnEnergyBurst and OnParticleReceived.
- **Content**: Fischl, Furina, Kazuha, Raiden; Noblesse Oblige, Golden Troupe, Cinder City, Marechaussee Hunter, Viridescent Venerer; the Favonius weapons, The Stringless, Ballad of the Fjords, Wolf-Fang, The Alley Hunter, Sapwood Blade, Festering Desire, Slingshot, Dragon's Bane, Xiphos' Moonlight; Zhongli's normal attacks.
- **Probes** in the runner, so that content no supported team carries is compared too.
- Cryo resonance (774 teams) needs cryo characters and the cryo aura and reactions (melt, freeze, superconduct, cryo swirl and crystallize); it comes next, with Rosaria and Escoffier the closest.

- **Reactions.** Overloaded, superconduct, electro-charged, swirl, frozen and shatter, burning, bloom, hyperbloom and burgeon, quicken, aggravate and spread, and the lunar reactions. Only vaporize and crystallize (without shards) exist; reaction events are already emitted, so sets like Instructor work for the new ones once they emit.
- **Resonances.** Electro (961 teams), cryo (774), geo (418), dendro (328), anemo (263) are named errors now.
- **Hexerei.** Two Hexerei characters (394 teams, Fischl with Mona, Sucrose, Razor and others) are an error until a second one is ported.
- **HP changes, for Furina (1,324 teams).** Her fanfare counts every HP change of the party, so she needs drains (`Drain` and `OnPlayerHPDrain`, which the Salon Members cause), `OnHeal` with the healed and overhealed amounts, incoming healing bonus mods (her burst's), and Ousia and Pneuma (Arkhe). HP debt (Arlecchino) belongs with them. Today HP is a ratio changed by heals and Hu Tao's drain, with no events.
- **Systems.** Nightsoul, shields beyond Zhongli's (strength, absorption), gadgets (dendro cores, crystallize shards and `pick_up_crystallize`, 511 teams), positions (`set_player_pos`, 45), stamina mods.
- **Actions of the ported characters** the library uses: Zhongli's press skill (46) and burst (36), Yelan's aimed shots (134), Bennett's charge (19), plunges. Zhongli's normal attack is done.
- **The runner's noisy cases.** Builds that spend most of the fight waiting for energy need up to about 320,000 iterations to reach a verdict and take most of the runner's time. As supported teams grow, run the variants on a schedule and only the library builds on every push.

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
