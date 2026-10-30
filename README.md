# gscouter-engine

An experiment: would a simulation engine written only for Gscouter, in Rust, be faster than gcsim and still score within 1% of it?

It ports, from gcsim's source (Gscouter's fork, `cenoda/gcsim@eb21884`), what one library team needs: Hu Tao / Xingqiu / Zhongli / Yelan (`teams/hutao-xingqiu-zhongli-yelan`) with its builds, scored under Gscouter's looped conditions (450 s, window from 150 s, Lv. 100 enemy at 10% resistance, energy every 8–12 s, swap delay 12).

It reads gcsim's config language as gcsim does: the same lexer, grammar and evaluator (`let`, `fn`, `if`/`else`, `while`, `for`, `switch` with `fallthrough`, `break`/`continue`/`return`, action lists with params and repeats, `.char.skill.ready` and the other fields, `wait`, `delay`, `execute_action` and the other built-ins), and the same setup lines (`options`, `target`, `energy`, `char`, `add weapon`, `add set`, `add stats`, `active`). A config gcsim runs, the engine runs unchanged, as long as its characters, weapons, sets and actions are ones the engine implements; anything else is an error naming it, never a silent difference.

## Results

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

Speed, on the same 4-CPU machine:

| | gcsim | Rust | Ratio |
|---|---|---|---|
| Native, 1 thread, 500 iterations | 9.8–10.1 s | 1.18–1.32 s | about 8× |
| Native, 4 threads, 10,000 iterations | 78.6 s | 6.2 s | about 13× |
| WebAssembly in V8 (Node 22, WASI), 1 thread, 500 iterations | 52.0 s | 1.75 s | about 30× |
| WebAssembly build size | 46 MB | 0.45 MB | |

Evaluating gcsl costs about 7% over the hand-compiled rotation of the first version (1.10 s).

The WebAssembly row is the one that matters for the site, which scores in the visitor's browser. Go's WebAssembly runs gcsim about 5× slower than native Go; Rust's runs about 1.4× slower than native Rust. The gcsim side is the reference harness built with `GOOS=wasip1`; Gscouter's site uses `GOOS=js`, which has the same compiler backend.

## What it does not do

- Only the abilities, weapons (Staff of Homa, Sacrificial Sword, Aqua Simulacra) and sets (Shimenawa, Emblem, Tenacity) this team and its rotation use. No hitbox geometry: every attack is assumed to hit the one enemy, which holds for this config.
- One target, at any position: no hitbox geometry. `target type=`, particle drops, `hurt`, random substats and `set_on_tick` are errors, as are the position built-ins and `pick_up_crystallize`.
- No fillers (Gscouter's `steady` package), no shields beyond Zhongli's shred, no crystallize shards.
- Random numbers are its own (xoshiro256++), so single fights differ from gcsim's; only the averages compare.

## How it works

`engine/src/gcsl.rs` is gcsim's lexer and parser (`pkg/gcs/ast`, `pkg/gcs/parser`), `engine/src/eval.rs` its evaluator (`pkg/gcs/eval`), run as gcsim's `RunSync` runs it: each action is handed to the fight, which runs until it needs the next one. `engine/src/profile.rs` sets characters up as gcsim's `AddChar` does, from `engine/src/data.rs`, which `reference/gendata` writes from gcsim's catalog (the names gcsl accepts for characters, weapons and sets, and the curves and ascension stats of what the engine implements).

`engine/src/sim.rs` keeps gcsim's order of work within a frame (enemy queue and gauge decay, constructs, stamina, animation, character queues, core queue, energy drops), its action state machine, hitlag (frozen characters and enemies, extended mods and queues), mods and their expiry rules, ICD tags and groups, per-character gauge slots, vaporize and crystallize. Tasks are an enum instead of closures, attacks live in a reused slab, and stats are fixed arrays, so a fight allocates almost nothing.

Size: the fight core about 2,400 lines, the gcsl lexer and parser 1,800, the evaluator 500; Hu Tao 235, Xingqiu 174, Yelan 139, Zhongli 112 (hold skill only).

## Running

```
cd engine && cargo build --release
./target/release/gse ../configs/team-looped.txt 10000 150 4   # config, iterations, warmup, threads

cd reference && go build -o reference .
./reference -config ../configs/team-looped.txt -iter 10000 -warmup 150 -j 4 -abil
go run ./gendata > ../engine/src/data.rs          # names and base stat data from gcsim's catalog
go run ./dump ../configs/team-looped.txt          # base stats as gcsim sets them up, to check against
```

WebAssembly: `cargo build --release --target wasm32-wasip1` and `GOOS=wasip1 GOARCH=wasm go build`, run with `node:wasi`.

## License

GNU AGPL v3 or later (`LICENSE`). The engine is ported from gcsim, which is under the AGPL v3 since 2026-09-19, so it keeps gcsim's license and copyright notice.
