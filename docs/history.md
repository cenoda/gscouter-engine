# History

> Status: Records, not kept up to date · Last updated: 2026-11-04

Measurements and step-by-step records of Phases 0 to 3, moved here from `README.md` and `docs/roadmap.md` on 2026-11-04 so that those two describe the engine as it is now. Each number is as of the date and commits given beside it; later changes (new content, world coordinates, the verification rules of 2026-11-02 and 2026-11-04) have moved many of them. The current state is in `README.md`, the plan in `docs/roadmap.md`.

## 1. The library census and the scope (October 2026)

Formerly roadmap §1 and §2. The scope that holds now (the library build and the probes within 1%, owner 2026-11-04) is roadmap §2; the "verification over the build space" below was dropped then.


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

### What the engine scored, and what done meant (2026-10-31)

The engine scores a player's showcase build on a library team's rotation: the rotation is the library's, the build is the player's (any constellation, any weapon and refinement, any set, any talent levels and stats). Library builds are the test data, not the scope. Until now they were both the order of porting and the definition of done ("a character is done when every library team with it passes"), which would end in an engine checked only on the builds the library happens to carry. Measured on 2026-10-31 (278 teams supported):

- Eight ported characters stop below C6 (`MAX_CONS`): Nahida, Chiori, Jean, Wriothesley, Yae Miko, Klee and Escoffier at C0, Zhongli at C1. A player's build above that is an error, so it would fall back to gcsim. (At 395 teams: Nahida, Wriothesley, Zhongli, Yae Miko and Escoffier implement C6, so three.)
- The other twenty-one implement C0 to C6 from gcsim's source, but `compare` checks a constellation only when a library build or a probe carries it. Lisa's library builds are 19 of 21 at C6; her C0 and C3 are checked only by two probes, and C4's discharges on dendro cores not at all. There are 41 probes. (At 374 teams: the three characters ported since, Alhaitham, Baizhu and Neuvillette, implement C0 to C6 with a probe for each of C1 to C6, and there are 62 probes. A constellation can also go unreached by its probe: Kuki's C6 never procs on her library build, and its probe had to be built to make it proc. At 395 teams, Kirara too, with probes C0 to C5: 68 probes.)
- Weapons, refinements and talent levels are checked only as the library builds carry them; the variants move stats (ER, crit, ATK, HP, EM) and nothing else.

So done is now:

- **A character**: C0 to C6 (`MAX_CONS = 6`), every action and param gcsim's package has, each constellation compared on at least one supported team.
- **A weapon**: refinements 1 to 5, compared at 1 and 5. **A set**: two and four pieces.
- **Verification over the build space**: `compare` generates, for each supported team, builds that change one axis of one member (constellation 0 to 6, refinement 1 or 5, a weapon of the same class that the engine has), and passes them like the variants. Probes remain for what this does not reach.
- **The measure of progress**: the share of real players' builds (Gscouter's requests or a sample of Enka showcases, on the teams they ask for) that the engine runs and has verified, beside the library census.


## 2. Comparison results, Phases 0 to 2

Formerly the "Results" section of `README.md`.

### The comparison runner

`reference/compare` runs every library team the engine supports under gcsim and under the engine, with its library build and builds with their stats moved (ER, crit, ATK, HP, EM), and passes a config when the 95% interval of each difference (team and every character) lies within ±1% of gcsim. Its latest run (both teams, Gscouter at `2e804aa`):

| Team | Build | Iterations | Team | Worst character | Energy waits (gcsim) |
|---|---|---|---|---|---|
| Hu Tao / Xingqiu / Zhongli / Yelan | library | 4,000 | −0.05% ± 0.15 | Yelan −0.18% ± 0.29 | 0 s |
| | ER from artifacts 0 | 1,000 | −0.02% ± 0.20 | Yelan −0.22% ± 0.35 | 0 s |
| | ER 0, bursts without readiness checks | 318,300 | −0.11% ± 0.30 | Xingqiu +0.26% ± 0.13 | 285 s |
| | ER half | 1,000 | −0.07% ± 0.24 | Zhongli +0.16% ± 0.32 | 0 s |
| | ER half, bursts without readiness checks | 41,356 | −0.02% ± 0.33 | Zhongli +0.29% ± 0.43 | 279 s |
| | ER +30% | 1,000 | −0.08% ± 0.23 | Xingqiu −0.24% ± 0.32 | 0 s |
| | CR −20%, CD −40% | 4,000 | −0.07% ± 0.16 | Yelan −0.23% ± 0.29 | 0 s |
| | CR +20%, CD +40% | 1,000 | −0.15% ± 0.28 | Yelan −0.38% ± 0.55 | 0 s |
| | ATK +40% | 4,000 | −0.05% ± 0.15 | Yelan −0.19% ± 0.29 | 0 s |
| | HP +40% | 4,000 | −0.06% ± 0.15 | Yelan −0.18% ± 0.29 | 0 s |
| | EM +150 | 4,000 | −0.06% ± 0.15 | Yelan −0.18% ± 0.28 | 0 s |
| Hu Tao / Yelan / Bennett / Xingqiu | library | 1,000 | −0.03% ± 0.11 | Yelan −0.05% ± 0.08 | 0.2 s |
| | ER from artifacts 0 | 54,460 | −0.14% ± 0.29 | Hu Tao −0.16% ± 0.28 | 220 s |
| | ER half | 18,404 | −0.13% ± 0.38 | Bennett −0.25% ± 0.38 | 141 s |
| | ER +30% | 1,000 | −0.03% ± 0.10 | Hu Tao −0.06% ± 0.19 | 0 s |
| | CR −20%, CD −40% | 1,000 | −0.12% ± 0.18 | Hu Tao −0.16% ± 0.27 | 1.3 s |
| | CR +20%, CD +40% | 1,000 | −0.01% ± 0.03 | Bennett −0.34% ± 0.41 | 0 s |
| | ATK +40% | 1,000 | −0.03% ± 0.11 | Bennett −0.04% ± 0.43 | 0.2 s |
| | HP +40% | 1,000 | −0.03% ± 0.11 | Bennett −0.04% ± 0.43 | 0.2 s |
| | EM +150 | 1,000 | −0.03% ± 0.12 | Bennett −0.04% ± 0.43 | 0.2 s |

Differences are gse against gcsim, ± the half-width of the 95% interval. All 20 pass.

Phase 2 has made 111 more teams supported so far. Their latest full run (Gscouter at `fc407ac`, library builds and probes; the variants run for the first ones only, see below): all 81 teams supported at the time and their 25 probes pass, 106 of 106. The largest team difference is −0.30% ± 0.37 (Yoimiya / Yelan / Rosaria / Bennett), the largest character difference Bennett −0.53% ± 0.45 (Xiangling / Yelan / Bennett / Xingqiu); the most iterations a verdict needed was 12,832. The 23 teams made supported after that run (Candace, Song of Days Past, Lion's Roar, The Viridescent Hunt) and the Hu Tao C6 probe pass too, 27 of 27 (largest team difference −0.27% ± 0.36, character Bennett +0.40% ± 0.50); that run found `.char.infusion.<key>` always false, now gcsim's WeaponInfuseIsActive (Bennett / Yelan / Fischl / Chevreuse was −71%, now passes at 20,244 iterations). Variant runs took about an hour per team on this session's machine; CI now runs them in parallel parts (see "Running"). Its first run (Gscouter at `14e187f`) passed every library build and probe of the 104 teams supported then. Earlier runs of the first fourteen Phase 2 teams, with their variants (Gscouter at `abf4ae3`):

| Team | Builds passing | Library build | Largest team difference | Largest character difference |
|---|---|---|---|---|
| Hu Tao / Fischl / Yelan / Bennett | 12/12 | +0.11% ± 0.21 | −0.24% ± 0.28 (ER half) | Bennett −0.49% ± 0.40 (ER 0, forced) |
| Hu Tao / Xingqiu / Yelan / Fischl | 9/9 | −0.04% ± 0.04 | +0.22% ± 0.60 (ER half) | Hu Tao +0.28% ± 0.60 (ER half) |
| Hu Tao / Fischl / Xingqiu / Bennett | 11/11 | +0.05% ± 0.34 | +0.28% ± 0.46 (crit −) | Hu Tao +0.38% ± 0.48 (crit −) |
| Bennett / Fischl / Furina / Xingqiu | 11/11 | +0.13% ± 0.18 | −0.40% ± 0.37 (ER 0) | Bennett −0.52% ± 0.42 (ER 0) |
| Hu Tao / Furina / Yelan / Xingqiu | 11/11 | −0.02% ± 0.10 | −0.10% ± 0.29 (ER 0) | Hu Tao −0.28% ± 0.44 (ER 0) |
| Hu Tao / Xingqiu / Bennett / Furina | 11/11 | −0.03% ± 0.12 | −0.32% ± 0.53 (ER half, forced) | Bennett +0.35% ± 0.52 (ER 0) |
| Hu Tao / Yelan / Bennett / Furina | 9/9 | −0.08% ± 0.12 | −0.11% ± 0.13 (ER half) | Bennett −0.25% ± 0.28 (ER 0) |
| Hu Tao / Zhongli / Yelan / Fischl | 9/9 | −0.10% ± 0.18 | −0.38% ± 0.39 (ER 0) | Hu Tao −0.43% ± 0.40 (ER 0) |
| Hu Tao / Xingqiu / Kazuha / Bennett | 9/9 | +0.13% ± 0.20 | −0.19% ± 0.32 (ER 0) | Bennett −0.29% ± 0.41 (ER 0) |
| Hu Tao / Xingqiu / Zhongli / Kazuha | 11/11 | −0.06% ± 0.29 | −0.37% ± 0.31 (EM +150) | Xingqiu +0.41% ± 0.41 (ER half) |
| Hu Tao / Yelan / Xingqiu / Kazuha | 9/9 | −0.02% ± 0.04 | +0.24% ± 0.50 (ER half) | Hu Tao +0.30% ± 0.46 (ER half) |
| Raiden / Fischl / Kazuha / Bennett | 11/11 | −0.02% ± 0.21 | −0.05% ± 0.21 (crit −) | Fischl −0.17% ± 0.23 (crit −) |
| Raiden / Furina / Bennett / Kazuha | 9/9 | −0.05% ± 0.18 | −0.13% ± 0.11 (ER +30%) | Bennett +0.34% ± 0.64 (ER 0) |
| Raiden / Yelan / Fischl / Bennett | 9/9 | −0.10% ± 0.15 | −0.25% ± 0.46 (ER half) | Yelan −0.35% ± 0.63 (ER half) |

A weapon, set or character that no supported team carries would go unchecked, so the runner also runs probes (`reference/probes`): a library team's build with some lines replaced, and its rotation too where no library rotation reaches an action (format under "Running"), compared like a variant. A probe's team need not be supported, only the probe build, so a new character is checked on its own library team with the weapons the engine lacks swapped out (Fischl on Hu Tao / Fischl / Yelan / Bennett before The Stringless and Ballad of the Fjords were ported, Furina at C0, C2 and C6). Every probe passes; each weapon and set without a supported team to carry it has one. Waits by reason and hits and damage by ability are in the runner's report as well; waits agree within about 0.1 s per fight (e.g. Xingqiu waiting for energy 75.38 s / 75.29 s). The first team's rotation checks `.burst.ready` before each burst, so a build short of ER skips bursts instead of waiting for energy; the runner adds builds whose bursts are used without the checks so that the waits Gscouter's fillers depend on are compared too. Those fights are mostly waiting, which is why they need hundreds of thousands of iterations.

Dendro (with Nahida, Wandering Evenstar, Deepwood Memories and dendro resonance; Sapwood Blade's leaf and The Alley Flash's lockout, which only dendro reactions and hits on the player reach) made nine more teams supported. Their library builds pass (Gscouter at `14e187f`):

| Team | Iterations | Team | Worst character | Energy waits (gcsim) |
|---|---|---|---|---|
| Nahida / Bennett / Xiangling / Kazuha (burning) | 1,000 | −0.06% ± 0.09 | Bennett −0.14% ± 0.32 | 2.1 s |
| Kazuha / Bennett / Raiden / Nahida | 45,704 | −0.33% ± 0.40 | Nahida −0.40% ± 0.41 | 87 s |
| Raiden / Fischl / Kazuha / Nahida (aggravate) | 1,000 | −0.02% ± 0.14 | Nahida −0.20% ± 0.28 | 1.7 s |
| Raiden / Fischl / Nahida / Bennett | 36,120 | −0.21% ± 0.37 | Nahida −0.25% ± 0.41 | 30 s |
| Raiden / Nahida / Bennett / Xiangling | 1,000 | −0.14% ± 0.11 | Raiden −0.22% ± 0.15 | 0.9 s |
| Bennett / Nahida / Yelan / Xingqiu (bloom, burgeon) | 1,000 | −0.06% ± 0.09 | Nahida +0.13% ± 0.29 | 0.1 s |
| Bennett / Nahida / Yoimiya / Xingqiu | 1,000 | −0.09% ± 0.22 | Bennett −0.36% ± 0.46 | 5.5 s |
| Gaming / Nahida / Xingqiu / Bennett | 1,000 | +0.05% ± 0.35 | Nahida −0.23% ± 0.49 | 4.2 s |
| Hu Tao / Nahida / Xingqiu / Yelan | 1,000 | −0.01% ± 0.19 | Xingqiu +0.11% ± 0.24 | 12 s |

Burgeon is decided by geometry: a dendro core appears at a random point 0.5 beyond the enemy's edge, and only the attacks whose area reaches it set it off (Bennett's burgeons per fight 154.5 / 154.3). The engine tests gcsim's own circles, fans and boxes against the core (`engine/src/pattern_tests.rs`, 400 cases checked against gcsim's shape code); an attack that could set off a core and has no pattern in the engine is an error that names it.

Shields with HP (gcsim's Tmpl: absorption by element, overwrite by type and target, expiry and break events, the shield bonus mods), crystallize shards and `pick_up_crystallize`, Archaic Petra and Zhongli's A1 made six more teams supported. gcsim drops a shield bonus mod the first time its amount reports done, and Gscouter's shield stats collector asks for the bonus on every shield, swap and hit on the player, so geo resonance's 15% and Zhongli's A1 last only until then; the engine does the same. Their library builds pass (Gscouter at `14e187f`):

| Team | Iterations | Team | Worst character | Energy waits (gcsim) |
|---|---|---|---|---|
| Gaming / Zhongli / Rosaria / Bennett (Archaic Petra, pick-ups) | 4,000 | −0.03% ± 0.24 | Bennett −0.21% ± 0.34 | 5.2 s |
| Hu Tao / Yelan / Zhongli / Furina (Archaic Petra, pick-ups) | 1,000 | −0.01% ± 0.04 | Zhongli −0.06% ± 0.17 | 3.9 s |
| Yelan / Xingqiu / Zhongli / Kazuha | 1,000 | −0.03% ± 0.05 | Kazuha +0.08% ± 0.12 | 0.0 s |
| Yoimiya / Xingqiu / Yelan / Zhongli (Archaic Petra, pick-ups) | 1,000 | −0.04% ± 0.08 | Zhongli +0.18% ± 0.26 | 12.9 s |
| Zhongli / Bennett / Xiangling / Kazuha | 4,000 | +0.08% ± 0.11 | Bennett +0.27% ± 0.35 | 3.9 s |
| Zhongli / Candace / Yelan / Xingqiu | 1,000 | −0.02% ± 0.07 | Zhongli −0.08% ± 0.12 | 0.5 s |

Chiori (her Tamato and the second doll a construct brings, the recast's forced swap, A1 and A4) brought geo resonance with her: six more teams. Their library builds pass (Gscouter at `14e187f`):

| Team | Iterations | Team | Worst character | Energy waits (gcsim) |
|---|---|---|---|---|
| Bennett / Chiori / Kazuha / Zhongli | 1,000 | −0.07% ± 0.21 | Kazuha −0.17% ± 0.26 | 10.5 s |
| Hu Tao / Chiori / Xingqiu / Zhongli | 1,000 | −0.17% ± 0.11 | Hu Tao −0.30% ± 0.19 | 0.8 s |
| Hu Tao / Yelan / Chiori / Zhongli | 1,000 | −0.05% ± 0.13 | Hu Tao −0.11% ± 0.32 | 0.4 s |
| Yoimiya / Chiori / Bennett / Zhongli | 1,000 | +0.00% ± 0.07 | Zhongli −0.10% ± 0.25 | 0.2 s |
| Yoimiya / Yelan / Zhongli / Chiori | 1,000 | +0.02% ± 0.14 | Chiori +0.08% ± 0.19 | 2.6 s |
| Zhongli / Chiori / Bennett / Fischl | 1,000 | −0.27% ± 0.24 | Zhongli −0.42% ± 0.36 | 0.8 s |

Zhongli's burst made two more (Raiden / Fischl / Nahida / Zhongli, team −0.09% ± 0.10; Yoimiya / Bennett / Xiangling / Zhongli, −0.23% ± 0.23; both at 1,000 iterations).

Jean brought reactions on the player (gcsim's ReactWithSelf: her field swirls the player's own aura, Bennett's pyro, and the swirl's aoe around the player hits the enemy; subscribers that need an enemy target skip a swirl on the player) and anemo resonance (cooldowns −5%, stamina −15%): seven more teams. Their library builds pass (Gscouter at `14e187f`):

| Team | Iterations | Team | Worst character | Energy waits (gcsim) |
|---|---|---|---|---|
| Hu Tao / Yelan / Jean / Furina | 1,000 | +0.01% ± 0.06 | Furina +0.10% ± 0.16 | 0.2 s |
| Jean / Furina / Yelan / Yoimiya | 1,000 | −0.01% ± 0.06 | Jean +0.11% ± 0.32 | 0.4 s |
| Jean / Xingqiu / Kazuha / Bennett (anemo resonance) | 1,000 | −0.02% ± 0.12 | Bennett −0.20% ± 0.45 | 1.8 s |
| Raiden / Furina / Bennett / Jean (self swirl) | 1,000 | +0.02% ± 0.15 | Furina −0.13% ± 0.23 | 2.2 s |
| Raiden / Furina / Yelan / Jean | 1,000 | +0.00% ± 0.07 | Jean −0.10% ± 0.28 | 0.8 s |
| Raiden / Yelan / Bennett / Jean (self swirl) | 4,532 | +0.11% ± 0.19 | Jean +0.24% ± 0.40 | 2.9 s |
| Yelan / Raiden / Xingqiu / Jean | 1,000 | +0.02% ± 0.10 | Jean −0.23% ± 0.29 | 1.7 s |

Sucrose made five more. Her burst absorbs only what its check area reaches: a box at the enemy's edge, not the player or Guoba, so absorb checks can now take gcsim's pattern, and gadgets have a position and radius. Waits now count as gcsim's stats do: a whole wait under the reason the action first failed for (her burst waits on energy, then on its cooldown; counted frame by frame it read 8.4 s of cooldown against gcsim's 2.5). Their library builds pass:

| Team | Iterations | Team | Worst character | Energy waits (gcsim) |
|---|---|---|---|---|
| Bennett / Sucrose / Xiangling / Xingqiu | 1,000 | −0.05% ± 0.12 | Bennett −0.29% ± 0.28 | 0.1 s |
| Gaming / Furina / Sucrose / Bennett | 4,616 | +0.31% ± 0.34 | Gaming +0.37% ± 0.42 | 21.6 s |
| Gaming / Rosaria / Chevreuse / Sucrose | 1,000 | +0.17% ± 0.69 | Sucrose +0.23% ± 0.72 | 7.9 s |
| Hu Tao / Xingqiu / Sucrose / Yelan | 1,000 | −0.05% ± 0.10 | Hu Tao −0.09% ± 0.20 | 0.2 s |
| Sucrose / Bennett / Raiden / Nahida | 1,000 | +0.06% ± 0.15 | Bennett +0.17% ± 0.50 | 23.8 s |

Then, taking the census's list of teams one item from support: Wriothesley (16 teams), Yae Miko (13), Gilded Dreams (12) and Diona (9), fifty more. Wriothesley's Chilling Penalty drains his HP and A1's Rebuke heals it, which A4 and Marechaussee Hunter count; Yae Miko's totems, the burst's thunderbolts and the revelation (gcsim's default: longer totems, superconduct's enhanced strike, A1's strike on the fourth totem); Diona's paws make and lengthen a shield. Their teams with dendro cores needed gcsim's hit patterns for Raiden, Fischl and Xiangling. Radiance (Wriothesley's and Yae Miko's) needs a Polestar Field, which nothing ported makes, and Sucrose's skill swirling Guoba's pyro window is an error until it is ported. All their library builds pass:

| Teams | Builds | Largest team difference | Largest character difference |
|---|---|---|---|
| Wriothesley's 16 | 16 of 16 pass | −0.25% ± 0.43 (with Bennett / Xiangling / Zhongli) | −0.33% ± 0.57 (Wriothesley, same team) |
| Yae Miko's 13 | 13 of 13 pass | −0.06% ± 0.11 (with Raiden / Xiangling / Chevreuse) | +0.25% ± 0.71 (Bennett, with Raiden / Jean) |
| Gilded Dreams' 12 | 12 of 12 pass | −0.10% ± 0.10 (Raiden / Yae Miko / Kazuha / Nahida) | −0.20% ± 0.33 (Zhongli, with Yoimiya / Yae Miko / Yelan) |
| Diona's 9 | 9 of 9 pass | +0.15% ± 0.39 (Yoimiya / Yelan / Diona / Bennett) | +0.27% ± 0.60 (Bennett, same team) |

Klee (C0, with gcsim's `action.Info.QueueAction`: a task that runs once an action has lasted some frames, and never if another action replaces it first) makes ten more teams, all of whose library builds pass. Klee / Fischl / Xingqiu / Bennett had looked −1.78% at 1,000 iterations; it was noise. Its bursts wait for energy, and a wait takes Klee's time on the field, which gives the energy the next burst waits for, so a short cycle can spiral into a wait of 40 s: the fight's DPS has a heavy tail (one 0.01 more ER on Xingqiu and Bennett moves it 2%), and gcsim's first 1,000 seeds happen to sit 0.7% above its mean. Over 8,000 seeds energy by source agrees within 0.1% in the first cycles, and the runner passes it at 36,000 iterations, +0.01% ± 0.34, with Xingqiu's energy waits 21.17 s against gcsim's 21.18 s:

| Team | Iterations | Team | Worst character | Energy waits (gcsim) |
|---|---|---|---|---|
| Klee / Fischl / Furina / Bennett | 1,000 | −0.05% ± 0.16 | Bennett −0.25% ± 0.61 | 2.4 s |
| Klee / Fischl / Xingqiu / Bennett | 36,000 | +0.01% ± 0.34 | Klee +0.01% ± 0.39 | 30.6 s |
| Klee / Furina / Bennett / Xiangling | 1,000 | −0.19% ± 0.26 | Xiangling −0.43% ± 0.37 | 0.5 s |
| Klee / Furina / Kazuha / Chevreuse | 1,000 | −0.14% ± 0.13 | Kazuha −0.31% ± 0.29 | 1.5 s |
| Klee / Furina / Xingqiu / Bennett | 1,000 | +0.08% ± 0.15 | Xingqiu +0.34% ± 0.20 | 3.6 s |
| Klee / Furina / Yelan / Bennett | 1,000 | +0.05% ± 0.09 | Klee +0.08% ± 0.15 | 1.0 s |
| Klee / Xingqiu / Bennett / Kazuha | 4,000 | −0.02% ± 0.16 | Bennett +0.24% ± 0.30 | 0.7 s |
| Raiden / Xingqiu / Klee / Xiangling | 1,000 | −0.04% ± 0.20 | Raiden −0.13% ± 0.28 | 0.6 s |
| Xingqiu / Bennett / Xiangling / Klee | 1,000 | +0.08% ± 0.24 | Bennett −0.21% ± 0.55 | 0.9 s |
| Xingqiu / Yelan / Bennett / Klee | 1,000 | −0.01% ± 0.19 | Xingqiu −0.12% ± 0.21 | 0.1 s |

Then Lynette (12 teams), Shenhe (12) and Thrilling Tales of Dragon Slayers (7), the census's largest one-item blockers. Lynette's Bogglecat Box is a gadget that takes hits (`Sim::add_hittable_gadget`, `Character::gadget_hit`): it converts to the first cryo, pyro, hydro or electro attack that reaches it, or to what its absorb check finds every 0.3 s, and fires Vivid Shots of that element; the attacks in her teams that can reach it got gcsim's hit patterns (Furina's, Rosaria's, Escoffier's, Xingqiu's skill, burst and orbital). Shenhe's Icy Quills add her current ATK times the skill's multiplier to each cryo hit of the kinds gcsim lists. Two general fixes came with them, found on these teams:

- Patterns kept their coordinates in f32. A dendro core sits exactly the enemy's radius + 0.5 from its center, so a circle of radius 0.5 on the enemy (Fischl's A4) touches it at exactly the sum of the radii: in f32 it reached such cores half the time, gcsim two thirds. Lynette / Fischl / Xingqiu / Nahida went from Lynette +2.5% to +0.2%. Patterns are f64 now (+0.6% instructions).
- Attack mods applied to reaction damage; gcsim's ApplyAttackMods skips it. Cryo resonance's crit rate made 12% of cryo swirls crit, and Lynette's crits fed her Favonius: Wriothesley / Shenhe / Lynette / Bennett went from +1.14% to +0.00%.

Of the teams supported before, four changed with them (two with dendro cores, two with cryo resonance and swirls); they pass again. All library builds pass:

| Teams | Builds | Largest team difference | Largest character difference |
|---|---|---|---|
| Lynette's 12 | 12 of 12 pass | −0.19% ± 0.35 (Gaming / Rosaria / Lynette / Bennett, 26,000 iterations) | +0.35% ± 0.38 (Bennett, with Lynette / Xiangling / Kazuha) |
| Shenhe's 12 | 12 of 12 pass | −0.24% ± 0.58 (Wriothesley / Shenhe / Furina / Bennett) | −0.49% ± 0.45 (Kazuha, with Wriothesley / Diona / Shenhe) |
| Thrilling Tales' 7 | 7 of 7 pass | −0.08% ± 0.19 (Gaming / Rosaria / Sucrose / Bennett) | −0.25% ± 0.49 (Bennett, with Hu Tao / Yelan / Sucrose) |
| The 4 that changed | 4 of 4 pass | −0.12% ± 0.23 (Wriothesley / Furina / Candace / Escoffier) | −0.36% ± 0.46 (Bennett, with Nahida / Yoimiya / Xingqiu) |

Lynette / Yelan / Fischl / Bennett waits 207 s a fight on energy and passes at 95,500 iterations (+0.05% ± 0.69).

Bennett's charged attack (gcsim's ChargeAttack: two physical hits on circles of radius 2.2 around the player, without hitlag; a sword's 20 stamina and a normal attack right before it, as gcsim's template asks) blocked 19 teams, but only one by itself: Bennett / Furina / Rosaria / Escoffier, whose library build passes at 1,000 iterations, team +0.12% ± 0.19, largest character difference Rosaria +0.33% ± 0.28, charged hits 59.3 / 59.2 a fight. The 141 Bennett teams supported before give byte-identical output.

Yae Miko's charged attack, which the census listed as blocking 55 teams, made two supported: the other 53 have other blockers too. It is gcsim's ChargeAttack: five 2 by 2 boxes 9 frames apart, each 1.65 further toward the target, in the ICD group whose 0.5 s lets only the first hit deal damage and apply electro. At Gscouter's distance the first two boxes reach the enemy; the other three miss it and reach only the gadgets they cover (`Sim::queue_gadget_attack`), which can be dendro cores. Their library builds pass (Gscouter at `14e187f`):

| Team | Iterations | Team | Worst character | Charged attacks per fight (gcsim) |
|---|---|---|---|---|
| Kazuha / Yae Miko / Fischl / Nahida | 1,000 | +0.01% ± 0.10 | Kazuha +0.19% ± 0.20 | 92.07 (92.08) |
| Nahida / Yelan / Yae Miko / Zhongli (hyperbloom) | 1,000 | −0.04% ± 0.10 | Nahida +0.15% ± 0.37 | 26.00 (26.00) |

Flower of Paradise Lost made six more teams supported, each one item from support: 80 EM, then bloom, hyperbloom and burgeon DMG +40% as a reaction bonus, and a stack of a quarter of that more for 10 s (hitlag extends it) at each bloom on the enemy, hyperbloom or burgeon the holder triggers, on field or off, up to four, once a second. gcsim's 10% for Lunar-Bloom is left out, as the engine has no lunar reactions. Its holders cover all three reactions: Bennett and Xiangling burgeon, Raiden hyperbloom, Xingqiu bloom. All their library builds pass (Gscouter at `14e187f`):

| Team | Iterations | Team | Worst character | Energy waits (gcsim) |
|---|---|---|---|---|
| Bennett / Fischl / Nahida / Xingqiu (Bennett, burgeon) | 1,000 | +0.02% ± 0.07 | Nahida −0.06% ± 0.29 | 0.2 s |
| Fischl / Raiden / Nahida / Candace (Raiden, hyperbloom) | 1,000 | −0.03% ± 0.08 | Candace −0.10% ± 0.19 | 0.3 s |
| Nahida / Xingqiu / Kazuha / Diona (Xingqiu, bloom) | 4,000 | +0.06% ± 0.14 | Xingqiu +0.08% ± 0.23 | 61 s |
| Nahida / Xingqiu / Raiden / Zhongli (Raiden) | 1,000 | −0.01% ± 0.05 | Zhongli +0.28% ± 0.28 | 0.3 s |
| Nahida / Yelan / Xingqiu / Xiangling (Xiangling, burgeon) | 1,000 | −0.04% ± 0.05 | Xingqiu −0.08% ± 0.12 | 0.1 s |
| Raiden / Fischl / Xingqiu / Nahida (Raiden) | 1,000 | −0.02% ± 0.13 | Fischl −0.11% ± 0.40 | 1.3 s |

Thundering Fury (electro +15%; with four pieces overloaded, electro-charged, superconduct and hyperbloom DMG +40%, aggravate's bonus +20%, and the holder's skill cooldown −1 s, once every 0.8 s, when it triggers one of them or quicken on an enemy while on field, or hyperbloom from a dendro core) made five more teams supported, four of them with the set on a pyro character beside Fischl. Their library builds pass, as do three probes that put it where those teams do not reach (Raiden on an aggravate team, Fischl beside Lynette's cryo, Fischl on a hyperbloom team):

| Team | Iterations | Team | Worst character | Energy waits (gcsim) |
|---|---|---|---|---|
| Bennett / Chevreuse / Fischl / Raiden | 12,000 | −0.08% ± 0.33 | Bennett −0.11% ± 0.35 | 6.8 s |
| Bennett / Chevreuse / Fischl / Yae Miko | 1,000 | +0.04% ± 0.23 | Chevreuse +0.08% ± 0.32 | 0.6 s |
| Hu Tao / Xingqiu / Fischl / Zhongli | 1,000 | −0.19% ± 0.26 | Hu Tao −0.23% ± 0.29 | 5.5 s |
| Yelan / Fischl / Yae Miko / Xingqiu | 1,000 | −0.11% ± 0.14 | Xingqiu −0.23% ± 0.41 | 9.1 s |
| Yoimiya / Yae Miko / Fischl / Xingqiu | 1,000 | −0.01% ± 0.06 | Xingqiu −0.06% ± 0.12 | 0.1 s |
| Probe: Raiden with it, Raiden / Fischl / Kazuha / Nahida | 4,000 | +0.00% ± 0.09 | Nahida −0.09% ± 0.15 | 4.5 s |
| Probe: Fischl with it, Lynette / Furina / Fischl / Bennett | 2,000 | +0.17% ± 0.59 | Bennett +0.24% ± 0.58 | 8.9 s |
| Probe: Fischl with it, Fischl / Nahida / Yoimiya / Xingqiu | 1,000 | +0.05% ± 0.23 | Yoimiya +0.22% ± 0.59 | 1.0 s |

Lisa made nine more teams supported (the roadmap had counted six blocked by her alone). Her module has the pressed and held skill with the enemy's Conductive stacks (the held skill takes its multiplier from the stacks as it hits and consumes them), the charged attack with A1's stack, Lightning Rose with A4 and C4's discharges, which choose among the enemy and the enemy-camp gadgets in the area (dendro cores, the Bogglecat Box: gadgets now say whether they are of the enemy camp), C1, C2 and C6's stacks on swap. The core gained enemy DEF mods (gcsim's AddDefMod, hitlag affected, at most 90% in total) for A4, and `.debuff.def` reads them. Its `.tags` conditions had read 0 for every tag; the enemy's Conductive stacks now read Lisa's count, and the tags ported content sets but the engine does not keep are errors (on the enemy Yelan's, Lynette's and Hu Tao's; on characters Yae Miko's totems and Shenhe's quills; no library team reads them). Every library build passes, with two probes for her lower constellations (19 of the 21 Lisa builds in the library are C6):

| Team | Build | Iterations | Team | Worst character | Energy waits (gcsim) |
|---|---|---|---|---|---|
| Hu Tao / Yelan / Xingqiu / Lisa | library (C6) | 1,000 | +0.09% ± 0.11 | Hu Tao +0.21% ± 0.23 | 142.4 s |
| | Lisa C0 (probe) | 1,000 | +0.04% ± 0.15 | Hu Tao +0.16% ± 0.26 | 142.7 s |
| Lisa / Xingqiu / Klee / Xiangling | library | 1,000 | +0.15% ± 0.46 | Xingqiu +0.22% ± 0.40 | 8.5 s |
| Lisa / Xingqiu / Yelan / Yoimiya | library | 1,000 | +0.01% ± 0.05 | Xingqiu +0.04% ± 0.12 | 17.1 s |
| Raiden / Bennett / Nahida / Lisa | library | 1,000 | +0.04% ± 0.22 | Lisa +0.10% ± 0.28 | 8.8 s |
| | Lisa C3 (probe) | 1,000 | +0.04% ± 0.22 | Lisa +0.09% ± 0.27 | 8.8 s |
| Raiden / Chevreuse / Lisa / Bennett | library | 1,000 | −0.15% ± 0.17 | Chevreuse −0.52% ± 0.33 | 9.0 s |
| Raiden / Lisa / Xiangling / Chevreuse | library | 1,000 | +0.02% ± 0.13 | Chevreuse +0.19% ± 0.25 | 1.1 s |
| Raiden / Xiangling / Lisa / Xingqiu | library | 1,000 | −0.02% ± 0.23 | Xingqiu −0.14% ± 0.30 | 6.5 s |
| Xingqiu / Lisa / Bennett / Xiangling | library | 1,000 | +0.02% ± 0.10 | Bennett +0.11% ± 0.25 | 0.1 s |
| Yoimiya / Lisa / Chevreuse / Bennett | library | 2,000 | −0.02% ± 0.52 | Bennett −0.06% ± 0.77 | 17.8 s |

Hits per ability agree (Hu Tao / Yelan / Xingqiu / Lisa per fight: Lightning Rose ticks 239.95 / 239.96, held skill 6 / 6; Lisa's energy waits 142.41 s / 142.40 s). No library team with her uses the pressed skill or meets dendro cores, so the pressed skill and C4's discharges on gadgets are not compared yet.

These five came from parallel branches off the same commit and were merged together. Merged, they make three more teams supported that each needed two of them: Lisa with Thundering Fury (Bennett / Chevreuse / Fischl / Lisa, +0.08% ± 0.71 at 8,000 iterations; Yoimiya / Chevreuse / Fischl / Lisa, −0.02% ± 0.28, Lisa −0.42% ± 0.45) and Lisa with Flower of Paradise Lost (Lisa / Xingqiu / Nahida / Zhongli, +0.02% ± 0.26). The merged engine was compared again on all 26 teams these five made supported, the five probes they added and the three teams those probes use: 34 of 34 builds pass, with the same results as on their branches.

Then three more branches, taken from the census's queue: Kuki Shinobu, Sacrificial Jade and Hakushin Ring. Kuki's module has her normal and charged attacks, Sanctifying Ring (its HP cost, the ring and its heal), her burst, A1, A4 and every constellation; her HP drain, ring healing and C4 and C6 follow the heal and drain events the engine already had. She made fifteen teams supported. Hakushin Ring (an electro reaction the holder triggers on field gives party members of electro and of the reaction's other element that element's DMG bonus, with an ICD per element; lunar-charged and stellar-conduct are noted for when the engine emits them) made seven. Sacrificial Jade (HP% and EM after 5 s off field, ending after 10 s on field; `stacks=1` starts with the buff, any other param is an error) blocked 401 teams but made none supported by itself: each of them has other blockers. Merged, CI compared every supported team's library build and every probe: 319 of 319 pass (Gscouter at `14e187f`). Their builds:

| Team | Build | Iterations | Team | Worst character |
|---|---|---|---|---|
| Kuki / Fischl / Xingqiu / Nahida | library (Kuki C6) | 1,000 | −0.01% ± 0.10 | Kuki −0.16% ± 0.44 |
| | Kuki C1 (probe) | 1,000 | −0.14% ± 0.15 | Xingqiu −0.20% ± 0.25 |
| | Kuki C4 (probe) | 1,000 | −0.01% ± 0.10 | Kuki −0.16% ± 0.44 |
| | Kuki C5 (probe) | 1,000 | −0.01% ± 0.10 | Kuki −0.16% ± 0.44 |
| Klee / Yelan / Kuki / Nahida (largest of Kuki's fifteen) | library | 1,000 | +0.28% ± 0.29 | Nahida +0.49% ± 0.42 |
| Lisa / Xingqiu / Fischl / Jean (largest of the ring's seven) | library (R5) | 1,000 | −0.16% ± 0.22 | Fischl −0.24% ± 0.37 |
| | Hakushin Ring R1 (probe, electro-charged) | 1,000 | −0.16% ± 0.22 | Fischl −0.24% ± 0.37 |
| Lisa / Xingqiu / Nahida / Zhongli | Hakushin Ring R5 on Lisa (probe, hyperbloom) | 1,000 | −0.04% ± 0.22 | Nahida +0.20% ± 0.41 |
| Lisa / Fischl / Rosaria / Nahida | Hakushin Ring R1 on Lisa (probe, superconduct) | 1,000 | −0.05% ± 0.08 | Fischl −0.15% ± 0.10 |
| Bennett / Nahida / Yoimiya / Xingqiu | Sacrificial Jade R1 on Nahida (probe) | 1,000 | −0.08% ± 0.22 | Bennett −0.44% ± 0.47 |
| | Sacrificial Jade R1, `stacks=1` (probe) | 1,000 | −0.07% ± 0.22 | Bennett −0.42% ± 0.47 |
| Klee / Fischl / Xingqiu / Bennett | Sacrificial Jade R5 on Klee (probe) | 1,000 | −0.03% ± 0.25 | Bennett −0.42% ± 0.58 |

On Kuki / Fischl / Xingqiu / Nahida the C5 probe gives gcsim the same DPS as her C6 library build, so her C6 changes nothing on that team.

Then Alhaitham (28 teams blocked by him alone, 306 supported), and from there three branches merged together: Prototype Amber (10 teams), Baizhu (18) and Neuvillette (34). Merged they make 374: their 62 teams and six that needed two of them. The characters are ported to C6, each with a probe per constellation from C1 to C6, and the weapon at every refinement, with two probes at R1 (library builds carry R5). CI passed each branch's library builds and probes, and those of the merged `main` (`b6106ff`, run 41201432741):

- Alhaitham: normal and charged attacks, the pressed and held skill and the low plunge after a held skill, the burst, the Chisel-Light Mirrors, A1, A4 and every constellation. His high plunge and the low plunge with `collision` are Xianyun's airborne plunges and stay errors.
- Baizhu: Universal Diagnosis (three hits on one snapshot, its particles and party heal), the burst's Seamless Shields with their refreshes and heals, and the Spiritvein a shield fires when replaced, expired or broken (a shield type whose expiry calls its owner), A1, A4 and every constellation. His teams with dendro cores needed gcsim's hit patterns for Kazuha's normal and charged attacks, A1 and absorbed burst ticks: pyro or electro (by absorption or Bennett's C6 infusion), they had passed `gse --check` and stopped with `unsupported: hit pattern` in the first fights.
- Neuvillette: the charged attack and Equitable Judgment (its HP drain, ticks and their early cancel; frames that follow its growing length), the skill and its thorn, the burst, A1, A4 and every constellation. Sourcewater Droplets are gadgets at gcsim's random points, drawn in its order; a charged attack takes up to three whose centers are in its box or fan, in Go's `rand.Perm` order. `.gadgets.dendrocore`, `.gadgets.sourcewaterdroplet` and `.gadgets.crystallizeshard` read the gadgets (three of his teams read the shards).
- Prototype Amber: the holder's burst on field gives it energy and heals every member 2, 4 and 6 s later, each heal queued on the character healed, so its hitlag delays it.

Kuki's C6 (EM +150 for 15 s when a drain leaves her at 25% HP or less) never procs on a library team: her ring's heals keep her above 25% after each skill. A probe (`kuki-c6`) drops her artifact EM and raises her HP so that she settles on the 20% floor and C6 procs in the scored window. At 1,000 iterations it passes, team −0.04%, Kuki −0.32%, with 3.31 procs per fight in gcsim and 3.30 in the engine. `start_hp=1` on a char line, the obvious way to start her low, is ignored by gcsim and the engine alike.

Then three branches merged together, Chevreuse's hit patterns, Nahida C1 to C6 and Kirara (21 teams, C0 to C6, probes `kirara-c0` to `kirara-c5`; her plunges are errors, as gcsim has none), made 395. Every Chevreuse attack has gcsim's pattern (`Pattern::FanDir` for the burst's secondary shells); a Chevreuse config with the target off the y axis is rejected, as Neuvillette's is. Nahida's C2 Lunar-Bloom crit part is unreachable while Lunar-Bloom is an error; Nilou's Bountiful Core joins C2's list when Nilou is ported. CI on the merged `main` (`17464bf`, run 41264956163) passes every library build and probe of the 395 teams. The build space then found a gadget bug: Lynette's Bogglecat Box kills itself in its own tick, and `tick_gadgets` skipped the next gadget and read past the end (Neuvillette / Xiangling / Lynette / Kirara with Lynette on Xiphos' Moonlight). Gadgets now carry gcsim's TargetKey, and loops that run hooks follow keys, as gcsim's nil slots do; no score moved on 82 configs (`41b703f`).

The build-space variants (see "Running") had their first complete run on `main` at `17464bf` (run 41264960806, parts 0-79): 395 teams, 14,889 builds. 13,733 pass and none fails; the other 1,156 the engine does not run, all constellations not yet ported (Yae Miko 318, Zhongli 280, Wriothesley 192, Escoffier 144, Jean 96, Klee 84, Chiori 42). In the run itself 9 were inconclusive at the 512,000 cap and one was the gadget panic; reruns (`compare -builds` through the workflow's `rerun` input, cap 2,048,000; run 41272810063 on `main`, run 41273779826 on the fix's branch) passed all ten. So every build the engine runs passes: 92.2% of the build space runs and is verified. The reruns show how heavy-tailed a team whose bursts wait for energy can be: Klee / Fischl / Xingqiu / Bennett with Xingqiu at C0 to C5 passed at 750,000 to 1,308,000 iterations (C0 is the same fight as C1: gcsim's Xingqiu has no C1 code; at C0, team −0.08% ± 0.09 and Klee −0.36% ± 0.48 at 1,000,000: Klee's DPS has a coefficient of variation of 1.74 under Xingqiu's energy waits, and the waits agree within 0.01 s, so the engine does not differ), Alhaitham / Xingqiu / Fischl / Raiden with Fischl at C4 and C5 at 1,070,000, Wriothesley / Nahida / Lynette / Bennett with Lynette on Sacrificial Sword at 1,626,000, the panic's build at 2,000. A second complete run on `main` at `5e2276f` (run 41280909462, parts 0-79), with Nahida's, Wriothesley's, Zhongli's and Yae Miko's constellations and the iteration cap raised past 512,000 where gcsim's cache already covers it: 14,928 builds, 14,562 pass, none fails, none is inconclusive, and the other 366 are the constellations of Escoffier (144), Jean (96), Klee (84) and Chiori (42), since ported. 97.5% of the build space runs and is verified.

Phase 2's closing run (`main` at `45084fc`, 2026-11-01; `docs/roadmap.md` §3, Phase 2 step 5) supports 496 teams: every library build and probe passes; of 18,718 build-space builds 18,717 pass (the slowest at 5,405,500 iterations), none fails, and one is an error that names what it lacks; of 4,262 variants 4,258 pass, two are refused by both engines and two are inconclusive.

The runner also classifies every library team. Of 4,570, 395 were supported and 4,175 not at this census (on `41b703f`, Gscouter at `14e187f`; 496 since Phase 2's last merges); what blocks the most teams:

| Kind | Most teams blocked |
|---|---|
| Characters | Xilonen 482, Xianyun 314, Mavuika 277, Durin 241, Citlali 221, Emilie 214, Ororon 213, Nicole 191, Arlecchino 184, Mona 174 |
| Weapons | Oathsworn Eye 352, Finale of the Deep 336, Chain Breaker 226, Cinnabar Spindle 141, Fading Twilight 107, Song of Stillness 107 |
| Sets | Ocean-Hued Clam 391, Obsidian Codex 385, Celestial Gift 332, Fragment of Harmonic Whimsy 283, Long Night's Oath 257, Unfinished Reverie 250 |
| Built-ins | `set_player_pos` 45, `set_on_tick` 1 |
| Actions of ported characters (81 teams) | plunges without Xianyun (Diluc's high plunge 17, Yae Miko's 7, Klee's 6, Alhaitham's, Furina's and Hu Tao's 5 each, and others), Xingqiu's charge 3, Diona's aim 2 |
| Constellations not ported (47 teams) | Chiori C6 12, C2 2, C1 1; Jean C4 7; Wriothesley C1 5, C6 2; Klee C2 4, C6 2, C1 1; Yae Miko C1 3, C6 3, C2 1; Zhongli C6 3, C2 2 |
| Other | two Hexerei characters 394 (Fischl with Mona, Sucrose, Razor and the others) |

A team is usually blocked by five to ten items at once, so porting the item that blocks the most teams rarely makes a team supported by itself: the ten items that blocked the most teams at the start of Phase 2 together made none supported. The report also lists the teams one or two items from support, which is how the teams above were reached.

### Earlier measurements

Accuracy at 10,000 iterations (`configs/team-looped.txt`, the config Gscouter assembles; seeds differ, since the engine has its own random numbers):

| | gcsim | Rust | Difference |
|---|---|---|---|
| Team | 72,263.7 ± 24.8 | 72,265.8 ± 24.9 | +0.00% |
| Hu Tao | 45,197.3 ± 12.5 | 45,193.8 ± 12.6 | −0.01% |
| Xingqiu | 9,790.9 ± 6.7 | 9,797.8 ± 6.7 | +0.07% |
| Zhongli | 1,472.1 ± 0.5 | 1,473.6 ± 0.5 | +0.10% |
| Yelan | 15,803.3 ± 10.4 | 15,800.6 ± 10.4 | −0.02% |

Hits per ability agree as well (e.g. per fight in the window: Hu Tao's charged attacks 132.7 / 132.6, Xingqiu's rain swords 518.2 / 519.5, Yelan's Exquisite Throws 438.2 / 440.1).

Other configs, 10,000 iterations except the first:

| Config | gcsim | Rust | Difference |
|---|---|---|---|
| `hutao-solo-looped.txt` (1,000 iterations) | 24,599.8 | 24,607.0 | +0.03% |
| `team-onepass.txt`: one pass under the source's conditions, enemy with HP (damage mode) | 67,904.9 | 67,919.1 | +0.02% |
| `lang-test.txt`: functions, switch with fallthrough, for, while with break, else if, wait, delay, fields, execute_action | 57,485.2 | 57,479.7 | −0.01% |

The parsed configs give exactly the scores the hand-compiled rotations of the first version gave (72,266.73 and 24,607.03 at 1,000 iterations), so the parser, the evaluator and the base stats computed from gcsim's catalog all match what was taken from gcsim before.

Speed, on the same 4-CPU machine (Phase 0; see below for Phase 1):

| | gcsim | Rust | Ratio |
|---|---|---|---|
| Native, 1 thread, 500 iterations | 9.8–10.1 s | 1.18–1.32 s | about 8× |
| Native, 4 threads, 10,000 iterations | 78.6 s | 6.2 s | about 13× |
| WebAssembly in V8 (Node 22, WASI), 1 thread, 500 iterations | 52.0 s | 1.75 s | about 30× |
| WebAssembly build size | 46 MB | 0.45 MB | |

Evaluating gcsl costs about 7% over the hand-compiled rotation of the first version (1.10 s).

Phase 2 so far costs about 4% more instructions on `team-looped.txt` (valgrind, 20 iterations: 468.3 million against 451 million at the start of the phase): attacks carry more fields (blunt, poise, from the fight itself, a hit pattern, now in f64, +0.6%) and there are nine aura slots, which the aura's tick skips while they are empty. Lisa added 0.24% (valgrind's lackey, 20 iterations: 471.4 million before, 472.5 million after; the damage formula now sums the enemy's DEF mods with its resistances), and the merged round of Lisa, Bennett's and Yae Miko's charges, Flower of Paradise Lost and Thundering Fury counts the same 472.5 million. It had reached 480.0 million before the animation tick stopped copying the action out and back each frame (−3%, with byte-identical output). Wall time on this session's machine is too noisy to show it (500 iterations, one thread, seven runs: median 1.31 s at the start of this round and after it). Phase 1 kept the speed. On this session's machine (slower than the one above), 500 iterations on one thread take 1.47 s (median of 7) against 1.46 s for Phase 0, +0.8%; 10,000 iterations on 4 threads take 7.6 s; the Bennett team takes 1.32 s for 500. The two reference configs still give the same scores to the last digit (72,266.73 and 24,607.03 at 1,000 iterations, checked by `engine/tests/scores.rs`), and the restructure gave byte-identical output on every config. Hu Tao's 0-dmg Paramita attack, ported later, adds a crit draw to her skill, so they are now 72,247.45 and 24,589.24.

The WebAssembly row is the one that matters for the site, which scores in the visitor's browser. Go's WebAssembly runs gcsim about 5× slower than native Go; Rust's runs about 1.4× slower than native Rust. The gcsim side is the reference harness built with `GOOS=wasip1`; Gscouter's site uses `GOOS=js`, which has the same compiler backend.


## 3. Phase records

Formerly roadmap §3, Phases 0 to 3, as written while they ran.

### Phase 0: prototype (done)

Hu Tao / Xingqiu / Zhongli / Yelan and the gcsl parser, within 0.03% of gcsim; about 8× faster native and about 30× faster in WebAssembly (`README.md`).

### Phase 1: foundations for porting at scale (done)

What had to be in place before dozens of characters, so that they are not rewritten later (`README.md` has the details and the full tables).

- **Comparison runner** (`reference/compare`, in CI). For each library team it assembles Gscouter's looped config, asks the engine what it lacks, and runs the supported ones under both engines: the library build and nine builds with their stats moved, among them ER-starved builds whose bursts wait for energy. Team and per-character DPS pass when the 95% interval of the difference lies within ±1%; iterations grow until each engine's standard error is at most 0.3% and further, up to 512,000, while a verdict is inconclusive, each step by the factor (2 to 4) at which the inconclusive intervals would clear the bound. Waits by reason and per-ability hits and damage are reported beside them, as a Markdown report and JSON.
  - Result: both supported teams and all their builds pass, 20 of 20; the largest team difference is −0.15%, the largest character difference −0.38% ± 0.55 (Yelan, crit +20%).
- **Generated data** (`reference/gendata`). Every character's, weapon's and set's package variables and constants as gcsim computes them (talent multipliers, frames, hitmarks, hitlag, mod keys), every ICD tag and group, base stats and what each character's `NewChar` and `Init` set: all 168 member keys of the library, 252 weapons, 55 sets. The four ported characters take their tables from it; their scores did not move (72,266.73 and 24,607.03 at 1,000 iterations).
- **Core structure** (`engine/src/content.rs`). Characters, weapons and sets are modules on hooks (actions, tasks, attack callbacks, mod amounts, attack mods, events in gcsim's subscription order, frames, conditions), dispatched statically through macro-generated `match`es. The restructure gave byte-identical output and the same speed (+0.8%, 1.47 s against 1.46 s for 500 iterations on one thread).
- **Check of the structure**: Bennett, with Deathmatch, Favonius Warbow, The Alley Flash, Harbinger of Dawn and Instructor, makes `hutao-yelan-bennett-xingqiu` a supported library team (team −0.03%). The core gained only general systems for it: weapon infusion, reaction events, fight-wide statuses, pyro resonance.
- **Progress measure**: the runner's census. 2 of 4,570 teams are supported. The largest blockers: Noblesse Oblige (2,087 teams), Golden Troupe (1,852), Favonius Sword (1,845), Scroll of the Hero of Cinder City (1,493), Favonius Lance (1,376), Furina (1,324), Viridescent Venerer (1,290), Fischl (1,001); electro resonance 961, cryo 774; `pick_up_crystallize` 511.

### Phase 2: the rest of the engine

Progress so far, taken in the order that makes the most library teams supported (`README.md` has the results):

- **395 of 4,570 teams supported** (2 at the start; census on `main` at `41b703f`, Gscouter at `14e187f`). CI on `main` at `17464bf` (run 41264956163) passes every library build and probe of the merged engine; the first complete build-space run (run 41264960806, with reruns) passes every build the engine runs; the variants and build-space variants run on the daily schedule.
- **The runner in CI, in parts**: `compare -only library,variants,probes -shard k/n`. Every push compares the library builds and probes in 32 parallel jobs. Since 2026-11-04 (owner) verification is the library builds and probes within 1% of gcsim, nothing more, and support means any build a player enters for content the engine has: the variants, build space and pages are no longer verified and have no schedule, and a manual run can still take their parts.
- **Electro, overload and electro-charged** (with Fischl), each reaction emitting its event as gcsim's `Reactable.React` does; reaction damage and `REACTION_LVL_BASE` from gcsim's combat package.
- **HP changes** (with Furina): `Player.Heal` and `Player.Drain` with `OnHeal` and `OnPlayerHPDrain`, overheal, and incoming healing bonus mods. HP debt came later (step 3 below).
- **Swirl, the player's aura and electro resonance** (with Kazuha and Raiden): swirl's hit on its own enemy, reaction bonus mods, Bennett's self infusion read by absorb checks; OnEnergyBurst and OnParticleReceived.
- **Content**: Fischl, Furina, Kazuha, Raiden; Noblesse Oblige, Golden Troupe, Cinder City, Marechaussee Hunter, Viridescent Venerer; the Favonius weapons, The Stringless, Ballad of the Fjords, Wolf-Fang, The Alley Hunter, Sapwood Blade, Festering Desire, Slingshot, Dragon's Bane, Xiphos' Moonlight; Zhongli's normal attacks.
- **Probes** in the runner, so that content no supported team carries is compared too.
- **Cryo** (with Rosaria and Escoffier): the cryo and frozen auras, frozen's decay, melt, freeze, superconduct and its physical shred, poise and shatter (attacks carry whether they are blunt and their poise damage), cryo swirl and crystallize, cryo resonance.
- **More content, by the census's list of teams one or two items from support**: Xiangling (Guoba is a gadget, with the pyro window absorb checks read), Yoimiya, Chevreuse, Diluc, Gaming, Candace; The Catch, Rust, Serpent Spine, Lion's Roar, The Viridescent Hunt; Crimson Witch of Flames, Song of Days Past (OnEnemyHit, gcsim's Flags.Custom); Yelan's constellations and aimed shots, Zhongli's press skill and C1, Hu Tao's C6.
- **Constellations**: one a character does not implement is now an error (it was silently ignored); Zhongli's C2, C4 and C6 and Escoffier's are not ported (both have since been).
- **Geometry, where it decides hits**: Diluc's burst ticks fly away from the player, and gcsim hits four of eight at Gscouter's target distance. The engine tests such boxes against the target's circle; every other hit on the enemy still lands.
- **Dendro** (with Nahida, Wandering Evenstar, Deepwood Memories): the dendro, quicken, burning-fuel and burning aura slots with gcsim's decay rules; burning and its ticks, quicken, aggravate, spread, bloom (and quicken's bloom), dendro cores as gadgets with burgeon and hyperbloom; dendro resonance. Cores are hit through attack patterns (gcsim's circles, fans and boxes, checked against gcsim's shape code), given so far for Bennett, Yoimiya, Gaming, Hu Tao, Xingqiu's normal attacks, Nahida and the reactions; any other pyro or electro attack that could reach a core is an error. The player takes the reactions' self damage, which drains HP from outside the party. Porting it found content written as if these could not happen: Sapwood Blade did nothing, The Alley Flash was never locked out, and Raiden's Eye did not ignore burning damage.
- **Shields and crystallize** (with Archaic Petra; six teams): shields with HP, element and target, gcsim's absorption, overwrite, expiry and break events and its shield bonus mods (which gcsim drops once they report done; Gscouter's shield stats collector asks on every shield, swap and hit on the player); crystallize shards and `pick_up_crystallize`; Zhongli's A1. Geo resonance came with Chiori, the second geo character (six teams): her dolls, the recast's forced swap (gcsim's Player.Exec from a task), A1 and A4, and gcsim's OnConstructSpawned.

- **Reactions on the player and anemo resonance** (with Jean; seven teams): gcsim's ReactWithSelf, whose swirl's aoe around the player hits the enemy; reaction events on the player, which the subscribers that need an enemy target skip; gcsim's cooldown reduction and stamina percentage mods for anemo resonance.
- **Sucrose** (five teams): her own dash frames, absorb checks over gcsim's pattern (gadgets now have a position and radius), and waits counted as gcsim's stats count them, under the reason an action first failed for.
- **gcsim's results cached** by config and chunk of seeds (`reference/harness/cache.go`), so a rerun after an engine change runs only the engine; CI restores, extends and saves the cache, and runs the library builds on pushes to work branches.
- **The census's one-item teams** (fifty): Wriothesley, Yae Miko, Diona (with shields lengthened by type and Diona's stamina mod), Gilded Dreams, and the hit patterns of Raiden, Fischl and Xiangling. Klee (ten teams) came after: her one team that looked −1.78% at 1,000 iterations passes at 36,000 (+0.01% ± 0.34); its energy waits spiral, which makes its DPS heavy-tailed.
- **Lynette, Shenhe and Thrilling Tales** (31 teams): Lynette's Bogglecat Box is a gadget that takes hits (`Sim::add_hittable_gadget`, `Character::gadget_hit`), so Furina, Rosaria, Escoffier and Xingqiu's skill, burst and orbital got gcsim's hit patterns; Shenhe's quills add flat damage from her ATK at each hit. Two general fixes came with them: patterns keep f64 coordinates (in f32, a hit at exactly the sum of the radii, which every dendro core is to Fischl's A4, landed half the time against gcsim's two thirds), and attack mods skip reaction damage as gcsim's ApplyAttackMods does (cryo resonance's crit rate made cryo swirls crit, and the crits fed Favonius).
- **Bennett's charged attack**: it blocked 19 teams, one of them alone (Bennett / Furina / Rosaria / Escoffier, which passes: team +0.12% ± 0.19); the 141 Bennett teams supported before are byte-identical.
- **Yae Miko's charged attack** (two teams of the 55 it blocked; the rest have other blockers): gcsim's five boxes along the way to the target, of which the three that miss the enemy reach only gadgets (`Sim::queue_gadget_attack`). Both library builds pass (largest team difference −0.04%).
- **Flower of Paradise Lost** (six teams, each blocked by it alone): its bloom, hyperbloom and burgeon bonus on the reaction bonus mods, with gcsim's stacks; its holders (Bennett, Xiangling, Raiden, Xingqiu) cover all three reactions, and every library build passes (largest team difference +0.06% ± 0.14, character Zhongli +0.28% ± 0.28).
- **Thundering Fury** (five teams, the set the census listed at 275 teams blocked): its 4-piece reaction bonuses and skill cooldown cut, on every reaction the engine has; the five library builds and three probes (aggravate, superconduct, hyperbloom) pass, the largest team difference −0.19% ± 0.26.
- **Lisa** (nine teams, all passing with their library builds; largest team difference −0.15% ± 0.17): the enemy's Conductive stacks, C4's discharges on enemy-camp gadgets, and enemy DEF mods for A4. `.tags` conditions read 0 for every tag before; the ones ported content sets and the engine does not keep are now errors.
- **The five merged** (Lisa, Bennett's and Yae Miko's charges, Flower of Paradise Lost, Thundering Fury): 26 teams, three of them needing two of these; the merged engine passes all 34 library and probe builds again.
- **Kuki Shinobu, Hakushin Ring and Sacrificial Jade**, three content branches merged together (22 teams: Kuki fifteen, the ring seven; Sacrificial Jade, listed at 401 teams blocked, alone completed none). Kuki is ported to C6 (probes at C1, C4 and C5), the ring with probes at R1 and on hyperbloom and superconduct, the jade with probes at R1, R5 and `stacks=1`. CI on the merged `main` passes every library build and probe, 319 of 319 (largest team difference among them +0.28% ± 0.29, Klee / Yelan / Kuki / Nahida).
- **gcsim run ahead of a port**: `compare -gcsim-only` runs gcsim alone, into the cache, on the selected builds whether the engine supports their teams or not, to the iterations gcsim's own standard errors need; `compare.yml` takes a `warm` input (team ids) that does it in CI, so dispatched on `main` at the start of a port it leaves the port's push only the engine to run. The cache job now runs one at a time per branch, so that overlapping runs no longer drop each other's chunks.
- **Build-space variants in the runner** (`-only buildspace`, §2): one member's constellation, refinement or weapon changed, in CI in 80 parts. `-deadline` lets a CI part stop starting and growing builds and save its report and gcsim's chunks before the runner's time limit (a Fischl constellation on Bennett / Chevreuse / Fischl / Raiden grew to 512,000 iterations); `import_run` merges a work branch's gcsim chunks into another branch's cache; dispatch inputs lose all whitespace. Its first complete run is below (verification over the player's build). An earlier partial run showed that a team whose bursts wait for energy can need more than the default 512,000 iterations: Klee / Fischl / Xingqiu / Bennett with Xingqiu at C0 (the same fight as C1: gcsim's Xingqiu has no C1 code) was inconclusive at 512,000 and passes at 1,000,000 (team −0.08% ± 0.09, Klee −0.36% ± 0.48; Klee's coefficient of variation 1.74, waits equal within 0.01 s), and Fischl's C0 and C1 on Bennett / Chevreuse / Fischl / Raiden passed only at 512,000.
- **Kuki's C6 probe**: C6 never procs on her library teams (the ring's heals keep her above 25% after each skill); `kuki-c6` drops her EM and raises her HP so that it procs in the scored window, and passes at 1,000 iterations (team −0.04%, Kuki −0.32%; 3.31 procs per fight in gcsim, 3.30 in the engine). `start_hp=1` on a char line is ignored by gcsim and the engine alike.
- **Alhaitham, then Baizhu, Neuvillette and Prototype Amber** (Phase 3 content, taken from the census's queue): Alhaitham made 306 teams supported; the other three, three branches from there merged together, 374 (their 62 teams and six that needed two of them). Each character to C6 with probes C1 to C6, the weapon with probes at R1. Baizhu's teams needed gcsim's hit patterns for Kazuha's normal and charged attacks, A1 and absorbed burst ticks, which can be pyro or electro and reach dendro cores; Neuvillette brought Sourcewater Droplets as gadgets and the `.gadgets.*` conditions. Alhaitham's high and collision plunges need Xianyun's airborne state and stay errors.
- **Chevreuse's hit patterns, Nahida C1 to C6 and Kirara**, three branches merged together (395 teams; Kirara 21). Every Chevreuse attack has gcsim's pattern (`Pattern::FanDir` for the burst's secondary shells), and a Chevreuse config with the target off the y axis is rejected, as Neuvillette's is. Nahida's C2 Lunar-Bloom crit part is unreachable while Lunar-Bloom is an error; Nilou's Bountiful Core joins C2's list when Nilou is ported. Kirara to C6 with probes C0 to C5; her plunges are errors, as gcsim has none. CI on the merged `main` (`17464bf`, run 41264956163) passes every library build and probe.
- **Gadgets by key**: Lynette's Bogglecat Box kills itself in its own tick, and `tick_gadgets` skipped the next gadget and read past the end (found by the build space on Neuvillette / Xiangling / Lynette / Kirara, Lynette on Xiphos' Moonlight). Gadgets carry gcsim's TargetKey, and loops that run hooks follow keys, as gcsim's nil slots; no score moved on 82 configs.
- **Verification over the player's build, first complete run** (`main` at `17464bf`, run 41264960806, parts 0-79): 395 teams, 14,889 builds; 13,733 pass, none fails, and 1,156 the engine does not run, all constellations not ported (Yae Miko 318, Zhongli 280, Wriothesley 192, Escoffier 144, Jean 96, Klee 84, Chiori 42). The 9 inconclusive at 512,000 and the gadget panic's build passed on rerun (`rerun`, cap 2,048,000; runs 41272810063 and 41273779826): Klee / Fischl / Xingqiu / Bennett with Xingqiu C0 to C5 at 750,000 to 1,308,000 iterations, Alhaitham / Xingqiu / Fischl / Raiden with Fischl C4 and C5 at 1,070,000, Wriothesley / Nahida / Lynette / Bennett with Lynette on Sacrificial Sword at 1,626,000. Every build the engine runs passes: 92.2% of the build space runs and is verified. A second complete run on `main` at `5e2276f` (run 41280909462, parts 0-79), with Nahida's, Wriothesley's, Zhongli's and Yae Miko's constellations and the iteration cap raised past 512,000 where gcsim's cache already covers it: 14,928 builds, 14,562 pass, none fails, none is inconclusive, and the other 366 are the constellations of Escoffier (144), Jean (96), Klee (84) and Chiori (42), since ported. 97.5% of the build space runs and is verified.

What remains of Phase 2 is systems and verification, in parallel branches under `CLAUDE.md`'s conditions (at most five unverified; a system that changes a hook or the order within a frame goes alone) (census of 2026-11-01, 395 supported). Content (characters, weapons and sets that need no new system) moves to Phase 3, where it can run in parallel.

1. **Verification over the player's build** (§2; done: second complete run, 2026-11-01, 14,562 of 14,562 builds the engine runs pass, 97.5% of 14,928, none inconclusive; see above): `compare` builds that change a member's constellation, refinement or weapon, run on every supported team; fix what they find. The runner has them (`-only buildspace`, in CI on the daily schedule, 80 parts, 16 a day): on 2026-10-31's 278 supported teams (Gscouter at `14e187f`), 10,478 builds, 6,672 constellations, 1,614 refinements, 2,192 weapons (two per member, which draw 29 of the 30 weapons the engine has; the 30th, Serpent Spine, is what every claymore user in them carries); the engine runs 9,336 of them (89.1%), and the other 1,142 are the constellations the eight characters below C6 lack (Nahida 288, Zhongli 170, Wriothesley 192, Yae Miko 168, Escoffier 126, Jean 84, Klee 78, Chiori 36). Builds as heavy-tailed as Klee / Fischl / Xingqiu / Bennett's need `-max-iter` above 512,000.
2. **The ported characters to C6** (done, 2026-11-01): Nahida, Wriothesley, Zhongli, Yae Miko, Escoffier, Jean, Klee and Chiori (with her low plunge) now run C0 to C6. Then an audit of every ported character's, weapon's and set's gcsim package for actions, params and conditions (branch `claude/audit-actions`): conditions that read 0 silently are fixed (`.element.<t>` for cryo, dendro, quicken, frozen and burning; Zhongli's A1, Diona's C6, Shenhe's quills, Hu Tao's Blood Blossom and other tags), and the charged attacks of Candace, Chevreuse, Escoffier and Xingqiu, Diona's and Yoimiya's aimed shots, aimed weak-spot hits (Fischl, Yelan) and Lynette's held skill are ported, each with a probe. Every param the 34 characters' packages read is handled; stamina had no gap.
3. **Systems** (done, 2026-11-01, in parallel branches; each with a character that uses it, to C6, and probes): Nightsoul with Xilonen and Cinder City's Nightsoul bonus (44 teams; the engine now emits gcsim's change to idle at an animation's end, and OnAttack, OnDash and OnPlunge before an action); HP debt with Finale of the Deep (9 teams; Arlecchino needs more and blocks no team alone); Hexerei for Fischl and Sucrose (9 teams); `set_player_pos` (3 teams), with the engine moved to gcsim's world coordinates (the player has a position and a facing, fans and boxes a direction, hits on the enemy decided by shape; Diluc's burst boxes, spawned from their edge as in gcsim, move his 8 teams' scores); Xianyun and the airborne plunges of every ported character that has one in gcsim (16 teams; teams blocked by actions fall from 81 to 7); Lunar-Charged with Ineffa (4 teams; Lunar-Bloom and Lunar-Crystallize stay errors until their characters); Stellar-Conduct and Polestar Fields with Sandrone, and Yae Miko's and Wriothesley's Radiance. No library team has Sandrone, so Stellar-Conduct is compared only by hand-written teams against gcsim (team differences within 0.13%), not by CI: it stays unverified until a supported team carries it. Stellar Swirl followed with Vesna (2026-11-03), compared by her library teams. Moving the player is an error with any character not checked against gcsim with a moved player (`MOVES_WITH_PLAYER`). On 2026-11-04 the 37 characters of the library's `set_player_pos` teams joined it after an audit of every place their gcsim packages read the player's position or facing, and when: patterns built at the action rather than at the hit (Kazuha, Raiden, Rosaria, Xianyun, Yun Jin, Xiangling, Mavuika, Furina) or the other way round, areas kept from the cast (Nahida's Shrine, Kazuha's absorb and C2 areas, Yae's totems), Baizhu's charge (the enemy is gcsim's offset), Guoba facing the enemy at each breath, and patterns where there were none (Nahida, Kazuha, shatter). Per-ability hits on the 37 library teams that run agreed with gcsim, team DPS within 0.3% at 2,000 iterations.
4. **Thoma** (done, 2026-11-01): ported again from gcsim, C0 to C6 with probes (13 teams). thoma-chevreuse-fischl-bennett is heavy-tailed (Fischl waits about 190 s of 450 for energy): −3.14% at 1,000 iterations, −0.04% at 20,000.
5. **Close the phase** (2026-11-01). The closing run on `main` at `45084fc` (runs 41297707416 for the library builds and probes, 41297719188 for variants 0-39 and build space 0-79, with reruns 41306109358, 41311080309, 41329311641): 496 of 4,570 teams supported (395 that morning). Every library build and probe passes. Build space: 18,718 builds, 18,717 pass, none fails, one the engine does not run (Yelan's burst hit has no pattern beside Lynette's box: `unsupported: hit pattern yelan Lingering Lifeline`, on neuvillette-yelan-lynette-xilonen with Lynette on Harbinger of Dawn), and two needed more than 2,048,000 iterations: thoma-chevreuse-fischl-bennett with Chevreuse on Deathmatch or Ballad of the Fjords (the same fight), inconclusive at 2,048,000 (Thoma −0.58% ± 0.80), pass at 5,405,500 (run 41358307263, 64 shards: team −0.04% ± 0.15, Thoma −0.18% ± 0.50). Variants: 4,262 builds, 4,258 pass, two are refused by gcsim and the engine alike (`both-error`: nahida-yaemiko-furina-xianyun's forced-ER variants plunge without Xianyun's burst), and two stay inconclusive at 2,048,000: neuvillette-yelan-kaedeharakazuha-zhongli's er-0-forced (Kazuha 0.0069 against 0.0072 DPS; nobody but Zhongli acts) and er-half-forced (Kazuha −0.27% ± 1.23, Neuvillette −0.54% ± 1.63, Yelan −0.49% ± 1.67), which would need tens of millions of iterations. The run made the runner treat a build both engines refuse as agreement and DPS under 1e-6 on both sides as equal (gcsim leaves 2e-12 where the engine has 0). Speed: 500 iterations of `team-looped.txt` on one thread take 1.10 s on this session's machine (median of 7); valgrind's lackey counts 487.96 million instructions for 20 iterations, 4.1% more than the 468.6 million before this round's systems (world coordinates, Nightsoul's events, lunar and Stellar reactions), against 451 million at the start of the phase.

6. **After the phase** (2026-11-02). Yelan's attacks have gcsim's hit patterns now (normal attacks, C6's barbs, Lingering Lifeline, the burst's first hit, Exquisite Throw and its C2 hit), so the one build-space build the engine did not run (neuvillette-yelan-lynette-xilonen with Lynette on Harbinger of Dawn) runs: −0.27% for the team at 1,000 iterations, hits per ability equal, and its rerun in CI passes.

**Known inconclusive builds** (2026-11-02). Three teams have builds that stay inconclusive, none failing. They are recorded here and left: a team with an inconclusive build stays unverified in the release manifest, so Gscouter keeps it on gcsim, and the bound is not loosened for them. Come back to them when their characters' code changes, or to trace the Raiden lean below.

| Team | Build | Iterations | Who | gcsim | gse | Difference | ±95% |
|---|---|---|---|---|---|---|---|
| neuvillette-yelan-kaedeharakazuha-zhongli | variant `er-half-forced` | 2,048,000 | Kazuha | | | −0.27% | 1.23% |
| | | | Neuvillette | | | −0.54% | 1.63% |
| | | | Yelan | | | −0.49% | 1.67% |
| neuvillette-yelan-kaedeharakazuha-zhongli | variant `er-0-forced` | 2,048,000 | Kazuha | 0.0072 | 0.0069 | (DPS near 0; nobody but Zhongli acts) | |
| neuvillette-fischl-nahida-raidenshogun | page `page-er-0:looped:0.4` | 512,000 | team | 7,436.2 | 7,436.5 | +0.00% | 0.07% |
| | | | Raiden | 97.4 | 98.0 | +0.66% | 0.62% |
| neuvillette-fischl-nahida-raidenshogun | page `page-er-0:looped:0.7` | 512,000 | team | 7,628.2 | 7,629.3 | +0.01% | 0.07% |
| | | | Raiden | 83.0 | 83.3 | +0.36% | 0.73% |
| neuvillette-fischl-nahida-raidenshogun | page `page-er-0:looped:0.23` | 512,000 | team | 7,034.3 | 7,037.8 | +0.05% | 0.09% |
| | | | Raiden | 185.4 | 186.0 | +0.33% | 0.83% |
| yoimiya-yaemiko-chevreuse-thoma | page `page-er-0:looped:0.26` | 1,500 (deadline) | team | 31,339.8 | 31,277.6 | −0.20% | 0.62% |
| | | | Thoma | 2,573.6 | 2,548.2 | −0.99% | 0.80% |

- The Kazuha team's two variants would need tens of millions of iterations; its library build, other variants and build space pass.
- Raiden is a filler in those page configs (83 to 186 DPS) and leans the same way, + 0.33% to + 0.66%, in all three; every other character there agrees within 0.2%. It may be a real bias of a few tenths of a percent in some small part of her damage: a lead, not a failure. Traced on `page-er-0:looped:0.4` (2026-11-02, local, 500,000 iterations each): Raiden +0.62% ± 0.63, and the one ability apart is her hyperbloom damage in the window, 0.0930 hits a fight against gcsim's 0.0888 (z = 6.9; Eye of Stormy Judgement, electro-charged and waits agree). Over the whole fight the engine triggers as many hyperblooms as gcsim, by ability (her Eye strikes 36.13 a fight against 36.12, 200,000 iterations), and nearly all of them come before 150 s; the window holds only the rare fights where cores still form after it. So the lean is in that tail, the fights whose blooms go on past 150 s, not in how a core or a hit works. Next step if it matters: compare when the last bloom falls, fight by fight, against gcsim's debug log.
- The Thoma build stopped at the 280-minute deadline after 1,500 iterations; Thoma's teams are heavy-tailed (the build-space builds went from −3.14% at 1,000 iterations to −0.04% at 20,000).
- The page configs ran only in 16 of the 80 page parts so far (the schedule of 2026-11-02); the other 64 may add to this list. A rerun of the four page builds to 2,048,000 iterations (run 41412604050) was started on 2026-11-02.

Weapons and sets that block many teams (Ocean-Hued Clam 391, Obsidian Codex 385, Oathsworn Eye 352) are Phase 3 content; Thrilling Tales (497, ten teams it alone blocked), Sacrificial Jade (401) and Hakushin Ring (216) are in. Catalysts can now be checked on Nahida: Wandering Evenstar is ported on her teams, and The Widsith (700 teams) passes its probe on one, though it alone unblocks none.

### Phase 3: content (most of the work)

- **Weapons (261) and sets (55).** Small one by one, but all are needed: a player's build can carry any of them, library or not (about 100 weapons and 20 sets appear in no library team). Port the ones real builds carry most first.
- **Characters**, in the order of the table in §1: 40 cover a quarter of the library, 100 cover 86%.
- Taken as a queue in parallel branches, a few at a time, merged and compared again together (on 2026-10-31 five branches made 26 teams supported, three of them needing two of the branches, and the merged engine passed all 34 builds). At 256 supported, 312 items blocked 4,314 teams, and taken in the order that completes the most teams 75 of them gave 50% and 150 gave 86%. The first ten in that order were Xilonen, Citlali, Finale of the Deep, Sacrificial Jade, Neuvillette, Kuki Shinobu, Alhaitham, Hakushin Ring, Baizhu, Ocean-Hued Clam; Sacrificial Jade, Kuki Shinobu, Hakushin Ring (22 teams), Alhaitham, Baizhu and Neuvillette (with Prototype Amber, 96 teams) are done.
- The queue now (census on `main` at `41b703f`, 395 supported): 303 items block the 4,175 teams not supported; 1,324 teams are one or two items from support, 585 of them one. The items that alone block the most teams: Xilonen 43, Citlali 34, Ororon 19, Beidou 16, Cyno 16, Tighnari 15, Dori 14, Ocean-Hued Clam 14, Layla 14, Thoma 13, Charlotte 12, Collei 11; then Yaoyao, Dehya, Mona, the pyro Traveler and Keqing at 10, Iron Sting, Finale of the Deep, the dendro Traveler, Thundersoother and Hexerei at 9. Counted over the teams one or two items away: Xilonen 110, Citlali 55, Ocean-Hued Clam 51, Fragment of Harmonic Whimsy 50, Arlecchino 49, Unfinished Reverie 46, Finale of the Deep 46. Xilonen, Citlali and Ororon need Nightsoul, Arlecchino and Finale of the Deep HP debt, and Thoma is held out (Phase 2, step 4); items that need a Phase 2 system wait for it.
- Done is §2's: every constellation, refinement and set count, verified over the build space, not only the library's builds.
- **Nefer** (2026-11-04, branch `claude/port-nefer`): the one item left blocking library teams once the Traveler, Aino / Flins / Lohen and Barbara branches merge (16 teams, 10 of them alone; the other 6 also need Aino). C0 to C6, all three charged-attack routes, Seeds of Deceit (a dendro-core-type gadget that reacts to nothing and never explodes; `SEED_CORE`), Veil of Falsehood, and the Verdant Dew rate mods (`Sim::dew_rate_mods`). Her held charged attack (Slither) moves the player toward the enemy each frame, the first content to move the player in the fight; every member then needs `MOVES_WITH_PLAYER`, so Lauma (on all 16 teams), Linnea and Kokomi joined it after the audit in `reference/audits/nefer-moves.md`. Collei and Zhongli (two of the Aino teams) have not been audited and stay errors there. Probes: `nefer-c1` to `nefer-c6`, `nefer-charge` and `nefer-c6-charge` (normal attacks, tapped charged attacks, the ordinary charged attack after a short Slither). Ascendant Gleam is on in every library team (Lauma is Moonsign too), so Nefer without it (her A1 and C2's veil off, C6's elevation off) is reached by no comparison.
- **The last weapons and sets** (2026-11-04, `claude/port-remaining-weapons`): every key in `data::WEAPON_KEYS` and `data::SET_KEYS` is ported. No library build carries these 89 weapons and 6 sets, so each is checked by probes, each weapon at R1 and R5 and each set at 2pc and 4pc (190 probes, CI run 41804839333). What those probes cannot reach stays unverified:
  - **Kills.** Library fights have no enemy kills: the target has no HP, and the fight ends at its duration. So the passives that need a kill never trigger: Blackcliff (Agate, Longsword, Pole, Slasher, Warbow), Gambler 4pc (skill reset), Twin Nephrite and White Iron Greatsword's heal. Their probes check base stats only. To reach them, add a probe with a target that has HP (`target ... hp=`) and the wearer on field.
  - **Enemy HP and enemy attacks.** Braveheart 4pc needs an enemy above 50% HP, which only damage mode has. Beacon of the Reed Sea's damage-taken part and Golden Majesty's shield strength bonus need the enemy to hit the player. To reach them, add a probe with an enemy that has HP and attacks.
  - **Stellar reactions from a spear.** The Stellar parts of Song of the Vigil and Frostbreath need their holder to trigger Stellar-Conduct or Stellar Swirl. In the library only Sandrone, Odette and Vesna trigger them, and none of them holds a spear. To reach them, add a hand-written team with a spear user beside Sandrone or Vesna.
  - **Moondrifts.** Golden Frostbound Oath's Lunar-Crystallize and Moondrift team parts need Moondrifts. Its probe (`goldenfrostboundoath-r*-gorou`) has none. A probe on a library team that makes Moondrifts would reach them.
  - **Range.** King's Squire's fallback onto the player needs the enemy more than 15 away. Every library target is within 15.
  - **Hexerei and Moonsign.** Daybreak Chronicles' Hexerei doubling needs a Hexerei pair. Snare Hook's Moonsign doubling needs a Moonsign team. Their probes' teams have neither. A probe on a Hexerei team (Fischl, Sucrose) and one on a Moonsign team would reach them.
  - **Bond of Life.** Silvershower Heartstrings' Bond of Life stack is not shown to be reached on Sigewinne's team. A probe on Arlecchino or another Bond of Life character would reach it.
  - **Stamina.** Wine and Song's dash stamina reduction needs the wearer's normal attack hit before a dash. Its host, Sucrose, never normal-attacks. Stamina binds in no library rotation, so Sturdy Bone's −15% is not tested either.
  - **Heal target.** Otherworldly Story heals slot 0, as gcsim's HealInfo does. On its probe the wearer is in slot 0, so it is not told apart from the wearer.
  - **Errors instead of differences.** Gest of the Mighty Wolf on Noelle, Itto or Mavuika needs gcsim's StateChange `segmented` flag. Cloudforged on Sethos, or with Shimenawa 4pc, needs gcsim's OnEnergyChange. Both are errors raised at fight setup, and `gse --check` does not list them.
