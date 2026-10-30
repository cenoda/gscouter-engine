# gscouter-engine

An experiment: would a simulation engine written only for Gscouter, in Rust, be faster than gcsim and still score within 1% of it?

It ports, from gcsim's source (Gscouter's fork, `cenoda/gcsim@eb21884`), what ten library teams need, scored under Gscouter's looped conditions (450 s, window from 150 s, Lv. 100 enemy at 10% resistance, energy every 8–12 s, swap delay 12): seven characters (Hu Tao, Xingqiu, Zhongli, Yelan, Bennett, Fischl, Furina), 24 weapons and 9 sets. Phase 1 of the roadmap (`docs/roadmap.md`) is done: a comparison runner, data generated from gcsim, and a structure that characters, weapons and sets attach to. Phase 2 is under way: electro, overload and electro-charged, and heals and drains, are in.

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

The eight teams Phase 2 has made supported so far, each with its library build and its variants (forced bursts only where the rotation checks `.burst.ready`), from their latest runs (Gscouter at `abf4ae3`):

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

A weapon, set or character that no supported team carries would go unchecked, so the runner also runs probes (`reference/probes`): a library team's build with some lines replaced, compared like a variant. A probe's team need not be supported, only the probe build, so a new character is checked on its own library team with the weapons the engine lacks swapped out (Fischl on Hu Tao / Fischl / Yelan / Bennett before The Stringless and Ballad of the Fjords were ported, Furina at C0, C2 and C6). Every probe passes; each weapon and set without a supported team to carry it has one. Waits by reason and hits and damage by ability are in the runner's report as well; waits agree within about 0.1 s per fight (e.g. Xingqiu waiting for energy 75.38 s / 75.29 s). The first team's rotation checks `.burst.ready` before each burst, so a build short of ER skips bursts instead of waiting for energy; the runner adds builds whose bursts are used without the checks so that the waits Gscouter's fillers depend on are compared too. Those fights are mostly waiting, which is why they need hundreds of thousands of iterations.

The runner also classifies every library team. Of 4,570, 10 are supported and 4,560 are not; what blocks the most teams:

| Kind | Most teams blocked |
|---|---|
| Characters | Nahida 683, Kazuha 487, Xilonen 482, Xiangling 383, Raiden 377, Escoffier 325, Xianyun 314, Chevreuse 296, Rosaria 286, Mavuika 277 |
| Weapons | Serpent Spine 942, The Widsith 700, Xiphos' Moonlight 526, Thrilling Tales of Dragon Slayers 497, Wandering Evenstar 470, Sacrificial Jade 401, Oathsworn Eye 352, Finale of the Deep 336 |
| Sets | Viridescent Venerer 1,290, Deepwood Memories 946, Gilded Dreams 417, Ocean-Hued Clam 391, Obsidian Codex 385, Celestial Gift 332 |
| Resonances | electro 961, cryo 774, geo 418, dendro 328, anemo 263 |
| Built-ins | `pick_up_crystallize` 511, `set_player_pos` 45, `set_on_tick` 1 |
| Actions of ported characters | Yelan's aim 134, Zhongli's press skill 46, Zhongli's burst 36, Bennett's charge 19 |
| Other | two Hexerei characters 394 (Fischl with Mona, Sucrose, Razor and the others) |

A team is usually blocked by five to ten items at once, so porting the item that blocks the most teams rarely makes a team supported by itself: the ten items that blocked the most teams at the start of Phase 2 together made none supported. The report also lists the teams one or two items from support, which is how the eight teams above were reached.

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

Phase 1 kept the speed. On this session's machine (slower than the one above), 500 iterations on one thread take 1.47 s (median of 7) against 1.46 s for Phase 0, +0.8%; 10,000 iterations on 4 threads take 7.6 s; the Bennett team takes 1.32 s for 500. The two reference configs still give the same scores to the last digit (72,266.73 and 24,607.03 at 1,000 iterations, checked by `engine/tests/scores.rs`), and the restructure gave byte-identical output on every config.

The WebAssembly row is the one that matters for the site, which scores in the visitor's browser. Go's WebAssembly runs gcsim about 5× slower than native Go; Rust's runs about 1.4× slower than native Rust. The gcsim side is the reference harness built with `GOOS=wasip1`; Gscouter's site uses `GOOS=js`, which has the same compiler backend.

What it would take to score Gscouter's results, in phases: `docs/roadmap.md`.

## What it does not do

- Only the abilities these teams use, these weapons (Staff of Homa, Sacrificial Sword, Aqua Simulacra, Deathmatch, The Alley Flash, Harbinger of Dawn, the five Favonius weapons, The Stringless, Ballad of the Fjords, Wolf-Fang, The Alley Hunter, Sapwood Blade, Festering Desire, Slingshot, Dragon's Bane) and sets (Shimenawa, Emblem, Tenacity, Instructor, Noblesse Oblige, Golden Troupe, Scroll of the Hero of Cinder City, Marechaussee Hunter), and pyro and hydro resonance. Every other character, weapon, set, action, resonance, built-in or target option is an error that names it (`unsupported: <what> <name>`), never a silent difference.
- One target, at any position: no hitbox geometry. `target type=`, particle drops, `hurt`, random substats and `set_on_tick` are errors, as are the position built-ins and `pick_up_crystallize`.
- Of the reactions, vaporize, overload, electro-charged and crystallize (without shards): the ones pyro, hydro, electro and geo cause among themselves. No implemented character applies cryo, dendro or anemo. No Nightsoul, no HP debt. No fillers (Gscouter's `steady` package).
- Random numbers are its own (xoshiro256++), so single fights differ from gcsim's; only the averages compare.

## How it works

`engine/src/gcsl.rs` is gcsim's lexer and parser (`pkg/gcs/ast`, `pkg/gcs/parser`), `engine/src/eval.rs` its evaluator (`pkg/gcs/eval`), run as gcsim's `RunSync` runs it: each action is handed to the fight, which runs until it needs the next one. `engine/src/profile.rs` sets characters up as gcsim's `AddChar` does.

`engine/src/sim.rs` keeps gcsim's order of work within a frame (enemy queue and gauge decay, constructs, stamina, shields, animation, character queues, core queue, energy drops), its action state machine, hitlag (frozen characters and enemies, extended mods, queues and infusions), mods and their expiry rules, ICD tags and groups, per-character gauge slots, the reactions of `Reactable.React` for pyro, hydro, electro and geo with their events and reaction damage, heals and drains with their events and incoming healing bonus, weapon infusion and fight-wide statuses. It names no character, weapon or set: they attach through `engine/src/content.rs`.

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
go run ./gendata                                                                 # regenerate engine/src/data.rs and engine/src/gcsimdata
go run ./dump ../configs/team-looped.txt                                         # base stats as gcsim sets them up
```

`compare` exits with 1 when a comparison fails or stays inconclusive. CI (`.github/workflows/compare.yml`) runs it on every push with Gscouter at a fixed commit; gcsim is fixed by `reference/go.mod`. The report is the job summary and an artifact.

WebAssembly: `cargo build --release --target wasm32-wasip1` and `GOOS=wasip1 GOARCH=wasm go build`, run with `node:wasi` (not remeasured in Phase 1).

## License

GNU AGPL v3 or later (`LICENSE`). The engine is ported from gcsim, which is under the AGPL v3 since 2026-09-19, so it keeps gcsim's license and copyright notice.
