# Parse API

> Status: `parse_version` 1, added 2026-11-05 for Phase 6 step 2 (`docs/independence.md` §5); not yet wired into Gscouter · Last updated: 2026-11-05

What a config's setup lines say, as versioned JSON, so that Gscouter's input path (`internal/config/config.go`, which reads the party of an assembled config, and `internal/pull/preset.go`, which reads every build field of a library `build.txt`) can stop calling gcsim's `pkg/gcs/parser` and `ast`. `internal/score` keeps gcsim's parser for the gcsim simulation path until step 5.

## 1. Entry points

- `gse --parse <config>`: prints the JSON; exits with 1 when the config does not parse (the JSON is then the error).
- `api::parse(src)`: `Ok(json)` or `Err(json)`.
- The Wasm module's `gse_parse(cfg, len)`: status 0 with the JSON, or status 1 with the error's JSON (a plain message only when the config is not UTF-8). `wasm/lib.rs` describes the calls.

The code is `engine/src/parse.rs`, over the engine's gcsl parser (`gcsl::Parser::parse_setup`). Parsing checks nothing about simulation support: that stays `gse --check`. A config parses even when the engine cannot run it.

## 2. The result

```text
{"parse_version": 1, "ok": true,
 "characters": [{"key", "level", "max_level", "ascension", "cons",
                 "talents": [attack, skill, burst], "params": {name: int},
                 "weapon": {"key", "level", "max_level", "refine", "params"} | null,
                 "sets": [{"key", "count", "params"}],
                 "stats": {stat: number}}],
 "active": key | null,
 "targets": [{"level", "resist": {element: number}, "hp", "pos": [x, y], "radius"}],
 "energy": {"mode": "once" | "every", "start", "end", "amount"} | null,
 "options": {"iterations", "duration", "swap_delay", "hitlag", "defhalt",
             "ignore_burst_energy", "damage_mode"},
 "hurt": bool,
 "errors": [{"class", "message"}]}
```

- **Keys** are the catalog's (`gse --catalog`, `docs/porting.md` §4): an alias (`ht`, `raiden`, `homa`, `cw`) is resolved to its key. Gscouter's Traveler mapping (`travelercryo` to `aethercryo`) stays its own.
- **Order**: characters in the order of their first line.
- **Repeated lines** change the same character, as in gcsim: a second `char` line (under any alias) replaces level, constellation and talents; `add stats` lines add up, stat by stat; a second `add set` of the same set replaces its count; a second `add weapon` replaces the weapon. A duplicate character is not an error in gcsim, so it is not one here.
- `talents` are the levels written, without constellation bonuses. `ascension` is gcsim's `AvatarAsc` of the max level (null for a character without base data). `weapon` is null without an `add weapon` line.
- `sets` are sorted by key. `stats` are the sums by gcsim's stat names (`hp`, `hp%`, `atk`, `atk%`, `def`, `def%`, `er`, `em`, `cr`, `cd`, `heal`, `pyro%` ... `phys%`, `atkspd%`, `dmg%`), in gcsim's stat order, zeros left out, as `preset.go` reads them. Constant expressions (`cr=0.1+0.2*2`) are folded; a stat that is not constant (`hp=x`) is an error, as in gcsim.
- Params (`+params=[...]` on a character, weapon or set) are given although Gscouter's presets drop them.
- `resist` lists elements with a nonzero resistance, by gcsim's element names. A target's radius defaults to 1, as gcsim's does.
- `errors` are the checks of a whole config that gcsim's `Parse` reports in its `Errors` without failing, in its order and with its messages: `too_many_characters`, `no_active_character`, `too_many_set_items`, `active_not_in_team`, `no_targets`, `target_hp_missing`. A `build.txt` alone (no `active`, no `target`) parses with `no_active_character` and `no_targets`, as `preset.go` relies on; `config.Characters` treats any of them as an error.

A config that does not parse:

```text
{"parse_version": 1, "ok": false, "error": {"class", "line": n | null, "message"}}
```

The first error only. Classes: `unknown_character` (a setup line of a name that is not a character: gcsim fails on it too, as an expression), `unknown_weapon`, `unknown_set`, `unsupported` (a form gcsim reads and the engine does not, by name: `add stats random`), `syntax` (everything else). `line` is 1-based, null where the parser does not know it (end of input, a non-constant stat). Messages follow gcsim's where the engine's parser has them; their wording is not part of the contract, the class is.

**Versioning.** Removing or renaming a field, or changing a value's meaning, raises `parse_version`; adding a field or an error class does not.

## 3. Agreement with gcsim

`reference/parsecheck` parses each config with gcsim's parser at the pinned `eb21884` and with `gse --parse`, and compares every field above (stats and resistances exactly, as float64). Fixtures in `reference/parsecheck/malformed/` give the expected class on their first line (`# expect: unknown_weapon`, or `# expect: ok no_active_character,no_targets`); a config gcsim refuses must be refused by the engine, and the reverse.

```sh
cd reference
go run ./compare -gscouter <Gscouter> -only library,probes -dry-run -configs <dir> -cache ""
go run ./parsecheck -gse ../engine/target/release/gse parsecheck/malformed ../configs <dir>
go run ./parsecheck -gse ../engine/target/release/gse -name build.txt <Gscouter>/teams
```

CI runs the same in the library job's part 0. On 2026-11-05 (Gscouter `2df20b3`): all 4,570 library configs and 918 probe configs as `compare` assembles them, the 4 reference configs and 18 fixtures (5,510) agree, and so do all 4,817 `build.txt` files of Gscouter's teams parsed alone. One declared difference: `add stats random` (random substats), which gcsim reads and the engine refuses as `unsupported`; no library build uses it.

`wasm/parity.sh` checks that `gse_parse` gives `gse --parse`'s bytes on every reference config and fixture.
