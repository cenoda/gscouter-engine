# Roadmap

> Status: Phases 0 to 3 done for the library; Phase 4 live in Gscouter (4,447 teams on release `211d588`, approved 2026-11-04, carried 2026-11-05); Phase 5, upkeep, now; Phase 6, independence from gcsim, under way (steps 0 to 2 done) · Last updated: 2026-11-05

What it takes for this engine to score Gscouter's results, and where that stands. The measurements and step-by-step records of the finished phases are in `docs/history.md`.

Rotation search belongs to [gscouter-optimizer](https://github.com/cenoda/gscouter-optimizer), a separate application that will use this engine ([its plan](https://github.com/cenoda/gscouter-optimizer/blob/main/docs/rotation-search.md); documentation only so far). It changes nothing below.

## 1. Where it stands

Measured 2026-11-04 (release run of `e6c8a07`, Gscouter at `e4d5796`):

| | |
|---|---|
| Library teams | 4,570; all supported by the census |
| Verified (library build and probes within 1%) | 4,447 |
| Approved by the owner, on the engine in Gscouter | 4,447 (4,436 group A, 11 group B) |
| Not verified | 121 idle rotations, 1 unsupported at the release (`amber-furina-bennett-rosaria`), 1 error in both engines (`mavuika-citlali-xilonen-bennett`) |
| Drift over 5,365 builds | −0.01% (bound ±0.2%) |
| Content | every library character (C0 to C6), 252 weapons, 55 sets, every resonance and reaction |
| Probes | 918 |

At the start (October 2026) the engine had four characters and two supported teams; the census of that time, and why characters alone were not enough, are `docs/history.md` §1.

## 2. Scope, and what done means

The engine scores a player's showcase build on a library team's rotation: the rotation is the library's, the build is the player's (any constellation, weapon and refinement, set, talent levels and stats). Library builds are the test data, not the scope.

- **Verified** (owner, 2026-11-04): a team's library build and the probes within 1% of gcsim (`docs/verification.md` §1), nothing more. Variants, build space and pages are no longer verified; their code stays for manual runs.
- **A character** is done at C0 to C6 (`MAX_CONS = 6`) with every action and param gcsim's package has, each constellation compared on at least one supported team (a library build or a probe).
- **A weapon**: refinements 1 to 5, compared at 1 and 5. **A set**: two and four pieces.
- **Support**: any build a player enters, for content the engine has. What it lacks is an error naming it, and Gscouter runs that config on gcsim.

## 3. Phases

### Phase 0: prototype (done)

Hu Tao / Xingqiu / Zhongli / Yelan and the gcsl parser, within 0.03% of gcsim; about 8× faster native and about 30× faster in WebAssembly.

### Phase 1: foundations (done)

The comparison runner (`reference/compare`, in CI), data generated from gcsim (`reference/gendata`), and the content structure (`engine/src/content.rs`: characters, weapons and sets as modules on hooks, dispatched statically). Bennett checked the structure.

### Phase 2: the rest of gcsim's systems (done, 2026-11-01)

Electro, cryo, dendro and every reaction; heals, drains and HP debt; hits on the player and shields; crystallize; the player's aura and reactions on the player; gadgets and hit patterns; gcsim's world coordinates and moving the player; Nightsoul, Hexerei, the airborne state, Lunar-Charged, Stellar-Conduct and Stellar Swirl; every resonance. Its closing run passed every library build and probe of the 496 teams supported then.

### Phase 3: content (done for the library, 2026-11-04)

Every character the library uses, then every weapon and set of gcsim's catalog, in parallel branches taken from the census (Lunar-Bloom and Lunar-Crystallize came with their characters). The last 89 weapons and 6 sets are carried by no library build and are checked by probes only, each weapon at R1 and R5 and each set at 2pc and 4pc. What those probes cannot reach runs but is unverified:

  - **Kills.** Library fights have no enemy kills: the target has no HP, and the fight ends at its duration. So the passives that need a kill never trigger: Blackcliff (Agate, Longsword, Pole, Slasher, Warbow), Gambler 4pc (skill reset), Twin Nephrite and White Iron Greatsword's heal. Their probes check base stats only. To reach them, add a probe with a target that has HP (`target ... hp=`) and the wearer on field.
  - **Enemy HP and enemy attacks.** Braveheart 4pc needs an enemy above 50% HP, which only damage mode has. Beacon of the Reed Sea's damage-taken part and Golden Majesty's shield strength bonus need the enemy to hit the player. To reach them, add a probe with an enemy that has HP and attacks.
  - **Stellar reactions from a spear.** The Stellar parts of Song of the Vigil and Frostbreath need their holder to trigger Stellar-Conduct or Stellar Swirl. In the library only Sandrone, Odette and Vesna trigger them, and none of them holds a spear. To reach them, add a hand-written team with a spear user beside Sandrone or Vesna.
  - **Moondrifts.** Golden Frostbound Oath's Lunar-Crystallize and Moondrift team parts need Moondrifts. Its probe (`goldenfrostboundoath-r*-gorou`) has none. A probe on a library team that makes Moondrifts would reach them.
  - **Range.** King's Squire's fallback onto the player needs the enemy more than 15 away. Every library target is within 15.
  - **Hexerei and Moonsign.** Daybreak Chronicles' Hexerei doubling needs a Hexerei pair. Snare Hook's Moonsign doubling needs a Moonsign team. Their probes' teams have neither. A probe on a Hexerei team (Fischl, Sucrose) and one on a Moonsign team would reach them.
  - **Bond of Life.** Silvershower Heartstrings' Bond of Life stack is not shown to be reached on Sigewinne's team. A probe on Arlecchino or another Bond of Life character would reach it.
  - **Stamina.** Wine and Song's dash stamina reduction needs the wearer's normal attack hit before a dash. Its host, Sucrose, never normal-attacks. Stamina binds in no library rotation, so Sturdy Bone's −15% is not tested either.
  - **Heal target.** Otherworldly Story heals slot 0, as gcsim's HealInfo does. On its probe the wearer is in slot 0, so it is not told apart from the wearer.
  - **Errors instead of differences.** Gest of the Mighty Wolf on Noelle, Itto or Mavuika needs gcsim's StateChange `segmented` flag. Cloudforged on Sethos, or with Shimenawa 4pc, needs gcsim's OnEnergyChange. Both are errors raised at fight setup, and `gse --check` does not list them.

### Phase 4: into Gscouter (live, 2026-11-04)

Done in Gscouter (its `docs/engine.md`): the engine's WebAssembly module runs in the browser's helper workers and, on wazero, on the server, so both give the same result. A team runs on the engine only when the pinned release verifies it and the owner has approved it (`docs/phase4-approval.md`); a config the engine refuses or fails on runs on gcsim. Engine teams run their own filler search, ranked on 50 iterations (owner, 2026-11-04). Release `211d588` is pinned (2026-11-05), with 4,447 teams approved; group B approvals carry forward when the old and new modules score a team identically.

### Phase 5: upkeep (now)

- **Following gcsim.** Every update of Gscouter's gcsim fork (game patches, fixes, new characters) is ported too, then compared again. New characters now cost twice: the fork and the engine. Phase 6 ends this; until it is far enough along, new characters wait and are not implemented on the fork (owner, 2026-11-04).
- **Releases.** Each release that changes the engine goes to the owner (`docs/phase4-approval.md` §5): approvals in group A carry forward, the rest come back for an answer.
- **Unverified teams.** The 121 idle rotations are fixed in Gscouter's library, not here; the next release then verifies them. `amber-furina-bennett-rosaria` passes `gse --check` at `1afae95`, so the next release should judge it. `mavuika-citlali-xilonen-bennett` errors in both engines (Mavuika's recast read the frame before the state ends).
- **Unverified content.** The gaps of Phase 3 above, each with the probe that would reach it.
- **Errors left on purpose** (`README.md`, "What it does not do"): ported when a player's build or a library rotation needs them, as Gscouter's fallback log shows (Gscouter `docs/engine.md` §1).
- **A lead, not a failure**: Raiden as a filler on `neuvillette-fischl-nahida-raidenshogun`'s page configs leaned +0.3% to +0.7%, in the tail of fights whose blooms go on past 150 s (`docs/history.md` §3, Phase 2, "Known inconclusive builds"). Page configs are no longer verified.
- **Source validation** (`cmd/validate` in Gscouter) stays on gcsim: it runs each entry under its source's conditions (several targets, other enemies, `hurt`), which scoring does not need.

### Phase 6: independence from gcsim (under way; planned 2026-11-04, steps 0 to 2 done 2026-11-05)

The direction is agreed with the owner; implementation has not started. The reviewed plan is [independence.md](independence.md), and the deliverables and completion gates are in [independence-roadmap.md](independence-roadmap.md).

The execution order is: (0) record the legacy-data exception (done, 2026-11-04) and demonstrate official/measurement sources on a released character; (1) make data maintenance and keys independent; (2) migrate Gscouter's input paths, full build parsing and boss data; (3) demonstrate independent verification with an evidence-backed correction, then approve and activate the replacement release gate; (4) migrate result views; (5) remove runtime fallback and the Go simulator after coverage and migration checks. Step 6 investigates intentionally matched gcsim behavior one mechanic at a time after step 3. Arlecchino is the initial investigation, not a predetermined +20% correction.

New characters wait until step 3 is complete and are then implemented only in the engine. Content sources follow the plan's no-datamining policy, with existing gcsim-generated data kept as a documented legacy exception (owner, 2026-11-04). Existing probes and regression coverage are preserved. The pinned gcsim comparison remains available as a diagnostic after mandatory agreement is removed. Current verification and approval rules remain active until the replacement gate is operational and approved.

## 4. Decisions for the owner

The earlier integration decisions below are answered. In Phase 6, keeping existing data as a legacy exception is decided (2026-11-04), as is the use of Enka data (2026-11-05: showcase API kept, new mappings observed from showcases); decisions on evidence sufficiency and estimates remain open in `docs/independence.md` §7.

1. **Scores that move.** A team on the engine scores within 1% of gcsim, not to the bit. Allowed per team, by the owner's approval (2026-11-04), with a `/updates` entry for each release that moves teams.
2. **Fillers can flip.** When two candidates are within 1%, the engines can pick different ones. Decided 2026-11-04: an engine team runs its own search, ranked on 50 iterations (Gscouter `docs/engine.md` §5).
3. **Two engines to keep.** The largest cost over time; it is Phase 5.

## 5. What it gains

- **Faster result pages.** Measured in headless Chromium on 4 CPUs (Gscouter `docs/engine.md` §5): Hu Tao / Xingqiu / Zhongli / Yelan 19.9 s on gcsim, 8.7 s on the engine with its own search; Neuvillette / Furina / Kazuha / Xilonen 21.8 s and 9.7 s.
- **More simulation in the same time.** Hundreds of iterations instead of 50, or a wider filler search, if the site wants them.
- **Faster library validation.** Estimated, not measured; source validation stays on gcsim.
