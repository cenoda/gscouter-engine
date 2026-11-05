# Porting from gcsim

> Status: In use · Last updated: 2026-11-05

How to port a character, weapon, set or system from gcsim, the gcsim behaviour the engine matches on purpose, the pitfalls found so far, how the content data is maintained (§4) and how to add a character from a content record (§5). Moved from `CLAUDE.md` on 2026-11-04. How a port is verified is `docs/verification.md`; how branches are run and merged is `CLAUDE.md`.

## 1. Order of work

For a new character, weapon or set:

1. Read its gcsim package, all of it, at the commit `reference/go.mod` pins (`cenoda/gcsim@eb21884`). Port from that source, not from memory of gcsim.
2. Take its tables from `engine/src/gamedata` and `engine/src/data.rs` (§4). Write its logic in `engine/src/chars/`, `engine/src/weapons/` or `engine/src/sets/`, on the hooks in `engine/src/content.rs`.
3. Port all of it, not only what the library's builds and rotations use: a character C0 to C6 (`MAX_CONS = 6`) with every action and param gcsim's package has, a weapon refinements 1 to 5, a set two and four pieces. What is left out stays an error that names it.
4. Run `compare` on every library team it makes supported, and add probes for what those builds do not carry: each constellation, refinements 1 and 5, two and four pieces (`docs/verification.md` §3).
5. It is done when all of that passes, not when its library teams do.

Before pushing, check locally what takes seconds: `cargo test`, the two reference scores to the last digit (`CLAUDE.md`), `gse --check` on the configs the change touches, and per-ability hits and damage on one dumped config against `go run . -config ... -abil` at 1,000 iterations. A screen for gross errors may use `compare -teams <id> -only library -cache ""`; its verdict is advisory.

Quick checks: dump the looped config with Gscouter's `go run ./cmd/gscouter -team teams/<id> -build teams/<id>/build.txt -dump`, then run `gse` and `go run . -config` at 1,000 iterations. To trace a difference in per-ability damage with equal hits, run one gcsim fight with its debug log (`simulation.NewCore(seed, true, cfg)`, then `c.Log.(*glog.Ctrl).Dump()`) beside the engine.

## 2. gcsim behaviour matched on purpose

Keep these matched:

- The order of work within a frame.
- Mod expiry: a mod counts in stat sums while its expiry is `> F`, and reads as active while `>= F`, unless gcsim has dropped it. The snapshot's `Stats()`, `ApplyAttackMods` (not for reaction damage), `ReactBonus` and `HealBonus` delete the mods of their kind whose expiry is `<= F`, so a mod at its expiry frame is inactive once one of them ran on that frame (`Char::pruned`; Raiden's eye is read so). `Stat`, `SelectStat`, `TotalAtk` and `TotalDef` drop nothing.
- Hitlag extends mods and queues.
- Gauges are kept per source character.
- In damage mode, a kill returns before the element attaches.
- A wait counts whole under the reason its action first failed for, as gcsim's stats do.
- gcsl evaluates both sides of `&&` and `||`, ignores `return` inside loops, and lexes `-` as an identifier character.
- The same seed gives the same result: keep the order of random draws. Events go to subscribers in gcsim's subscription order for that reason.

## 3. Pitfalls found so far

Check new code for each of these.

**Silent defaults**

- A param, field or constellation the engine does not handle must be an error, not a default. Found silently wrong: constellations above what a character implemented (now `MAX_CONS`), Yelan's `breakthrough`, `.char.infusion.<key>` (was always false).
- Lists of reactions, swirls, attack tags or elements must cover every one the engine has (VV's swirl bonus missed cryo swirl; Raiden's Eye did not ignore burning damage). Grep for the existing variants, and gcsim's ported packages for the new ones, when adding one.
- Content written before a system existed assumes it never happens: Sapwood Blade did nothing until dendro reactions, The Alley Flash ignored hits on the player. When adding a system, grep the modules' comments for "never", "none of which", "no fight", "does nothing".
- A character's private state must not live in a field other modules read. Clorinde's Night Watch counter kept in `normal_counter` made Fischl's C6 (which reads the active character's normal counter, as gcsim's does) skip ticks; gcsim keeps such counters apart (`Char::normal_s_counter`).

**Gadgets and geometry**

- Gadgets see an attack as it was before the enemy took it (gcsim hands each target its own copy, before ICD and reactions), and only through a hit pattern (`Pattern`). An attack without one that could reach a dendro core or Lynette's Bogglecat Box is an error; give patterns from gcsim's `combat.New...Hit` calls. A gadget that reacts to hits is added with `add_hittable_gadget` and handled in its owner's `gadget_hit`.
- Geometry sits on exact boundaries: a dendro core is exactly the enemy's radius + 0.5 from its center, so a circle of radius 0.5 on the enemy touches it at exactly the sum of the radii. Keep coordinates in f64 and compute them as gcsim does (in f32, Fischl's A4 reached such cores half the time against gcsim's two thirds).
- The engine works in gcsim's world coordinates: the player has a position and a facing, and a hit lands on the enemy when its shape reaches it, as gcsim's AttackWillLand decides. Build patterns with the helpers that mirror gcsim's calls (`circle_on_player`, `box_on_player`, `fan_on_player`, `circle_on_target`), at the moment gcsim builds them. Compare hits per ability.
- A character joins `MOVES_WITH_PLAYER` only once checked against gcsim with a moved player. Content can move the player in the fight, not only `set_player_pos`: Nefer's Slither (a held charged attack) steps toward the enemy each frame, so its teams need every member in `MOVES_WITH_PLAYER` (`gse --check` names the others; audits in `reference/audits/`).
- A new dendro-core-type gadget (Nefer's Seeds of Deceit, Nilou's Bountiful Cores) counts toward the five-core limit and is picked by abilities that pick cores, but is not a `dendrocore.Gadget` for Kaveh's rupture or `.gadgets.dendrocore.count`.
- Absorb checks read the enemy, then gadgets, then the player, and only what gcsim's check pattern reaches (`Sim::absorb_check_in`; Sucrose's burst does not reach the player or Guoba).

**gcsim's special cases**

- gcsim's ApplyAttackMods skips reaction damage (`AttackTag::is_reaction`); a crit-rate mod on a reaction makes it crit, and crits feed Favonius.
- gcsim has special cases away from the obvious code: burning's ICD is checked for every character, not only the attacker (`pkg/enemy/attack.go`).
- Stellar-Conduct and Polestar Fields are ported with Sandrone, and Radiance with them; Sandrone's library teams compare them with Odette (who also enables them). The cryo Traveler enables them too, but its library teams all carry Exaiphanes Blade, so only hand-written teams checked it (with Fischl and Sucrose it matched gcsim). Vodyanitsa's Wandering Vortex hooks into the Stellar Vortex (`stellar.rs`). Stellar Swirl is ported with Vesna, whose library teams compare it; Shenhe's C2 on it was checked by a hand-written team, and Escoffier's C1 cannot reach it (it needs an all hydro and cryo party, which cannot swirl). Qiqi reads Radiance but does not create it.

**Reading results**

- A library build checks only the constellations, weapons and refinements it carries (Lisa's are 19 of 21 at C6). Code no comparison reaches is unverified, whatever its source; say so in the report and add the probe that reaches it.
- A rotation can error in a few fights only, and each engine draws its own fights: `compare` calls it both-error when the engine reaches the same error. A rotation that does nothing in the scored window is idle: it passes, stays unverified, and is listed for fixing in Gscouter's library (`docs/verification.md` §2).
- Before trusting a quick check, look at per-ability hits and damage (`gse` output and the harness's `-abil`), and at waits.
- A team whose bursts wait for energy can spiral (a wait takes the field time that makes the next burst's energy), so its DPS is heavy-tailed and 1,000 iterations can be 2% off (Klee / Fischl / Xingqiu / Bennett looked −1.78% and passes at 36,000). Let `compare` decide before hunting a cause.

**Code layout**

- The engine's modules are listed twice, in `engine/src/main.rs` and `engine/src/lib.rs` (the library Gscouter links): a new top-level module goes in both.
- Keep the hot path free of allocation and dynamic dispatch: characters, weapons and sets dispatch through the macro-generated `match`es of `chars!`, `weapons!` and `sets!` (`README.md`, "How it works").

## 4. Content data

`engine/src/data.rs` (names and keys; base stats, curves and ascension stats; what each character's `NewChar` and `Init` set) and `engine/src/gamedata/` (a module per gcsim package: talent multipliers, frames, hitmarks, hitlag, mod keys; the ICD tags and groups; the shared combat and crystallize tables) are maintained in this repository and edited by hand. No gcsim source and no generator is needed to change them.

**Provenance.** Every value there is `legacy_gcsim`: generated from gcsim `eb21884` by `reference/gendata` and kept under the owner's legacy-data exception of 2026-11-04 (`docs/independence.md` §4). The files' headers say so. A value replaced by anything else records its new provenance as a row of `RECORDS` in `engine/src/provenance.rs`, beside the edit, in the same commit:

- `kind` and `key`: the content key (`data::CHAR_KEYS`, `WEAPON_KEYS`, `SET_KEYS`, or `SHARED_KEYS` for `combat`, `crystallize`, `icd`, `curves`).
- `field`: the value, as a path below `engine/src` with an index or field where the item holds several, e.g. `gamedata::chars::xiangling::SKILL[0]` or `data::CHARS.base`.
- `evidence` (`official`, `derived`, `measured`, `estimated`), the game `version`, the `source` (a URL, a video with its timestamp, or a derived value's formula and inputs) and the `observed` date; `note` for anything else (an estimate's assumptions).

A key or field with no row is `legacy_gcsim`. Nothing in a fight reads the table, so the hot structs carry no provenance. `cargo test` checks that every content key has a provenance and that each row names a known key, a path, its source, version and date, once. `gse --catalog` reports the rows by key.

**Names and keys.** `data.rs` holds the one key table: `CHAR_NAMES`, `WEAPON_NAMES` and `SET_NAMES` map every name gcsl accepts (keys and aliases, lowercase) to an id, and `CHAR_KEYS`, `WEAPON_KEYS` and `SET_KEYS` map the id to its key. They were gcsim's `pkg/shortcut` tables at `eb21884` and still equal them, entry for entry (checked 2026-11-05, 781 names). A new name or alias is added there only, keeping the tables sorted; `cargo test` checks that keys are unique, that each key is a name of itself and that every name resolves to a listed key. Gscouter takes its keys from the export of this table (`gse --catalog`, below), not from gcsim.

**The catalog export.** `gse --catalog` (also `api::catalog` and the Wasm module's `gse_catalog`) prints the key table as versioned JSON (`engine/src/catalog.rs`): `catalog_version`, `provenance_default`, then `characters`, `weapons` and `sets`, each sorted by key with its `aliases`, whether the engine `implemented` it, and its data (a character's `rarity`, `element`, `weapon_class` and `burst_energy`; a weapon's `rarity` and `weapon_class`) and `provenance`. Removing or renaming a field raises `catalog_version`; adding one does not. On 2026-11-05 the export agreed with gcsim's `pkg/catalog` on every character's rarity, element, weapon class and burst energy cost and every weapon's rarity and class (Aloy's special rarity is 5 here).

**The parse API.** `gse --parse` (also `api::parse` and `gse_parse`) gives what a config's setup lines say, with the catalog's keys, as versioned JSON (`parse_version`); the contract and its check against gcsim's parser are `docs/parse-api.md`. A change to gcsl's setup lines keeps `reference/parsecheck` agreeing.

**New content** comes through the content records and converter of §5 below (`docs/content-format.md`), with the sources of `docs/independence.md` §4; its output names its generator, and its values carry their provenance.

`reference/gendata` is no longer part of this workflow. It is kept to reproduce the legacy data from gcsim `eb21884`; running it overwrites `data.rs` and `gamedata/` with gcsim's values and the old headers, so run it only into a copy (`go run ./gendata -engine <copy of engine>`) to diff against.

## 5. Adding a character from a content record

How new content is added without gcsim (Phase 6, `docs/independence.md` §4–5). The record format, its checks and its selections are `docs/content-format.md`. Until step 3's transition, a character added this way has no gcsim to be compared with, so it is not released to Gscouter under the current rules; the steps below make its data, not its approval.

1. **Its name.** Add the key and its aliases to the engine's key table first (`data.rs` `CHAR_NAMES` and `CHAR_KEYS`, §4). The validator rejects a record whose key is not there.
2. **Layer 1 from official pages.** Read the character's HoYoWiki page as the step 0 pilot did (`docs/pilot-xiangling/official.md` §2: render it, one load, an identifying User-Agent, within `docs/independence.md` §4's rules). Write each talent row as an `official_rounded` observation with `shown` exactly as the page prints it, the energy cost, the cooldowns and durations, the constellations that raise talents, and the numbers in the passives and constellations. Record the base stats at the 14 breakpoints as `check = true` values. Levels the page does not show are `derived` (formula, inputs) or `estimated`.
3. **Layer 2 from public evidence.** For each mechanic of the contribution checklist (normal and charged attacks, skill, burst, A1, A4, C1–C6; hitlag, ICD, strike type, poise, hitboxes, durability, particles, frames), give a `measured` or `stated` observation with its source, or the standard `estimated` value (`docs/independence.md` §4, Layer 2) with its `basis`, or `unresolved` with why. Frames are counted from video at 60 fps (gcsim's `frames.md` procedure); a video source gives the timestamp. For a consequential estimate (ICD of persistent attacks, particles, geometry), run the plausible alternatives on a representative team and write each as a `[[value.sensitivity]]`; over 5% marks the result as highly uncertain (`docs/independence.md` §7.2).
4. **Targets.** Declare the tables module's items in `[items]` (with `len`) and give each value its `target`; values the character's logic will read go in the tables module, not in the logic.
5. **Check and convert.** `gsc check`, `gsc status` (it should read `provisional` or `high_uncertainty` until layer 2 is measured), then `gsc convert <record> --select independent --out engine/src/gamedata/chars/`. Add the module to `chars/mod.rs` and the printed row to `data.rs` `CHARS`, in key order. Do not edit generated output by hand: change the record and convert again.
6. **The logic** goes in `engine/src/chars/`, on the hooks of `engine/src/content.rs`, reading only the generated tables. What it does not implement is an error that names it (`gse --check`).
7. **Tests** follow the evidence (`docs/independence.md` §5 step 3): public damage numbers reproduced with the same build within 1% per hit, measured frames and ICD, and per-ability hits and damage.

An existing character keeps `select = "legacy"`: its record reproduces its engine data exactly, and an official or measured value replaces a legacy one only as a step 6 change (`content/tests/xiangling.rs` shows both on Xiangling). `gsc import <key>` writes a draft record from a character's legacy data to start from; its units are guesses to review, and what it cannot classify is `unclassified`.
