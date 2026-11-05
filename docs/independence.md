# Independence from gcsim

> Status: Direction agreed with the owner; reviewed plan and execution roadmap updated on 2026-11-05, implementation not started. Open policy decisions remain in §7. Nothing here is done; the rules in `CLAUDE.md` and `docs/verification.md` still hold until each step below changes them · Last updated: 2026-11-05

How Gscouter and this engine stop depending on gcsim, in what order, and what stays. This is Phase 6 of `docs/roadmap.md`. The parts in Gscouter are listed here so the plan is in one place; they are made on Gscouter's `claude/...` branches under its own rules.

The execution order, deliverables and completion gates are in [the independence roadmap](independence-roadmap.md). Updating this plan does not approve an unresolved data policy or activate the future verification rules.

## 1. Why

- **New content costs twice.** Each new character is implemented in Gscouter's gcsim fork, then ported here. For the characters the fork implemented itself (Lohen, Sandrone, Vesna, Vodyanitsa), the research was ours already; the port is the same work done again.
- **gcsim updates slowly**, and Gscouter calls code that keeps changing and is not ours.
- **gcsim's Go simulator is the bottleneck** of the site (Gscouter `docs/engine.md` §5, `docs/speed.md`).
- **The accuracy bar stops accuracy.** "Within 1% of gcsim" was the scaffold that brought the engine to gcsim's level. While it stands, the engine cannot fix gcsim's errors: a fix fails the bar. Fixing such errors, such as Arlecchino's damage, which the site says is about 20% too low (`issue.arlecchino`), is why the engine was built.

## 2. What gcsim does for us today

gcsim is not only a simulator. For Gscouter it is five things:

| gcsim's part | Where it is used |
|---|---|
| The simulator (Go) | Scores of teams not on the engine, the fallback for configs the engine refuses, timelines, character details, charts and the chart editor, the browser build (`cmd/wasm`) |
| The gcsl language and its parser | Assembling configs and reading their party (`internal/config`, `library`, `pull`, `score`) |
| The game data pipeline | Multipliers, base stats, curves, ICD tags and groups; the engine takes them through `reference/gendata` |
| Frame data measured by its community | Frames, hitmarks, hitlag |
| Key tables and mappings | Character and weapon names to keys (`pkg/shortcut`), Enka IDs and the artifact main stat table (`internal/enka/gen.sh` copies them from gcsim's UI data) |

The path of a result page, and where gcsim is on it:

```
 player UID / GOOD file
   ① showcase to build      ◀── gcsim data: Enka ID ↔ key, artifact main stats
   ② find library teams     ◀── gcsim code: name → key (shortcut.CharNameToKey)
   ③ assemble config, party ◀── gcsim code: gcsl parser (rejects unknown names)
   ④ score simulation       ── approved team → engine ─(refused/failed)→ gcsim
                               other teams  → gcsim
   ⑤ result page            ── score, filler search → as ④
                               timeline, details, charts, chart editor → gcsim always
```

Off the site: the library comes from the community gcsim database (`cmd/harvest`, `cmd/import`) and is checked against its sources on gcsim (`cmd/validate`); the engine's data comes from gcsim's source (`reference/gendata`); the engine is verified against gcsim (`reference/compare`, `.github/workflows/compare.yml`).

## 3. What stays

These are not dependencies to remove:

- **gcsl as a format.** Community rotations are written in it. Its lexer, parser and evaluator are already the engine's own (`engine/src/gcsl.rs`, `engine/src/eval.rs`). Grammar added beyond gcsim's makes compatibility one way (the engine reads gcsim configs, not always the reverse); decide that when it comes up.
- **The community gcsim database as a source of rotations.** It is data, not code.
- **Source validation (`cmd/validate`) on gcsim.** Its sources' DPS are gcsim numbers, so gcsim is the right check. It stays an offline tool, not part of the site.
- **A pinned gcsim (`eb21884`, `reference/go.mod`) as a regression aid for existing content**, as an optional diagnostic even after the checks of §5 step 3 replace its role as the accuracy gate. Removing the comparison tool is not a condition of independence. It does not follow gcsim's updates.

## 4. Where data comes from

### Principle

- **No datamining** (owner, 2026-11-04). Nothing is extracted from the game client, and no pre-release, beta or leaked data is used. The terms of service forbid reverse engineering, and HoYoverse has acted on it (the 2021 notice to the Honey Hunter wiki). Databases built from extracted data (Honey Hunter, genshin-db, AnimeGameData, gcsim's newly generated data) are not used either: not extracting it ourselves does not change where it comes from.
- So a new character is implemented **after its release**. Being earlier than gcsim comes from our speed, not from early data.
- **No crawler.** HoYoLAB's rules on automated access were not found. Official pages are read the way a person reads them: one page per new character, by the session that implements it.
- Each value records its source (an official page, a measurement with its video, a community test, or an estimate). Estimates are marked as such and replaced when measurements appear.

### Source feasibility and provenance

Before committing to this pipeline, demonstrate it on one released character: identify the actual official tables available, their level coverage and any missing values. The table below describes intended sources, not a completed availability audit. A shared growth ratio or curve is a derived value, not an official observation; record its formula, inputs and validation, and do not assume it applies to every talent.

Each value records its content key, unit, game version, source URL or video timestamp, observation date and evidence kind: official, derived, measured or estimated. Preserve the inputs for derived values and the assumptions for estimates. Source changes need a reviewable diff.

### Layer 1: numbers the game shows

From HoYoverse's official HoYoWiki (wiki.hoyolab.com):

| Value | Source |
|---|---|
| Talent multipliers, levels 1 to 11 or more | The talent tables on the character's HoYoWiki page |
| Multipliers up to level 15 | Talent multipliers grow by fixed shared ratios from level 1 |
| Base stats and ascension bonuses | HoYoWiki's tables; the growth curves are shared with existing characters and already in the engine |
| Conditions, durations and amounts of passives and constellations | The official descriptions |
| Damage formula | HoYoWiki's damage formula page |
| **A check** | Enka showcases, which Gscouter already reads, carry each character's actual base HP, ATK and DEF: a new character's showcases check our stats automatically |

### Layer 2: values the game does not show

ICD, hitlag, element durability, particles, hitboxes, frames.

1. Start from the game's standard values (most attacks: the standard ICD of 2.5 s or 3 hits; 1U or 2U durability; hitlag of the weapon class), marked as estimates.
2. Replace them with public measurements as they appear: community tests (KQM TCL and the like) and frame counts with their videos. These measure the game; they are not datamining.
3. Calibrate against public damage numbers (videos, HoYoLAB posts), as Lohen was (Gscouter `docs/characters/lohen.md`).

Until then, the results of such a character say that they include estimates. For consequential unknowns such as ICD and particle generation, test plausible alternatives on representative rotations and record whether team rankings, build recommendations or energy waits change. This is model uncertainty, separate from random simulation error; do not label it a statistical confidence interval. A material recommendation change keeps the affected result provisional until the evidence or scope resolves it.

### Learned from gcsim

gcsim implements a character with no other simulator to compare against. What to take from it:

- **Its frame measurement procedure** (gcsim `ui/packages/docs/docs/mechanics/frames.md`, `hitlag.md`, `aoe.md`): record the game at 60 fps, count frame by frame, keep the video as the evidence.
- **Its contribution checklist** (gcsim `CONTRIBUTING.md`): normal attack, charged attack, skill, burst, A1, A4, C1 to C6, hitlag, ICD, strike type, poise, hitboxes, durability, particles, frames. Ours goes in `docs/porting.md`, rewritten as how to implement content.
- **Evidence per value**, as Gscouter's character documents already keep.

What not to take: its ICD, hitlag, hitboxes, durability and particles come from the game's extracted ability data, as Lohen's did on the fork. Under the principle above they come from measurements or standard estimates instead.

### Existing data

`engine/src/data.rs` and `engine/src/gcsimdata/` (129 characters, 252 weapons, 55 sets), as well as Gscouter's existing `artifact_main_gen.json`, were generated from gcsim, whose pipeline reads extracted data. Decided by the owner (2026-11-04): they are kept as an explicit, documented legacy exception rather than replaced wholesale. Renaming files or moving them to repository-owned maintenance does not change their provenance, so legacy records retain an explicit `legacy_gcsim` provenance marker until an individual value is replaced by an official observation, measurement or step 6 correction. New content and post-independence corrections follow the no-datamining rules above.

### Enka data

Two kinds of Enka data are used, and they are treated differently (owner, 2026-11-05):

- **The showcase API** (a player's build from their UID) returns what the game's own Character Showcase publishes. It is official data the player chose to show, not extracted data, and it stays the site's input.
- **Enka's store data** (`EnkaNetwork/API-docs`, `store/gi/`: ID-to-name mappings, in-game names; read by Gscouter's `internal/enka/gennames`) and gcsim's UI copies of Enka mappings (`character.dm.json`, `weapon.dm.json`, `artifact.dm.json`) are built from the game's files. The existing mappings are kept under the same legacy exception as above, with their provenance recorded. New mappings are not taken from them: for a new character or weapon, the session that adds it reads one showcase that has it and records the observed `avatarId` (or weapon `itemId`) against the key, with the showcase as the source. This is an observation of the official API, not extracted data.

## 5. Steps

Dependency first (owner, 2026-11-04): new characters wait until this is far enough along; they are not implemented on the fork meanwhile. A character neither engine has simply has no library teams, so waiting costs coverage, not wrong scores. Each step keeps the two reference scores of `CLAUDE.md` to the last digit unless it says otherwise.

| Step | Work | Size | When done |
|---|---|---|---|
| 0 | Source pilot | small pilot | Legacy exception is recorded (done, 2026-11-04) and one released character demonstrates the source workflow |
| 1 | Engine data and keys | medium | Content data can be added, validated and consumed without gcsim source or generation tools |
| 2 | Gscouter ①②③ | medium | The site's front no longer calls gcsim code |
| 3 | Verification pilot, then approval transition | medium; owner's decisions | Independent checks and regression run in CI; the evidence gate and release policy are approved |
| 4 | Result page ⑤ | large | gcsim is left only as the fallback |
| 5 | Fallback, then removal | medium | gcsim is gone from the site |
| 6 | gcsim behaviour matched on purpose | per item | Accuracy starts going past gcsim's |

Step 0 precedes step 1. An Arlecchino investigation demonstrates the proposed verification workflow before step 3 changes the active gate; it does not authorize a score-changing release under the current rules. New characters come in after step 3, on the normal path, engine only. Before step 4 they lack timelines and charts, and their failures cannot fall back to gcsim. Step 6 can start on existing teams as soon as step 3 is done; Arlecchino first.

### Step 0: policy and source pilot

The existing-data policy is decided (2026-11-04: documented legacy exception, §4; Enka data, 2026-11-05). Pilot the proposed official/measurement workflow on one released character before step 1 finalizes the input format. List unavailable fields and how each is handled; do not call the source pipeline complete when those fields are unaccounted for.

- **The pilot character is one already in the legacy data**, so each value read from official pages can be diffed against its `legacy_gcsim` value. Agreement checks the workflow; a difference is explained (rounding, derivation, a game change) before the workflow is trusted. A new character would need implementing, which waits until after step 3.
- **Record how the official pages are read.** HoYoWiki renders its pages in the browser; if a session can read them only through the site's own API, record that, and whether it stays within "one page per character, read as a person reads it" (§4), before the workflow is adopted.
- **Include the Enka mapping** of the pilot character as observed from a showcase (§4, Enka data), and check it against the legacy mapping.

### Step 1: engine data and keys

- Under the legacy-data exception (§4), move `engine/src/data.rs` and `engine/src/gcsimdata/` to repository-owned maintenance, preserving explicit legacy provenance metadata, and rename the module. Separate editable source records from converter output: remove the old generated warning only from manually maintained files; generated output still names its generator.
- Retire `reference/gendata` from the maintained data workflow once its replacement is demonstrated. The Rust engine already has no Cargo dependencies; building it alone is not proof that data maintenance is independent.
- One key table, in `data.rs` (names, aliases, keys), from which Gscouter takes its keys too. New names are added there only.
- An input format for new content (the values of §4 with their sources) and a converter to the engine's data.

### Step 2: Gscouter ①②③

| Where | What changes |
|---|---|
| `internal/enka/gen.sh`, `internal/enka/gennames` | The existing Enka mappings, names and `artifact_main_gen.json` stop being copied from gcsim's module and Enka's store; they are kept in Gscouter as legacy data with their provenance (§4, Enka data). New IDs come from the key table of step 1, recorded from an observed showcase. Not for speed: so a new character's showcase can be read without a gcsim update |
| `shortcut` in `internal/config/weapon.go` (with `core/keys`) and `internal/library/library.go` | The engine's key table |
| gcsim's `parser` and `ast` in `internal/config/config.go` and `internal/pull/preset.go` | A versioned engine parse API through wazero: ordered party plus levels, ascension, constellations, talents, weapons, refinements, sets and stats needed by preset consumers; preserve alias, expression and error semantics |

Also migrate `internal/config/bosses.go`, which initializes boss resistances through gcsim's `enemy`, `info` and `attributes`, and the `attributes` uses in `pull/preset.go`. Give replacement boss data the same provenance policy. A party-only API does not replace `internal/pull/preset.go`. Verify representative presets and malformed inputs against existing behavior, and keep parsing separate from simulation support checks.

Scope: step 2 removes gcsim from the input path ①②③ only. `internal/score` (`score.go`, `requests.go`) parses and evaluates configs with gcsim's `parser` and `eval` to run gcsim itself; that use goes with the gcsim simulation path in steps 4 and 5, not here. The inventory of imports (2026-11-05, at Gscouter `2df20b3`, non-test files): `config` (`weapon.go`, `config.go`, `bosses.go`), `library`, `pull/preset.go` for step 2; `chart` (`start.go`: `catalog`, `model`, `shortcut`; `preview.go`), `timeline` (`stats`), `score/chasca.go` for step 4; `score.go`, `requests.go` for step 5. `internal/web`'s message files only link to gcsim's repository.

### Step 3: verification and approval

- **The engine's own regression**: fixed reference scores over the library, so a change that should not move scores keeps them, as the two reference configs do now.
- **Game checks**: public damage numbers, Enka stats and measured frames and ICD, kept as tests with their sources. Regression detects change, not truth. Preserve all 918 existing probes and their coverage gaps when replacing their runner. For new content, map actions, passives and C0–C6 behavior to evidence and tests, including buff conditions, per-ability damage/hit counts, reactions/ICD and energy generation/consumption; explicitly mark anything still estimated or unverified. Define tolerances from the evidence and rounding, not from agreement with gcsim.
- **The pinned gcsim becomes an aid**: after the transition, a difference from it is recorded and explained, not an automatic failure. Retain the comparison runner, useful diagnostics and optional cached comparisons for existing content. Record intentional differences with affected mechanics, evidence, tests and release version. Arlecchino is the first pilot: reproduce the reported discrepancy under controlled conditions, isolate its cause through attack/event details, and demonstrate the correction and affected-team report. The reported −20% is a hypothesis to investigate, not a target adjustment; an inconclusive result remains inconclusive.
- **Approval revised** (owner, 2026-11-04: "of course"): `docs/phase4-approval.md` defines group A against gcsim; it is redefined on the checks above, release by release, and the engine becomes the default rather than approved team by team.
- **Then remove the rule** "within 1% of gcsim" from `CLAUDE.md` and `docs/verification.md` (owner, 2026-11-04: to be removed, not yet), only once the replacement CI and release gate are operational and approved. Decouple release manifests and approval from gcsim verdicts; retain module hashes, support reporting and reproducibility. Retire only obsolete coupling in `harness`, `dump`, `manifest` and CI, not useful comparison infrastructure. Version regression baselines; review intended changes with their evidence and affected-team reports rather than overwriting expected scores to make CI pass.

### Step 4: result page ⑤

- The engine writes the events and stats that timelines, character details and charts need. Define a versioned event/stat contract, with units, frame timestamps and warmup semantics. Detailed traces are opt-in so normal score runs do not pay their allocation or serialization cost; check that tracing preserves scores and random draws.
- `internal/timeline`, `internal/chart` (`preview.go`; `start.go`, which also uses gcsim's `catalog`, `model` and `shortcut`) and the character details move to it; so does `internal/score/chasca.go`, which reaches into gcsim's internals.

### Step 5: fallback, then removal

- Port what the engine refuses (`README.md`, "What it does not do"; `gse --check`) in the order Gscouter's fallback log shows it is needed.
- Before removal, define the observed fallback coverage and observation period, explicit behavior for remaining unsupported inputs, rollback and old-link recalculation policies. An engine-only character cannot fall back to gcsim; report its unsupported configuration or failure explicitly.
- Then remove from Gscouter: gcsim's path in `score.RunIterations` and the filler search, the browser build of gcsim (`cmd/wasm`), the fork (`go.mod`'s `replace`, `library.ForkOnly`, `ForkOnlyWeapons`, the teams' `gcsim_fork`, `docs/fork.md`), cache keys and share links that record gcsim's version (`gcsimModule`, `share.GcsimFork`; old links need a recalculation policy), `regression.yml` and `cmd/compare` moved to the engine's regression, and the tests that import gcsim (`internal/config`, `internal/weights` and `internal/score` through its `parser`; `internal/chart` and `score/chasca_test.go` through its core packages) moved to the engine's APIs.
- The site's words: `about.scores` ("within 1% of gcsim"), `issue.arlecchino` once fixed, the messages in each language that name gcsim, and a `/updates` entry.
- Documents: Gscouter's `design.md`, `engine.md`, `fork.md`; here `README.md`, `CLAUDE.md`, `docs/porting.md`, `docs/verification.md`, `docs/roadmap.md`.

gscouter-optimizer was planned on the engine from the start and has nothing to remove.

### Step 6: gcsim behaviour matched on purpose

`docs/porting.md` §2 lists what was kept as gcsim does it: the order of work within a frame, mod expiry, hitlag extending mods and queues, gauges per source character, a kill returning before the element attaches, how waits count, gcsl's quirks, the order of random draws. Each is judged against the game, one at a time. A change that moves scores is a release the owner approves, with a `/updates` entry. Arlecchino is the first investigation; the reported discrepancy and its cause must be established before a correction is released.

## 6. Problems that remain

- **Hidden values right after a release** are estimates until the community measures them; such characters' results say so.
- **A weaker signal.** gcsim compared every library team automatically; game checks are samples. Regression stays complete (the engine's own scores); the claim of accuracy rests on samples, and the site's wording changes with it.
- **Rotations for new characters** are not in the gcsim database while gcsim lacks them; they are written from guides, as Lohen's were from KQM's. Source validation has no number to check them against.
- **Trust.** "The same as gcsim" is what users trust now; the site has to explain scores that differ from gcsim's.
- **Timelines and charts** need step 4.
- **The licence stays.** The engine is a port of gcsim and stays under the GNU AGPL v3 (`LICENSE`) whatever it later depends on.

## 7. For the owner to decide

1. **Existing data (§4): decided 2026-11-04.** Kept as a documented legacy exception (`legacy_gcsim` provenance); new content and step 6 corrections use the independent sources of §4.
2. The game checks of step 3: what counts as enough evidence for a new character, and for a change of step 6.
3. How far to measure the hidden values of layer 2, and where standard estimates are enough.
4. Whether to add grammar to gcsl (§3). The roadmap defers extensions during migration; a later proposal can revisit this.
5. **Enka data (§4): decided 2026-11-05.** The showcase API stays; existing store-derived mappings are legacy data; new mappings are recorded from observed showcases.
