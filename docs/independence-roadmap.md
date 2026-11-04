# Independence execution roadmap

> Status: Planned; documentation updated 2026-11-04, no implementation milestone complete. Existing verification and approval rules remain active.

This is the execution companion to [the independence plan](independence.md), Phase 6 of [the project roadmap](roadmap.md). Milestone numbers match the plan. Engine changes belong here; site changes belong on Gscouter's own `claude/...` branches under its rules. This roadmap does not approve unresolved policy choices or a production release.

## Sequence

`0 → 1 → 2 → 3 pilot → 3 gate transition → 4 → 5`

Step 6 corrections and engine-only new characters can proceed after the step 3 transition. Evidence gathering for the pilot can start earlier; score-changing releases still follow the active rules. Existing regression checks remain active throughout.

| Milestone | Deliverables | Completion evidence |
|---|---|---|
| 0. Data policy and source pilot | Legacy extracted data policy recorded (done, 2026-11-04: documented legacy exception); one released character's source inventory; official/derived/measured/estimated classification | Every required field has evidence or an explicit unresolved disposition; official table coverage and derivations have been checked; the legacy exception is recorded in `docs/independence.md`. |
| 1. Independent data maintenance | Provenance-bearing input format and converter; maintained data and canonical key/alias catalog; replacement for `reference/gendata` | Add or update a content-data fixture without gcsim source or tools. Validate units, keys, references and source metadata. Pure ownership changes preserve existing scores. |
| 2. Site input migration | Engine metadata export and versioned parse API; Enka mapping path; preset parsing; independent boss resistance data; removal of input-path gcsim types | Existing representative builds, aliases, expressions and invalid inputs preserve intended behavior. Presets retain all build fields. A new-name fixture reaches the engine without gcsim recognizing the name. Audit imports and initialization paths, not only direct parser calls. |
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

1. Existing-data policy: decided 2026-11-04 (kept as a documented legacy exception with explicit `legacy_gcsim` provenance; `docs/independence.md` §4, §7).
2. Produce the released-character source pilot and list missing official or measured fields.
3. Inventory Gscouter runtime imports and data initialization, including boss presets and full build parsing; define the metadata/parse contract before wiring consumers.
4. Preserve the current probes and start the controlled Arlecchino investigation to inform the proposed evidence gate.
5. Finalize evidence sufficiency, provisional-result behavior and release approval at step 3b. Defer gcsl grammar extensions during migration.

## Progress reporting

For each milestone, record its status, implementation commits, validation report and remaining blockers here. Mark a milestone complete only with the completion evidence above. At this revision all milestones are planned; no simulator code, data, active accuracy rule or production approval has changed.
