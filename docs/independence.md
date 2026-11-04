# Independence from gcsim

> Status: Plan agreed with the owner, not started (2026-11-04). Nothing here is done; the rules in `CLAUDE.md` and `docs/verification.md` still hold until each step below changes them · Last updated: 2026-11-04

How Gscouter and this engine stop depending on gcsim, in what order, and what stays. This is Phase 6 of `docs/roadmap.md`. The parts in Gscouter are listed here so the plan is in one place; they are made on Gscouter's `claude/...` branches under its own rules.

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
- **A pinned gcsim (`eb21884`, `reference/go.mod`) as a regression aid for existing content**, until the checks of §5 step 3 replace it. It does not follow gcsim's updates.

## 4. Where data comes from

### Principle

- **No datamining** (owner, 2026-11-04). Nothing is extracted from the game client, and no pre-release, beta or leaked data is used. The terms of service forbid reverse engineering, and HoYoverse has acted on it (the 2021 notice to the Honey Hunter wiki). Databases built from extracted data (Honey Hunter, genshin-db, AnimeGameData, gcsim's newly generated data) are not used either: not extracting it ourselves does not change where it comes from.
- So a new character is implemented **after its release**. Being earlier than gcsim comes from our speed, not from early data.
- **No crawler.** HoYoLAB's rules on automated access were not found. Official pages are read the way a person reads them: one page per new character, by the session that implements it.
- Each value records its source (an official page, a measurement with its video, a community test, or an estimate). Estimates are marked as such and replaced when measurements appear.

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

Until then, the results of such a character say that they include estimates.

### Learned from gcsim

gcsim implements a character with no other simulator to compare against. What to take from it:

- **Its frame measurement procedure** (gcsim `ui/packages/docs/docs/mechanics/frames.md`, `hitlag.md`, `aoe.md`): record the game at 60 fps, count frame by frame, keep the video as the evidence.
- **Its contribution checklist** (gcsim `CONTRIBUTING.md`): normal attack, charged attack, skill, burst, A1, A4, C1 to C6, hitlag, ICD, strike type, poise, hitboxes, durability, particles, frames. Ours goes in `docs/porting.md`, rewritten as how to implement content.
- **Evidence per value**, as Gscouter's character documents already keep.

What not to take: its ICD, hitlag, hitboxes, durability and particles come from the game's extracted ability data, as Lohen's did on the fork. Under the principle above they come from measurements or standard estimates instead.

### Existing data

`engine/src/data.rs` and `engine/src/gcsimdata/` (129 characters, 252 weapons, 55 sets) were generated from gcsim, whose pipeline reads extracted data. Whether to keep them as they are, or check them against HoYoWiki and replace them with official values, is the owner's decision (§7).

## 5. Steps

Dependency first (owner, 2026-11-04): new characters wait until this is far enough along; they are not implemented on the fork meanwhile. A character neither engine has simply has no library teams, so waiting costs coverage, not wrong scores. Each step keeps the two reference scores of `CLAUDE.md` to the last digit unless it says otherwise.

| Step | Work | Size | When done |
|---|---|---|---|
| 1 | Engine data and keys | small | The engine builds without gcsim's source |
| 2 | Gscouter ①②③ | medium | The site's front no longer calls gcsim code |
| 3 | Verification and approval | medium; owner's decisions | The "within 1% of gcsim" rule can be removed |
| 4 | Result page ⑤ | large | gcsim is left only as the fallback |
| 5 | Fallback, then removal | medium | gcsim is gone from the site |
| 6 | gcsim behaviour matched on purpose | per item | Accuracy starts going past gcsim's |

New characters come in after step 3, on the normal path, engine only. Before step 4 they lack timelines and charts; nothing else is special about them. Step 6 can start on existing teams as soon as step 3 is done; Arlecchino first.

### Step 1: engine data and keys

- Make `engine/src/data.rs` and `engine/src/gcsimdata/` this repository's source data: no longer regenerated, the "do not edit" header lifted, the module renamed.
- Retire `reference/gendata`.
- One key table, in `data.rs` (names, aliases, keys), from which Gscouter takes its keys too. New names are added there only.
- An input format for new content (the values of §4 with their sources) and a converter to the engine's data.

### Step 2: Gscouter ①②③

| Where | What changes |
|---|---|
| `internal/enka/gen.sh` | Enka mappings and names straight from Enka, not gcsim's UI data. Not for speed: so a new character's showcase can be read without a gcsim update. `artifact_main_gen.json` is kept as it is (main stats do not change) |
| `shortcut.*NameToKey` in `internal/config`, `library`, `pull`, `score` | The engine's key table |
| gcsim's `parser` in the same packages | The engine module parses and returns the party (an export such as `gse_parse`, through wazero) |

### Step 3: verification and approval

- **The engine's own regression**: fixed reference scores over the library, so a change that should not move scores keeps them, as the two reference configs do now.
- **Game checks**: public damage numbers, Enka stats and measured frames and ICD, kept as tests with their sources.
- **The pinned gcsim becomes an aid**: a difference from it is recorded and explained, not a failure. Arlecchino is the first case.
- **Approval revised** (owner, 2026-11-04: "of course"): `docs/phase4-approval.md` defines group A against gcsim; it is redefined on the checks above, release by release, and the engine becomes the default rather than approved team by team.
- **Then remove the rule** "within 1% of gcsim" from `CLAUDE.md` and `docs/verification.md` (owner, 2026-11-04: to be removed, not yet), with `reference/compare`, `harness`, `dump`, `manifest`'s gcsim parts and `compare.yml`'s gcsim cache.

### Step 4: result page ⑤

- The engine writes the events and stats that timelines, character details and charts need.
- `internal/timeline`, `internal/chart` (`preview.go`, `start.go`) and the character details move to it; so does `internal/score/chasca.go`, which reaches into gcsim's internals.

### Step 5: fallback, then removal

- Port what the engine refuses (`README.md`, "What it does not do"; `gse --check`) in the order Gscouter's fallback log shows it is needed.
- Then remove from Gscouter: gcsim's path in `score.RunIterations` and the filler search, the browser build of gcsim (`cmd/wasm`), the fork (`go.mod`'s `replace`, `library.ForkOnly`, `ForkOnlyWeapons`, the teams' `gcsim_fork`, `docs/fork.md`), cache keys and share links that record gcsim's version (`gcsimModule`, `share.GcsimFork`; old links need a recalculation policy), `regression.yml` and `cmd/compare` moved to the engine's regression.
- The site's words: `about.scores` ("within 1% of gcsim"), `issue.arlecchino` once fixed, the messages in each language that name gcsim, and a `/updates` entry.
- Documents: Gscouter's `design.md`, `engine.md`, `fork.md`; here `README.md`, `CLAUDE.md`, `docs/porting.md`, `docs/verification.md`, `docs/roadmap.md`.

gscouter-optimizer was planned on the engine from the start and has nothing to remove.

### Step 6: gcsim behaviour matched on purpose

`docs/porting.md` §2 lists what was kept as gcsim does it: the order of work within a frame, mod expiry, hitlag extending mods and queues, gauges per source character, a kill returning before the element attaches, how waits count, gcsl's quirks, the order of random draws. Each is judged against the game, one at a time. A change that moves scores is a release the owner approves, with a `/updates` entry. Arlecchino's −20% is first; its cause is not known yet.

## 6. Problems that remain

- **Hidden values right after a release** are estimates until the community measures them; such characters' results say so.
- **A weaker signal.** gcsim compared every library team automatically; game checks are samples. Regression stays complete (the engine's own scores); the claim of accuracy rests on samples, and the site's wording changes with it.
- **Rotations for new characters** are not in the gcsim database while gcsim lacks them; they are written from guides, as Lohen's were from KQM's. Source validation has no number to check them against.
- **Trust.** "The same as gcsim" is what users trust now; the site has to explain scores that differ from gcsim's.
- **Timelines and charts** need step 4.
- **The licence stays.** The engine is a port of gcsim and stays under the GNU AGPL v3 (`LICENSE`) whatever it later depends on.

## 7. For the owner to decide

1. The existing data (§4): keep it, or check it against HoYoWiki and replace it.
2. The game checks of step 3: what counts as enough evidence for a new character, and for a change of step 6.
3. How far to measure the hidden values of layer 2, and where standard estimates are enough.
4. Whether to add grammar to gcsl (§3).
