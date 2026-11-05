# Independence execution roadmap

> Status: Milestones 0 to 2 complete (2026-11-05); milestone 3a next, in a new coordinator session; 3b to 6 planned. Existing verification and approval rules remain active.

This is the execution companion to [the independence plan](independence.md), Phase 6 of [the project roadmap](roadmap.md). Milestone numbers match the plan. Engine changes belong here; site changes belong on Gscouter's own `claude/...` branches under its rules. This roadmap does not approve unresolved policy choices or a production release.

## Sequence

`0 → 1 → 2 → 3 pilot → 3 gate transition → 4 → 5`

Step 6 corrections and engine-only new characters can proceed after the step 3 transition. Evidence gathering for the pilot can start earlier; score-changing releases still follow the active rules. Existing regression checks remain active throughout.

| Milestone | Deliverables | Completion evidence |
|---|---|---|
| 0. Data policy and source pilot | Legacy extracted data policy recorded (done, 2026-11-04: documented legacy exception; Enka data, 2026-11-05); source inventory of one released character already in the legacy data; official/derived/measured/estimated classification; how official pages are read; the character's Enka mapping observed from a showcase | Every required field has evidence or an explicit unresolved disposition; official table coverage and derivations have been checked; each official value is diffed against its `legacy_gcsim` value and every difference explained; the reading method is recorded and within §4's rules; the legacy exception is recorded in `docs/independence.md`. |
| 1. Independent data maintenance | Provenance-bearing input format and converter; maintained data and canonical key/alias catalog; replacement for `reference/gendata` | Add or update a content-data fixture without gcsim source or tools. Validate units, keys, references and source metadata. Pure ownership changes preserve existing scores. |
| 2. Site input migration | Engine metadata export and versioned parse API; Enka mappings kept as legacy data, new IDs from the key table; preset parsing; independent boss resistance data; removal of input-path gcsim types | Existing representative builds, aliases, expressions and invalid inputs preserve intended behavior. Presets retain all build fields. A new-name fixture reaches the engine without gcsim recognizing the name. Audit imports and initialization paths, not only direct parser calls. gcsim's `parser` and `eval` remain in `internal/score` for the gcsim simulation path until step 5. |
| 3a. Independent verification pilot | Library regression runner; all 918 probes retained with coverage map; evidence-backed mechanic tests; Arlecchino investigation and affected-team report | CI runs the proposed suite alongside the current gate. Controlled fixtures expose attack counts, damage, buffs, reactions and energy where relevant. The Arlecchino report reproduces and isolates the discrepancy, or explicitly records it as unresolved; it never tunes toward +20%. |
| 3b. Verification and release transition | Approved evidence requirements and provisional-result policy; revised release approval, manifest and baseline process; updated active rules | The replacement CI/release path works, preserves module identity and native/Wasm parity checks, and has an approved pilot demonstrating a justified correction. An unresolved Arlecchino investigation needs another demonstrated correction before this gate. Only then remove mandatory gcsim agreement. |
| 4. Result-page migration | Versioned event/stat output; engine timelines, character details, charts and chart editor; migration of `score/chasca.go` | Consumer checks cover frame/window semantics and totals. Tracing leaves score output and random draws unchanged. Measure score-only performance and trace overhead separately. New-content pages clearly handle unavailable views until migration completes. |
| 5. Runtime removal | Fallback inventory and prioritized support work; engine-only failure handling; cache/share migration; rollback plan; removal of runtime gcsim paths and browser build | Observe fallback reasons over a recorded, chosen period; each remaining case is supported or explicitly handled. Server and browser flows work without the Go simulator. Old links recalculate under a disclosed engine version. Offline source validation and optional comparison remain available. |
| 6. Evidence-led corrections | One mechanic at a time from `porting.md`'s intentionally matched behaviors; reproduction, evidence, focused tests and release impact | Each intentional score change has a reviewed baseline update, affected-team report, owner-approved release and `/updates` entry. No unexplained baseline replacement. |

## Verification contract to finalize in step 3

- **Regression:** fixed inputs and seeds over the library plus retained probes, with versioned expected outputs. Changes meant to preserve scores keep them; intentional changes carry an explanation and reviewed baseline diff. Preserve the two reference scores until an explicitly approved correction affects them.
- **Evidence:** record build, enemy, buffs, game version and source conditions for observed damage; test individual mechanics as well as aggregate DPS. Map actions, passives, constellations, weapon refinements and set effects to evidence or a declared gap. Keep current unreachable-probe gaps visible.
- **Uncertainty:** distinguish official, derived, measured and estimated values. For consequential unknowns, compare plausible alternatives and record effects on energy waits, team order and build recommendations. Material recommendation changes keep affected outputs provisional; this is not Monte Carlo error.
- **Comparison:** keep pinned gcsim as a diagnostic for existing content, outside the future accuracy gate. Intentional differences record their evidence and affected versions. New content does not require a gcsim implementation.
- **Release:** bind reports and baseline versions to the engine module and input data. Finalize approval and rollback conditions before switching the site's default. A documentation update alone does not change current team approvals.

## Decisions and first work

1. Existing-data policy: decided 2026-11-04 (kept as a documented legacy exception with explicit `legacy_gcsim` provenance; `docs/independence.md` §4, §7). Enka data: decided 2026-11-05 (showcase API stays; store-derived mappings are legacy; new mappings read from a showcase the owner chooses; nothing recorded from users' showcases).
2. Produce the source pilot on a released character already in the legacy data, diff it against that data, and list missing official or measured fields.
3. Inventory Gscouter runtime imports and data initialization (first pass recorded in `docs/independence.md` step 2, 2026-11-05), including boss presets and full build parsing; define the metadata/parse contract before wiring consumers.
4. Preserve the current probes and start the controlled Arlecchino investigation to inform the proposed evidence gate.
5. Evidence sufficiency and provisional-result behavior for new characters: decided 2026-11-05 (`docs/independence.md` §7.2–7.3: 1% per hit, 5% provisional DPS target, 5% estimate-sensitivity check; existing teams keep 1%). Finalize release approval at step 3b. Defer gcsl grammar extensions during migration.

## Progress reporting

For each milestone, record its status, implementation commits, validation report and remaining blockers here. Mark a milestone complete only with the completion evidence above. Milestones 3 to 6 are planned; no simulator code, data, active accuracy rule or production approval has changed.

### Milestone 0: complete (2026-11-05)

- **Reports:** [`docs/pilot-xiangling/official.md`](pilot-xiangling/official.md) (layer 1), [`docs/pilot-xiangling/hidden.md`](pilot-xiangling/hidden.md) (layer 2). Commits `6013044`, `3c5b5a0`, merged in `3f2d241`, `b20d2d5`.
- **Layer 1:** 301 official values compared with `legacy_gcsim`; 301 agree at the page's display precision, none differs. The page covers every reachable talent level, so nothing was derived; growth ratios are shared within a family of rows only (plunges leave the normal-attack sequence at Lv.11). The page must be rendered in a browser; the reading method and its requests are recorded. Deviation: the pilot's browser loads used the default User-Agent (the rule changed mid-pilot).
- **Layer 2:** 47 rows: 12 measured, 8 stated, 7 fixed by official text, 3 standard estimates, 17 unresolved (hitlag, strike type and hitboxes have no public source). Public evidence disagrees with legacy in four places (C0 spin hits 8 vs 9; two spin hits per rotation against large enemies; a snapshot frame; Guoba's breath interval read literally). The standard ICD estimate is wrong for the spin and Guoba.
- **Estimate sensitivity** (a test team, against §7.2's 5% check): Pyronado hit rate team +16 to +28%, Xiangling +46 to +57%; standard ICD on the spin team −4.9%, Xiangling −12%; C0 spin count team −3.4%, Xiangling −12%; one fewer Guoba particle −27% on an energy-starved build, under 0.4% otherwise. A new character like Xiangling built on standard estimates would be marked highly uncertain until its geometry and ICD are measured.
- **Dispositions:** Xiangling's Enka `avatarId` was not observed; new IDs come from a showcase the owner chooses, read when the character is ported (§4, Enka data). Full-precision talent values and growth between breakpoints stay legacy.
- **Carried forward, not blockers for step 1:** measure Pyronado against a single enemy of Gscouter's target size; count Guoba's breaths and the burst's cast frames; the owner's decision on whether "by analogy with an existing character" is an allowed estimate for hitlag and hitboxes; the C0 spin-count disagreement as a step 6 candidate.

### Milestone 1: complete (2026-11-05)

- **Commits:** `claude/independence-data-ownership` (`be4c73f`, `c542eee`, `1050c64`, `20ae527`; CI run 477 passed) and `claude/independence-content-format` (`6652cab`; CI run 478 passed), merged together in `4f4dbf5` (CI run 479 passed on the merge).
- **Repository-owned data:** `engine/src/gcsimdata` renamed to `gamedata`; `data.rs` and all 232 data files carry a `legacy_gcsim` provenance header instead of "generated, do not edit" (values and code unchanged, checked file by file). `engine/src/provenance.rs` records any value that replaces legacy (empty so far; no row means `legacy_gcsim`; nothing in a fight reads it). `docs/porting.md` §4.
- **One key catalog:** `data.rs`'s name and key tables equal gcsim's `pkg/shortcut` at `eb21884` entry for entry (781 names); no alias was missing. `gse --catalog`, `api::catalog` and the Wasm export `gse_catalog` give versioned JSON (`catalog_version` 1) with keys, aliases, rarity, element, weapon class and burst energy, for Gscouter's step 2. Native and Wasm outputs are byte-identical (`wasm/parity.sh`).
- **Input format and converter:** `content/` (crate `gse-content`, no dependencies, binary `gsc`: `check`, `status`, `convert`, `import`); format reference `docs/content-format.md`; adding a character `docs/porting.md` §5. Xiangling's record reproduces her engine data byte for byte with the legacy selection; the official-rounded selection differs only within display rounding (at most 0.38%); her status as new content is `high_uncertainty`, as the pilot found. Every character's legacy data round-trips through `import` and `convert`.
- **`reference/gendata`** is out of the maintained workflow, kept only to reproduce the legacy data.
- **Scores:** both reference scores to the last digit (72,247.45, 24,589.24); CI passed on each branch and the merge.
- **Decided after it** (`docs/content-format.md` §8): new base stats from HoYoWiki's integers, refined from showcases; unreachable talent levels not filled; layer 2 values held in logic go into tables for new content; weapon and set conversion when first needed.
- **Open:** how hitlag and hitboxes of new content are obtained. Estimates by analogy were rejected by the owner (2026-11-05). A test on 302 sampled library builds (hitlag off against on, 1,000 iterations): of 288 compared teams, 149 moved by more than 2% and 76 by more than 5%; 10 rotations broke. So hitlag cannot default to zero; it must be measured for each new character.

### Milestone 2: complete (2026-11-05)

- **Engine:** the versioned parse API (`docs/parse-api.md`, `parse_version` 1): `gse --parse`, `api::parse`, Wasm export `gse_parse`. `claude/independence-parse-api` (`684483d`, CI run 481 passed), merged in `211d588` (CI run 482 passed). `reference/parsecheck` compares every field with gcsim `eb21884`'s parser: 5,510 configs and Gscouter's 4,817 `build.txt` files agree; the one declared difference is `add stats random` (refused as `unsupported`; no library build uses it). CI runs it in part 0.
- **Release:** `211d588` (module `db01704ce071`), run 483: 4,447 teams verified, drift −0.011% ± 0.002% over 5,365 builds.
- **Gscouter** (`main` `3959000`, the owner's go of 2026-11-05):
  - Keys from the engine's catalog (`internal/gamekeys`, byte-identical to `gse --catalog` at `8a3c580`).
  - Enka mappings kept as legacy data with their provenance; nothing recorded from users' showcases.
  - Boss resistances as Gscouter's own legacy table, checked row by row against gcsim.
  - Setup lines read with `gse_parse` (`internal/gse`); gcsim's parser kept only in a test that checks agreement.
  - Release `211d588` pinned. Group B approvals now carry forward when the old and new modules score the team identically (owner, 2026-11-05); all 11 carried.
- **Scores:** both reference scores to the last digit.
- **Left on gcsim:** chart, preview, timeline and `score/chasca.go` (step 4); scoring itself (step 5).
