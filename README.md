# gscouter-engine

An experiment: would a simulation engine written only for Gscouter, in Rust, be faster than gcsim and still score within 1% of it?

It ports, from gcsim's source (Gscouter's fork, `cenoda/gcsim@eb21884`), what one library team needs: Hu Tao / Xingqiu / Zhongli / Yelan (`teams/hutao-xingqiu-zhongli-yelan`) with its builds, scored under Gscouter's looped conditions (450 s, window from 150 s, Lv. 100 enemy at 10% resistance, energy every 8–12 s, swap delay 12).

## Results

Accuracy at 10,000 iterations (`configs/team-looped.txt`, the config Gscouter assembles; seeds differ, since the engine has its own random numbers):

| | gcsim | Rust | Difference |
|---|---|---|---|
| Team | 72,263.7 ± 24.8 | 72,265.8 ± 24.9 | +0.00% |
| Hu Tao | 45,197.3 ± 12.5 | 45,193.8 ± 12.6 | −0.01% |
| Xingqiu | 9,790.9 ± 6.7 | 9,797.8 ± 6.7 | +0.07% |
| Zhongli | 1,472.1 ± 0.5 | 1,473.6 ± 0.5 | +0.10% |
| Yelan | 15,803.3 ± 10.4 | 15,800.6 ± 10.4 | −0.02% |

Hits per ability agree as well (e.g. per fight in the window: Hu Tao's charged attacks 132.7 / 132.6, Xingqiu's rain swords 518.2 / 519.5, Yelan's Exquisite Throws 438.2 / 440.1). Hu Tao alone (`configs/hutao-solo-looped.txt`, 1,000 iterations): gcsim 24,599.8, Rust 24,607.0 (+0.03%).

Speed, on the same 4-CPU machine:

| | gcsim | Rust | Ratio |
|---|---|---|---|
| Native, 1 thread, 500 iterations | 9.8–10.1 s | 1.10–1.12 s | about 9× |
| Native, 4 threads, 10,000 iterations | 78.6 s | 5.6 s | about 14× |
| WebAssembly in V8 (Node 22, WASI), 1 thread, 500 iterations | 52.0 s | 1.53–1.56 s | about 34× |
| WebAssembly build size | 46 MB | 0.22 MB | |

The WebAssembly row is the one that matters for the site, which scores in the visitor's browser. Go's WebAssembly runs gcsim about 5× slower than native Go; Rust's runs about 1.4× slower than native Rust. The gcsim side is the reference harness built with `GOOS=wasip1`; Gscouter's site uses `GOOS=js`, which has the same compiler backend.

## What it does not do

- Only the abilities, weapons (Staff of Homa, Sacrificial Sword, Aqua Simulacra) and sets (Shimenawa, Emblem, Tenacity) this team and its rotation use. No hitbox geometry: every attack is assumed to hit the one enemy, which holds for this config.
- Rotations are compiled by hand into a small instruction list (`engine/src/configs.rs`); there is no gcsl parser.
- No fillers (Gscouter's `steady` package), no one-pass check, no hurt, no shields beyond Zhongli's shred, no crystallize shards.
- Random numbers are its own (xoshiro256++), so single fights differ from gcsim's; only the averages compare.

## How it works

`engine/src/sim.rs` keeps gcsim's order of work within a frame (enemy queue and gauge decay, constructs, stamina, animation, character queues, core queue, energy drops), its action state machine, hitlag (frozen characters and enemies, extended mods and queues), mods and their expiry rules, ICD tags and groups, per-character gauge slots, vaporize and crystallize. Tasks are an enum instead of closures, attacks live in a reused slab, and stats are fixed arrays, so a fight allocates almost nothing.

Size: the core is about 2,000 lines; Hu Tao 235, Xingqiu 174, Yelan 139, Zhongli 112 (hold skill only).

## Running

```
cd engine && cargo build --release
./target/release/gse team-looped 10000 150 4      # config, iterations, warmup, threads

cd reference && go build -o reference .
./reference -config ../configs/team-looped.txt -iter 10000 -warmup 150 -j 4 -abil
go run ./dump ../configs/team-looped.txt          # base stats the engine takes as data
```

WebAssembly: `cargo build --release --target wasm32-wasip1` and `GOOS=wasip1 GOARCH=wasm go build`, run with `node:wasi`.
