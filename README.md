# gscouter-engine

A Rust port of gcsim, written only for [Gscouter](https://github.com/cenoda/Gscouter): it scores Gscouter's teams within 1% of gcsim, several times faster. Bit-exact agreement is not a goal. It is ported from the source of Gscouter's fork of gcsim (`cenoda/gcsim@eb21884`, fixed by `reference/go.mod`), and runs Gscouter's looped conditions (450 s, scored from 150 s, Lv. 100 enemy at 10% resistance, energy every 8–12 s, swap delay 12).

## Status

> 2026-11-04 · release of engine `e6c8a07`, Gscouter at `e4d5796`

- **Content.** Every character the library's 4,570 teams use (168 member keys, each Traveler element once), each C0 to C6 with every action and param of its gcsim package; all 252 weapons (refinements 1 to 5) and 55 sets (two and four pieces) of gcsim's catalog; every resonance; every reaction, Lunar and Stellar ones included. The census counts all 4,570 library teams supported.
- **Verified.** The release of `e6c8a07` (CI run with `release=true`) verified 4,447 teams: each library build within 1% of gcsim for team DPS and every character with at least 10% of it, at 4,000 to 32,000 iterations, with 918 probes for the constellations, refinements and set counts no library build carries. The drift over its 5,365 builds is −0.01% (bound ±0.2%). Of the verified teams' library builds, 3,851 are within 0.1% of gcsim, 4,443 within 0.5%, all within 0.76%. Not verified: 121 idle teams (their rotations do nothing in the scored window in either engine; to fix in Gscouter's library), `amber-furina-bennett-rosaria` (unsupported at the release) and `mavuika-citlali-xilonen-bennett` (an error in both engines).
- **In Gscouter.** The owner approved the release's 4,436 group A teams and 11 of group B on 2026-11-04; they score on the engine on the site, in the browser and on the server (Gscouter `docs/engine.md`). A config the engine refuses runs on gcsim.
- **Speed.** 500 iterations of `configs/team-looped.txt` on one thread take about 1.3 s natively (2026-11-04, this session's 4-CPU machine), against about 10 s for gcsim natively (Phase 0). On Gscouter's pages that search for fillers, a result takes about half the time on the engine (Gscouter `docs/engine.md` §5).

What comes next is in `docs/roadmap.md`; how the engine is checked, `docs/verification.md`; the measurements and records of Phases 0 to 3, `docs/history.md`. All documents: `docs/README.md`.

It reads gcsim's config language as gcsim does: the same lexer, grammar and evaluator (`let`, `fn`, `if`/`else`, `while`, `for`, `switch` with `fallthrough`, `break`/`continue`/`return`, action lists with params and repeats, `.char.skill.ready` and the other fields, `wait`, `delay`, `execute_action` and the other built-ins), and the same setup lines (`options`, `target`, `energy`, `char`, `add weapon`, `add set`, `add stats`, `active`). A config gcsim runs, the engine runs unchanged, as long as it has what the config uses; anything else is an error naming it, never a silent difference.

## What it does not do

Everything below is an error that names it (`unsupported: <what>`, listed by `gse --check`), never a silent difference. Gscouter runs such a config on gcsim.

- **Fights**: one target; `target type=`, particle drops, `hurt`, random substats. `set_on_tick` only with the functions the engine can run.
- **Moving the player** (`set_player_pos`, or Nefer's held charged attack) with a character not checked against gcsim with a moved player (`MOVES_WITH_PLAYER`).
- **Reactions on the player** other than swirl (and a swirl of frozen or a Stellar Swirl there); a Stellar Swirl detonation during a jump.
- **Hexerei** with two Hexerei characters when one of them lacks its Hexerei bonuses.
- **Corners of content**, each named by its error: Diluc's C2 (being hit); Neuvillette or Chevreuse with the target off the y axis; the hydro Traveler with Neuvillette or Sigewinne; Key of Khaj-Nisut or Vermillion Hereafter 4pc on two characters; Gest of the Mighty Wolf on Noelle, Itto or Mavuika and Cloudforged's energy-loss stacks (raised at fight setup, not by `gse --check`); Dahlia's burst below ascension 4; Dori's charged attack; and a few states gcsim allows that the engine does not keep (Yaoyao's radishes beyond its limit, Lisa's C4 with more than 16 gadgets, an attack with no hit pattern that could reach a gadget).
- **Gscouter's fillers** (its `steady` package) are Gscouter's, around the engine, as around gcsim.
- **Random numbers** are its own (xoshiro256++), so single fights differ from gcsim's; only the averages compare.

What the probes cannot reach (kills, enemy HP and attacks, range, stamina, some Hexerei and Moonsign parts) runs but is unverified: `docs/roadmap.md` §3.

## How it works

`engine/src/gcsl.rs` is gcsim's lexer and parser (`pkg/gcs/ast`, `pkg/gcs/parser`), `engine/src/eval.rs` its evaluator (`pkg/gcs/eval`), run as gcsim's `RunSync` runs it: each action is handed to the fight, which runs until it needs the next one. `engine/src/profile.rs` sets characters up as gcsim's `AddChar` does.

`engine/src/sim.rs` keeps gcsim's order of work within a frame (enemy queue and gauge decay, constructs, stamina, shields, animation, character queues, core queue, energy drops), its action state machine, hitlag (frozen characters and enemies, extended mods, queues and infusions), mods and their expiry rules, ICD tags and groups, per-character gauge slots, the reactions of `Reactable.React` with their events and reaction damage (frozen's decay, poise and shatter, burning's ticks and its fuel's hold on dendro and quicken included), gadgets with their own auras and dendro cores, attack patterns for the gadgets an attack reaches, hits on the player, heals and drains with their events and incoming healing bonus, weapon infusion and fight-wide statuses. It names no character, weapon or set: they attach through `engine/src/content.rs`.

- Each character is a module in `engine/src/chars`, each weapon in `engine/src/weapons`, each set in `engine/src/sets`, implementing `Character`, `WeaponImpl` or `SetImpl`: hooks with default no-ops for its actions, its tasks, the callbacks of its attacks, the amounts of its mods, its attack mods, the events it subscribes to (enemy damage, skill, swap, state change, reactions), its frames and its condition fields. One line in `chars!`, `weapons!` or `sets!` registers it; the macros generate the enum of kinds, the enum of their states and the `match` that dispatches to them, so calls are static and the hot path has no dynamic dispatch or allocation.
- Mods have a key hashed at compile time that carries gcsim's name (which `.char.mods.<key>` reads), and either a fixed amount of one stat or an owner that computes it. Tasks are a character's, a weapon's or a set's, with an id and an argument. Events go to subscribers in gcsim's subscription order, so random numbers are drawn in the same order.
- Data comes from gcsim: `reference/gendata` copies the gcsim module into a temporary directory, adds to every character, weapon and artifact package a file registering its package variables and constants, and runs a dumper there, so values are what gcsim computes, frame tables built by `init` included. It writes `engine/src/data.rs` (names; base stats of every character and weapon; what each character's `NewChar` and `Init` set: element, weapon class, body, energy, normal hit count, talent constellations, animation start delays) and `engine/src/gcsimdata` (a module per package: talent multipliers, frames, hitmarks, hitlag, mod keys; and every ICD tag and group). It covers all 168 member keys of the library, 252 weapons and 55 sets. `engine/src/nightsoul.rs` and `engine/src/stellar.rs` hold Nightsoul and the Stellar reactions; `engine/src/check.rs` is `gse --check`.

Porting a character is then its logic only: Bennett is about 300 lines, Deathmatch and Instructor under 40 each. The fight core (`sim.rs`) is about 6,500 lines, the content interface (`content.rs`) about 700; the engine is about 84,000 lines without its generated data (2026-11-04). How to port, and what to watch for: `docs/porting.md`.

## Running

```
cd engine && cargo build --release && cargo test --release
./target/release/gse ../configs/team-looped.txt 10000 150 4   # config, iterations, warmup, threads [--seed N]
./target/release/gse --check ../configs/team-looped.txt        # what the config needs that the engine lacks

cd reference
go run . -config ../configs/team-looped.txt -iter 10000 -warmup 150 -j 4 -abil   # the same under gcsim
go run ./compare -gscouter ../../Gscouter -json compare.json -md compare.md      # the whole library (runs gcsim too: leave verdicts to CI)
go run ./compare -gscouter ../../Gscouter -teams hutao-yelan-bennett-xingqiu     # one team
go run ./compare -gscouter ../../Gscouter -only library,probes -shard 3/32        # one of CI's 32 parts
go run ./compare -gscouter ../../Gscouter -census                                # classify every team, run nothing
go run ./compare -gscouter ../../Gscouter -gcsim-only -teams <id,id>             # gcsim alone, into the cache
go run ./compare -gscouter ../../Gscouter -gcsim-only -builds <team/build> -seeds 0:256000   # gcsim alone on a range of a build's seeds
go run ./gendata                                                                 # regenerate engine/src/data.rs and engine/src/gcsimdata
go run ./dump ../configs/team-looped.txt                                         # base stats as gcsim sets them up
```

What `compare` decides and how, the probes' format, CI and releases: `docs/verification.md`.

For Gscouter, the engine is also a library (`engine/src/lib.rs`): `api::run_iterations(config, warmup, seed, first, count)` runs iterations `first` to `first + count - 1` (seed `seed + i`) and reduces each fight as Gscouter's `score.RunIterations` does (`score.Iteration`: team DPS over the window and over its two halves, at gcsim's 0.5 s buckets, DPS by character, waits in the window by reason, frames, names); `api::to_json` writes them as Gscouter's JSON. A config with anything the engine lacks is an error naming it. Gscouter's two guards hold for every fight, the command's too: a fight ends at 100,000 s (`MAX_FIGHT_FRAMES`, so a rotation that never ends or an enemy that never dies stops), and more than 10,000 actions (`wait`, `delay`, `execute_action`) on one frame is Gscouter's error for a rotation stuck there (`STUCK`). The `gse` command compiles the modules into itself rather than through the library, which cost 0.4% more instructions; keeping the buckets costs 0.14% (valgrind, 20 iterations of `team-looped.txt`: 470.4 million before, 471.1 million after), with byte-identical output.

`wasm/` is the engine as a WebAssembly module without imports (`cargo build --release --target wasm32-unknown-unknown` there; 4.0 MB at release `e6c8a07`, 1.2 MB when it had four characters), for Gscouter's browser workers and, through wazero, its server, so that both give the same results. It exports `gse_run_slice` (the JSON above) and `gse_check` (what a config lacks); `wasm/lib.rs` describes the calls, and `wasm/run.mjs` runs it in Node. `wasm/parity.sh` checks that it gives what the engine gives natively (`engine/examples/slice.rs`), byte for byte, and CI runs it with the engine's tests. 500 iterations of `team-looped.txt` took 2.9 s in Node 22 on a session's machine against 1.6 s natively (Phase 4, 2026-11-02).

## License

GNU AGPL v3 or later (`LICENSE`). The engine is ported from gcsim, which is under the AGPL v3 since 2026-09-19, so it keeps gcsim's license and copyright notice.
