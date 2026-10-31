# gscouter-engine

An experiment: would a simulation engine written only for Gscouter, in Rust, be faster than gcsim and still score within 1% of it?

It ports, from gcsim's source (Gscouter's fork, `cenoda/gcsim@eb21884`), what 256 library teams need, scored under Gscouter's looped conditions (450 s, window from 150 s, Lv. 100 enemy at 10% resistance, energy every 8–12 s, swap delay 12): twenty-eight characters (Hu Tao, Xingqiu, Zhongli, Yelan, Bennett, Fischl, Furina, Kazuha, Raiden, Xiangling, Yoimiya, Chevreuse, Rosaria, Escoffier, Diluc, Gaming, Candace, Nahida, Chiori, Jean, Sucrose, Wriothesley, Yae Miko, Diona, Klee, Lynette, Shenhe, Lisa), 28 weapons and 16 sets. Phase 1 of the roadmap (`docs/roadmap.md`) is done: a comparison runner, data generated from gcsim, and a structure that characters, weapons and sets attach to. Phase 2 is under way: electro, cryo and dendro with their reactions (overload, electro-charged, swirl, melt, freeze and shatter, superconduct; burning, quicken, aggravate, spread, bloom with its dendro cores, burgeon, hyperbloom), heals and drains, hits on the player, shields with HP and crystallize shards (`pick_up_crystallize`), the player's aura, gadgets and the attack patterns that hit them, reactions on the player (Jean's self swirl), and every resonance are in.

It reads gcsim's config language as gcsim does: the same lexer, grammar and evaluator (`let`, `fn`, `if`/`else`, `while`, `for`, `switch` with `fallthrough`, `break`/`continue`/`return`, action lists with params and repeats, `.char.skill.ready` and the other fields, `wait`, `delay`, `execute_action` and the other built-ins), and the same setup lines (`options`, `target`, `energy`, `char`, `add weapon`, `add set`, `add stats`, `active`). A config gcsim runs, the engine runs unchanged, as long as its characters, weapons, sets and actions are ones the engine implements; anything else is an error naming it, never a silent difference.

## Results

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

A weapon, set or character that no supported team carries would go unchecked, so the runner also runs probes (`reference/probes`): a library team's build with some lines replaced, compared like a variant. A probe's team need not be supported, only the probe build, so a new character is checked on its own library team with the weapons the engine lacks swapped out (Fischl on Hu Tao / Fischl / Yelan / Bennett before The Stringless and Ballad of the Fjords were ported, Furina at C0, C2 and C6). Every probe passes; each weapon and set without a supported team to carry it has one. Waits by reason and hits and damage by ability are in the runner's report as well; waits agree within about 0.1 s per fight (e.g. Xingqiu waiting for energy 75.38 s / 75.29 s). The first team's rotation checks `.burst.ready` before each burst, so a build short of ER skips bursts instead of waiting for energy; the runner adds builds whose bursts are used without the checks so that the waits Gscouter's fillers depend on are compared too. Those fights are mostly waiting, which is why they need hundreds of thousands of iterations.

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

The runner also classifies every library team. Of 4,570, 256 are supported and 4,314 are not; what blocks the most teams:

| Kind | Most teams blocked |
|---|---|
| Characters | Xilonen 482, Xianyun 314, Mavuika 277, Neuvillette 254, Durin 241, Citlali 221, Emilie 214, Ororon 213, Nicole 191, Arlecchino 184 |
| Weapons | Sacrificial Jade 401, Oathsworn Eye 352, Finale of the Deep 336, Chain Breaker 226, Hakushin Ring 216, Prototype Amber 211 |
| Sets | Ocean-Hued Clam 391, Obsidian Codex 385, Celestial Gift 332, Fragment of Harmonic Whimsy 283, Long Night's Oath 257, Unfinished Reverie 250 |
| Built-ins | `set_player_pos` 45, `set_on_tick` 1 |
| Actions of ported characters | plunges without Xianyun 69, Xingqiu's charge 3, Diona's and Yoimiya's aim 2 each |
| Constellations not ported | Nahida C2 and C6, 19; Chiori C6, 12; Jean C4, 7; Wriothesley C1 and C6, 7; Klee C1, C2 and C6, 7; Yae Miko C1 and C6, 6; Zhongli C2 and C6, 5 |
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

Phase 2 so far costs about 4% more instructions on `team-looped.txt` (valgrind, 20 iterations: 468.3 million against 451 million at the start of the phase): attacks carry more fields (blunt, poise, from the fight itself, a hit pattern, now in f64, +0.6%) and there are nine aura slots, which the aura's tick skips while they are empty. Lisa added 0.24% (valgrind's lackey, 20 iterations: 471.4 million before, 472.5 million after; the damage formula now sums the enemy's DEF mods with its resistances), and the merged round of Lisa, Bennett's and Yae Miko's charges, Flower of Paradise Lost and Thundering Fury counts the same 472.5 million. It had reached 480.0 million before the animation tick stopped copying the action out and back each frame (−3%, with byte-identical output). Wall time on this session's machine is too noisy to show it (500 iterations, one thread, seven runs: median 1.31 s at the start of this round and after it). Phase 1 kept the speed. On this session's machine (slower than the one above), 500 iterations on one thread take 1.47 s (median of 7) against 1.46 s for Phase 0, +0.8%; 10,000 iterations on 4 threads take 7.6 s; the Bennett team takes 1.32 s for 500. The two reference configs still give the same scores to the last digit (72,266.73 and 24,607.03 at 1,000 iterations, checked by `engine/tests/scores.rs`), and the restructure gave byte-identical output on every config.

The WebAssembly row is the one that matters for the site, which scores in the visitor's browser. Go's WebAssembly runs gcsim about 5× slower than native Go; Rust's runs about 1.4× slower than native Rust. The gcsim side is the reference harness built with `GOOS=wasip1`; Gscouter's site uses `GOOS=js`, which has the same compiler backend.

What it would take to score Gscouter's results, in phases: `docs/roadmap.md`.

## What it does not do

- Only the abilities these teams use, these weapons (Staff of Homa, Sacrificial Sword, Aqua Simulacra, Deathmatch, The Alley Flash, Harbinger of Dawn, the five Favonius weapons, The Stringless, Ballad of the Fjords, Wolf-Fang, The Alley Hunter, Sapwood Blade, Festering Desire, Slingshot, Dragon's Bane, Xiphos' Moonlight, The Catch, Rust, Serpent Spine, Lion's Roar, The Viridescent Hunt, Wandering Evenstar, The Widsith, Thrilling Tales of Dragon Slayers) and sets (Shimenawa, Emblem, Tenacity, Instructor, Noblesse Oblige, Golden Troupe, Scroll of the Hero of Cinder City, Marechaussee Hunter, Viridescent Venerer, Crimson Witch of Flames, Song of Days Past, Deepwood Memories, Archaic Petra, Gilded Dreams, Flower of Paradise Lost, Thundering Fury), and every resonance. Every other character, weapon, set, action, constellation, resonance, built-in or target option is an error that names it (`unsupported: <what> <name>`), never a silent difference. Each character declares the highest constellation it implements (Zhongli C1, Escoffier, Nahida, Chiori, Jean, Wriothesley and Yae Miko C0, the others C6; Sucrose's Hexerei bonuses need a second Hexerei character, which is an error).
- One target. The player stands at the origin facing it and never moves; hits are taken to land, except where gcsim's hitbox decides it for a target at Gscouter's distance (Diluc's burst, whose ticks fly away from the player: a box against the target's circle, `Sim::box_hits`). Gadgets are hit by gcsim's patterns (circles, fans and boxes) where an attack has one; a pyro or electro attack without one that could reach a dendro core is an error. The player is hit by the self damage of burning, bloom, burgeon and hyperbloom when their area reaches it (bloom's and burgeon's do at Gscouter's distance), which shields absorb and the rest drains from the active character; Diluc's C2 (being hit) is an error. `target type=`, particle drops, `hurt`, random substats and `set_on_tick` are errors, as are the position built-ins.
- Of the reactions, all but the lunar ones: vaporize, melt, overload, electro-charged, superconduct, freeze and shatter, swirl, crystallize, burning, quicken, aggravate, spread, bloom, burgeon and hyperbloom. Of reactions on the player, only swirls (the only ones gcsim's characters here cause); no Nightsoul, no HP debt, no fillers (Gscouter's `steady` package).
- Tags (gcsl's `.tags` and `.<char>.tags`) read 0, as gcsim's do when unset, except the enemy's Conductive stacks (Lisa's); the tags ported content sets that the engine does not keep are errors.
- Random numbers are its own (xoshiro256++), so single fights differ from gcsim's; only the averages compare.

## How it works

`engine/src/gcsl.rs` is gcsim's lexer and parser (`pkg/gcs/ast`, `pkg/gcs/parser`), `engine/src/eval.rs` its evaluator (`pkg/gcs/eval`), run as gcsim's `RunSync` runs it: each action is handed to the fight, which runs until it needs the next one. `engine/src/profile.rs` sets characters up as gcsim's `AddChar` does.

`engine/src/sim.rs` keeps gcsim's order of work within a frame (enemy queue and gauge decay, constructs, stamina, shields, animation, character queues, core queue, energy drops), its action state machine, hitlag (frozen characters and enemies, extended mods, queues and infusions), mods and their expiry rules, ICD tags and groups, per-character gauge slots, the reactions of `Reactable.React` with their events and reaction damage (frozen's decay, poise and shatter, burning's ticks and its fuel's hold on dendro and quicken included), gadgets with their own auras and dendro cores, attack patterns for the gadgets an attack reaches, hits on the player, heals and drains with their events and incoming healing bonus, weapon infusion and fight-wide statuses. It names no character, weapon or set: they attach through `engine/src/content.rs`.

- Each character is a module in `engine/src/chars`, each weapon in `engine/src/weapons`, each set in `engine/src/sets`, implementing `Character`, `WeaponImpl` or `SetImpl`: hooks with default no-ops for its actions, its tasks, the callbacks of its attacks, the amounts of its mods, its attack mods, the events it subscribes to (enemy damage, skill, swap, state change, reactions), its frames and its condition fields. One line in `chars!`, `weapons!` or `sets!` registers it; the macros generate the enum of kinds, the enum of their states and the `match` that dispatches to them, so calls are static and the hot path has no dynamic dispatch or allocation.
- Mods have a key hashed at compile time that carries gcsim's name (which `.char.mods.<key>` reads), and either a fixed amount of one stat or an owner that computes it. Tasks are a character's, a weapon's or a set's, with an id and an argument. Events go to subscribers in gcsim's subscription order, so random numbers are drawn in the same order.
- Data comes from gcsim: `reference/gendata` copies the gcsim module into a temporary directory, adds to every character, weapon and artifact package a file registering its package variables and constants, and runs a dumper there, so values are what gcsim computes, frame tables built by `init` included. It writes `engine/src/data.rs` (names; base stats of every character and weapon; what each character's `NewChar` and `Init` set: element, weapon class, body, energy, normal hit count, talent constellations, animation start delays) and `engine/src/gcsimdata` (a module per package: talent multipliers, frames, hitmarks, hitlag, mod keys; and every ICD tag and group). It covers all 168 member keys of the library, 252 weapons and 55 sets.

Porting a character is then its logic only: Bennett is 240 lines, the four weapons and Instructor 25 to 40 each. The fight core is 2,200 lines, the content interface 350.

## Running

```
cd engine && cargo build --release && cargo test --release
./target/release/gse ../configs/team-looped.txt 10000 150 4   # config, iterations, warmup, threads [--seed N]
./target/release/gse --check ../configs/team-looped.txt        # what the config needs that the engine lacks

cd reference
go run . -config ../configs/team-looped.txt -iter 10000 -warmup 150 -j 4 -abil   # the same under gcsim
go run ./compare -gscouter ../../Gscouter -json compare.json -md compare.md      # the whole library
go run ./compare -gscouter ../../Gscouter -teams hutao-yelan-bennett-xingqiu     # one team
go run ./compare -gscouter ../../Gscouter -only variants -shard 3/40             # the variants of one part of the teams
go run ./gendata                                                                 # regenerate engine/src/data.rs and engine/src/gcsimdata
go run ./dump ../configs/team-looped.txt                                         # base stats as gcsim sets them up
```

`compare` exits with 1 when a comparison fails or stays inconclusive. `-only library,variants,probes` picks the kinds of build to compare, `-shard k/n` the k-th of n parts of the teams (the census still covers every team), and `-dry-run` lists the builds without comparing them. CI (`.github/workflows/compare.yml`) runs it with Gscouter at a fixed commit; gcsim is fixed by `reference/go.mod`. Every push compares the library builds and probes in four parallel parts. The variants take about an hour per team, so a daily schedule runs 8 of their 40 parts, every team's variants once in five days; a manual run takes the parts to run (`0-39` for all). The reports are the job summaries and artifacts. gcsim's results are cached by config and chunk of 500 seeds (`reference/harness/cache.go`; `-cache <dir>` on `compare` and the harness, `~/.cache/gscouter-engine/gcsim` by default, empty for none): gcsim is fixed and a seed always gives the same fight, so a stored chunk is what running it again gives, and only the engine runs again for an engine change. `compare` rounds its batches up to whole chunks. In CI each job restores the latest cache and uploads what it added, and a last job merges them into the next (one at a time per branch, so that none drops another's chunks). gcsim's results do not depend on the engine, so `compare -gcsim-only` runs gcsim alone on the selected builds, of teams the engine does not support too, into the cache, to the iterations gcsim's own standard errors need; a manual run of the workflow with `warm` set to team ids does it in CI. Dispatched on `main` at the start of a port, it leaves the port's compare only the engine to run. Raise `version` in `cache.go` when the harness changes what it measures; a new gcsim (`go.sum`) starts afresh by itself.

WebAssembly: `cargo build --release --target wasm32-wasip1` and `GOOS=wasip1 GOARCH=wasm go build`, run with `node:wasi` (not remeasured in Phase 1).

## License

GNU AGPL v3 or later (`LICENSE`). The engine is ported from gcsim, which is under the AGPL v3 since 2026-09-19, so it keeps gcsim's license and copyright notice.
