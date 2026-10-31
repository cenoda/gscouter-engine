# gscouter-engine

An experiment: would a simulation engine written only for Gscouter, in Rust, be faster than gcsim and still score within 1% of it?

It ports, from gcsim's source (Gscouter's fork, `cenoda/gcsim@eb21884`), what 125 library teams need, scored under Gscouter's looped conditions (450 s, window from 150 s, Lv. 100 enemy at 10% resistance, energy every 8–12 s, swap delay 12): nineteen characters (Hu Tao, Xingqiu, Zhongli, Yelan, Bennett, Fischl, Furina, Kazuha, Raiden, Xiangling, Yoimiya, Chevreuse, Rosaria, Escoffier, Diluc, Gaming, Candace, Nahida, Chiori), 27 weapons and 13 sets. Phase 1 of the roadmap (`docs/roadmap.md`) is done: a comparison runner, data generated from gcsim, and a structure that characters, weapons and sets attach to. Phase 2 is under way: electro, cryo and dendro with their reactions (overload, electro-charged, swirl, melt, freeze and shatter, superconduct; burning, quicken, aggravate, spread, bloom with its dendro cores, burgeon, hyperbloom), heals and drains, hits on the player, shields with HP and crystallize shards (`pick_up_crystallize`), the player's aura, gadgets and the attack patterns that hit them, and pyro, hydro, electro, cryo, dendro and geo resonance are in.

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

The runner also classifies every library team. Of 4,570, 125 are supported and 4,445 are not; what blocks the most teams:

| Kind | Most teams blocked |
|---|---|
| Characters | Xilonen 482, Xianyun 314, Mavuika 277, Neuvillette 254, Durin 241, Yae Miko 239, Citlali 221, Emilie 214, Ororon 213, Sucrose 212 |
| Weapons | Thrilling Tales of Dragon Slayers 497, Sacrificial Jade 401, Oathsworn Eye 352, Finale of the Deep 336, Chain Breaker 226, Hakushin Ring 216 |
| Sets | Gilded Dreams 417, Ocean-Hued Clam 391, Obsidian Codex 385, Celestial Gift 332, Flower of Paradise Lost 290, Fragment of Harmonic Whimsy 283 |
| Resonances | anemo 263, geo with characters not ported (Noelle, Yun Jin, Albedo, Navia and others) |
| Built-ins | `set_player_pos` 45, `set_on_tick` 1 |
| Actions of ported characters | Zhongli's burst 36, Bennett's charge 19, plunges without Xianyun 43 |
| Constellations not ported | Nahida C2 and C6, 19; Zhongli C2 and C6, 5 |
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

Phase 2 so far costs about 6% more instructions on `team-looped.txt` (valgrind, 20 iterations: 479 million against 451 million at the start of the phase; dendro added 1.7%, from 471 million): attacks carry more fields (blunt, poise, from the fight itself, a hit pattern) and there are nine aura slots, which the aura's tick skips while they are empty. Wall time on this session's machine is too noisy to show it (500 iterations, one thread, ten runs alternated: median 2.15 s against 2.17 s). Phase 1 kept the speed. On this session's machine (slower than the one above), 500 iterations on one thread take 1.47 s (median of 7) against 1.46 s for Phase 0, +0.8%; 10,000 iterations on 4 threads take 7.6 s; the Bennett team takes 1.32 s for 500. The two reference configs still give the same scores to the last digit (72,266.73 and 24,607.03 at 1,000 iterations, checked by `engine/tests/scores.rs`), and the restructure gave byte-identical output on every config.

The WebAssembly row is the one that matters for the site, which scores in the visitor's browser. Go's WebAssembly runs gcsim about 5× slower than native Go; Rust's runs about 1.4× slower than native Rust. The gcsim side is the reference harness built with `GOOS=wasip1`; Gscouter's site uses `GOOS=js`, which has the same compiler backend.

What it would take to score Gscouter's results, in phases: `docs/roadmap.md`.

## What it does not do

- Only the abilities these teams use, these weapons (Staff of Homa, Sacrificial Sword, Aqua Simulacra, Deathmatch, The Alley Flash, Harbinger of Dawn, the five Favonius weapons, The Stringless, Ballad of the Fjords, Wolf-Fang, The Alley Hunter, Sapwood Blade, Festering Desire, Slingshot, Dragon's Bane, Xiphos' Moonlight, The Catch, Rust, Serpent Spine, Lion's Roar, The Viridescent Hunt, Wandering Evenstar, The Widsith) and sets (Shimenawa, Emblem, Tenacity, Instructor, Noblesse Oblige, Golden Troupe, Scroll of the Hero of Cinder City, Marechaussee Hunter, Viridescent Venerer, Crimson Witch of Flames, Song of Days Past, Deepwood Memories, Archaic Petra), and pyro, hydro, electro, cryo, dendro and geo resonance. Every other character, weapon, set, action, constellation, resonance, built-in or target option is an error that names it (`unsupported: <what> <name>`), never a silent difference. Each character declares the highest constellation it implements (Zhongli C1, Escoffier, Nahida and Chiori C0, the others C6).
- One target. The player stands at the origin facing it and never moves; hits are taken to land, except where gcsim's hitbox decides it for a target at Gscouter's distance (Diluc's burst, whose ticks fly away from the player: a box against the target's circle, `Sim::box_hits`). Gadgets are hit by gcsim's patterns (circles, fans and boxes) where an attack has one; a pyro or electro attack without one that could reach a dendro core is an error. The player is hit by the self damage of burning, bloom, burgeon and hyperbloom when their area reaches it (bloom's and burgeon's do at Gscouter's distance), which shields absorb and the rest drains from the active character; Diluc's C2 (being hit) is an error. `target type=`, particle drops, `hurt`, random substats and `set_on_tick` are errors, as are the position built-ins.
- Of the reactions, all but the lunar ones: vaporize, melt, overload, electro-charged, superconduct, freeze and shatter, swirl, crystallize, burning, quicken, aggravate, spread, bloom, burgeon and hyperbloom. No reactions on the player (Jean's burst swirls the player's aura), no Nightsoul, no HP debt, no fillers (Gscouter's `steady` package).
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

`compare` exits with 1 when a comparison fails or stays inconclusive. `-only library,variants,probes` picks the kinds of build to compare, `-shard k/n` the k-th of n parts of the teams (the census still covers every team), and `-dry-run` lists the builds without comparing them. CI (`.github/workflows/compare.yml`) runs it with Gscouter at a fixed commit; gcsim is fixed by `reference/go.mod`. Every push compares the library builds and probes in four parallel parts. The variants take about an hour per team, so a daily schedule runs 8 of their 40 parts, every team's variants once in five days; a manual run takes the parts to run (`0-39` for all). The reports are the job summaries and artifacts.

WebAssembly: `cargo build --release --target wasm32-wasip1` and `GOOS=wasip1 GOARCH=wasm go build`, run with `node:wasi` (not remeasured in Phase 1).

## License

GNU AGPL v3 or later (`LICENSE`). The engine is ported from gcsim, which is under the AGPL v3 since 2026-09-19, so it keeps gcsim's license and copyright notice.
