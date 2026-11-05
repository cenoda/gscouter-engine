# Xiangling: layer 2 source pilot

> Status: Pilot record, step 0 of Phase 6 (`docs/independence.md` §4 "Layer 2", §5 "Step 0"; `docs/independence-roadmap.md` milestone 0). This is documentation only: no engine code, score or config changed. Last updated: 2026-11-05

This document checks whether Xiangling's hidden values (ICD, durability, hitlag, strike type and poise, hitboxes, particles, frames) can come from public measurements of the game or from the standard estimates of `docs/independence.md` §4 layer 2 step 1. It diffs each one against the legacy values. The layer 1 values (talent tables, stats, descriptions) are in `docs/pilot-xiangling/official.md`.

Legacy means gcsim's `internal/characters/xiangling` at `cenoda/gcsim@eb21884` (the commit `reference/go.mod` fixes), and the engine's port of it: `engine/src/chars/xiangling.rs`, plus `engine/src/gcsimdata/chars/xiangling.rs`, which is generated from that package. Legacy values are what this pilot compares against. They are not evidence.

## 1. Sources read

These were read the way a person reads them: a handful of pages, no crawler, 2026-11-05. No datamined source was used: no Honey Hunter, genshin-db, AnimeGameData, ambr.top, or gcsim's generated data used as a source. None of the pages below says that it took a value from datamined data.

| # | Source | What it gave |
|---|---|---|
| S1 | KQM TCL evidence vault, Xiangling: <https://library.keqingmains.com/evidence/characters/pyro/xiangling> | Tested findings with video or sheet links (each cited below by its title) |
| S2 | KQM Xiangling guide (jamberry, "Updated for Version 3.4"): <https://keqingmains.com/xiangling/> | Community statements on ICD, snapshots, particles and the Pyronado hit count. Most have no evidence link |
| S3 | KQM TCL Elemental Gauge Database ("Updated for Patch 4.5"): <https://library.keqingmains.com/resources/compendiums/elemental-gauges> | Xiangling's gauges: skill press 1U, burst 1U, C2 1U. It does not state its method for each entry |
| S4 | KQM TCL Energy and Gauge Theory pages: <https://library.keqingmains.com/combat-mechanics/energy>, <https://library.keqingmains.com/combat-mechanics/elemental-effects/elemental-gauge-theory> | General rules only. Nothing specific to Xiangling |
| S5 | "Xiangling Frames" sheet (counts by Kurt#5846, videos by Kolibri#7675: <https://youtu.be/ibZji4bP7Gs>, <https://youtu.be/tb1bGGVNJoA>): <https://docs.google.com/spreadsheets/d/1tWMpn1M34w0rCutUQ5cem28fIVoFVp7YjOXtIm2Rvow> (first tab, CSV export) | Normal and charged attack hitmarks and cancels, three trials each, counted without hitlag. No game version is given |
| S6 | "Pyronado and Glacial Waltz frame counts" sheet, linked from S1's "Pyronado Proc Frames and Interactions": <https://docs.google.com/spreadsheets/d/14FzSNVsEFG6wg1oDt49b8vPr_zjmKzRJsAQ2VC71OIc> | Pyronado proc frames from five videos against large bosses |
| S7 | "Xianyun Plunge Frames" sheet (gcsim cites it for frames, credited to charliex3000): <https://docs.google.com/spreadsheets/d/16ZDmEBZOD4xeUcodxFJVuLC7IIqM9LGLilkct65mElg> | A method guide only, with no values for Xiangling |
| S8 | gcsim's procedure documents at the pinned commit: `ui/packages/docs/docs/mechanics/frames.md`, `hitlag.md`, `aoe.md`; `CONTRIBUTING.md` | How values are measured and named. These are not values |

The videos themselves were not watched. A finding is recorded here with its video link as the source cites it.

### Where gcsim's legacy values come from

The plan treats two kinds of legacy values differently: values that cite a community measurement, and values that came from extracted ability data (`docs/independence.md` §4, "Learned from gcsim").

- **Frames cite measurements.** The `docs.frames` section of gcsim's `config.yml` cites S5 twice (with Kolibri's two videos) and S7 (with charliex3000's video <https://youtu.be/WkPMCWW4NNs>). S5 matches the normal and charged frames (§2.1). S7 has no values, so the plunge frames have no checkable source.
- **Some frames have no source.** `burst.go`'s cast and spin timing carries the comments "approx 73 frames per cycle" and "TODO: anim length idk if this is accurate or not". `guoba.go`'s breath timing says "first pew pew is at 126 … then every 100" and "queue this in advance because that's how it is on live". Neither comment links a count.
- **Hitlag, hitboxes, ICD groups and durability are named as the game's own ability fields.** gcsim's `hitlag.md` describes `hitHaltTime`, `hitHaltTimeScale`, `canBeDefenseHalt` and `deployable` (the last "determined experimentally"). The `abilities` section of `config.yml` lists box and circle sizes and hitlag times that no measurement explains. The ICD group table (`pkg/core/attacks/icd_groups.dm.go`) is generated. So under §4, everything here except the frames is extracted data and not a source. The one exception is `deployable`, which gcsim says was found experimentally, but it cites no test for Xiangling.

## 2. Per-mechanic table

Evidence kinds:

- **measured**: a public test with linked video or sheet evidence.
- **stated**: a community statement without linked evidence (mostly S2). It is weaker than measured, and is counted separately in §4.
- **official**: the in-game description (layer 1), listed only where it fixes a hidden value.
- **estimated-standard**: the standard estimate of `docs/independence.md` §4 layer 2 step 1.
- **unresolved**: neither public evidence nor a defined standard estimate.

Units: frames at 60 fps, and durability in gcsim units (25 = 1U). "Agrees" compares against legacy.

### 2.1 Normal attacks (N1–N5)

| Mechanic | Unit | Legacy | Public evidence | Kind | Agrees |
|---|---|---|---|---|---|
| Hitmarks N1, N2, N3-1/2, N4-1..4, N5 | f from the start of the action | 12; 8; 11, 18; 5, 15, 24, 29; 21 | S5 (3 trials): 12; 7/8/8; 11, 18; 5, 15/15/14, 24/24/23, 29/29/28; 21 | measured | yes, within 1 f of every trial |
| Cancel to the next normal | f | N1→N2 20, N2→N3 17, N3→N4 28, N4→N5 37, N5→N1 70 | S5: 20; 16/16/17; 28; 37/37/36; 70/70/71 | measured | yes, within 1 f |
| Cancel to charged | f | N1 20, N2 17, N3 24, N4 34 | S5: 19/20/19; 17; 24; 34 | measured | yes |
| Cancel to skill, burst, dash, jump, swap | f | the last hitmark | S5 marks these "x", meaning cancellable at any point, so the hitmark is used | measured | yes |
| ICD | tag, group | normal-attack tag, default group (2.5 s / 3 hits) | none for Xiangling | estimated-standard | yes. Physical hits apply no element, so this matters only under an infusion (Chongyun's field) |
| Durability | U | 1U (25) | none | estimated-standard | yes. It matters only under an infusion |
| Hitlag | s | halt 0.03 (N1, N2, N3-1, N4-4), 0 (N3-2, N4-1..3), 0.09 (N5); scale 0.01; defense halt | none. The plan's "hitlag of the weapon class" has no published polearm table | unresolved | n/a |
| Strike type, poise | | spear (gcsim's StrikeTypeSpear); poise not set | none | unresolved | n/a. The engine keeps no strike type, and poise only for blunt attacks |
| Hitbox | box w×h on the player | N1 1.2×3; N2, N3-1, N5 1.6×3.3; N3-2, N4 1.2×3.3 | none | unresolved | n/a |

### 2.2 Charged attack and plunges

| Mechanic | Unit | Legacy | Public evidence | Kind | Agrees |
|---|---|---|---|---|---|
| CA hitmark | f | 24 | S5: 24/24/24 | measured | yes |
| CA cancels | f | →N1 67, →skill 69, →burst 67, →swap 66, →dash/jump 24 | S5: 67/67/66; 70/69/66; 67/66/67; 66/65/66; x | measured | yes |
| CA ICD | group | pole extra-attack group (0.5 s reset; element on the first hit only) | none | unresolved. The standard estimate (2.5 s / 3) differs from legacy | no (estimate ≠ legacy). Physical, so it matters only under an infusion |
| CA durability, hitlag, hitbox | | 1U; scale 0.01, no halt, defense halt, deployable; circle r 0.8 on the target | none | 1U estimated-standard (agrees); the rest unresolved | — |
| Plunge hitmarks and frames (only from Xianyun's jump) | f | low 45, high 46, collision 39; frame tables in `plunge.go` | S7 has no values | unresolved | n/a |
| Plunge poise, strike type, radius | | low 100 / high 150 poise, blunt; radius 3 / 5; collision slash, r 1 | none | unresolved | n/a |

### 2.3 Guoba (skill)

| Mechanic | Unit | Legacy | Public evidence | Kind | Agrees |
|---|---|---|---|---|---|
| Cast frames | f | 39; dash and jump 14; swap 38 | none found | unresolved | n/a |
| Spawn; breath times; lifetime | f | spawns at 13; breaths at Guoba's ticks 103, 203, 303, 403, each landing 10 f later (126 f from the cast, then every 100 f); lifetime 438 | Official text (S2): "After 2 seconds, Guoba starts breathing fire … every 1.5 seconds 4 times". That gives 120 f, then every 90 f. No frame count found | official text approximate; frames unresolved | breath count yes (4). Interval no (100 vs 90), but the text is not a frame measurement |
| ICD | tag | none: every breath applies pyro | none says so explicitly | unresolved. The standard estimate (2.5 s / 3 hits) would apply on breaths 1 and 3 only | no (estimate ≠ legacy) |
| Durability | U | 1U | S3: skill press 1U | stated (compendium, method not given) | yes; the standard estimate also agrees |
| Self-infusion | durability, f | 20 for a 30 f window (+1) at each breath | S1 "Guoba Self Applies Pyro" (Terrapin#8603, v1.6, <https://www.youtube.com/watch?v=6_-UsmXAdbU>): it exists and can be swirled | measured (that it exists); values unresolved | existence yes |
| Hitbox | | fan 60°, r 5 (6 with A1), from Guoba 1.3 in front of the player, toward the enemy | A1 "+20% flame range" is official. The base range was not found | official ratio; base unresolved | ratio yes |
| Particles | count per cast; ICD | 1 per breath that hits an enemy, 1 s particle ICD: 4 per cast | S2: "one Elemental Particle every time he hits at least one enemy, as long as one of them is left alive". S1 "Guoba No Particles On Kill" (@clevernt, v4.7, <https://youtu.be/yumGVMXXRBU>, <https://youtu.be/w5nxcOfUge8>) | per-breath rule stated; no-particle-on-kill measured | count yes (4 breaths × 1). gcsim does not model no particle on a kill; that does not matter while the enemy does not die |
| Particle arrival | f | the character's particle delay (`set_particle_delay`) | none | unresolved | n/a |
| Snapshot | | at spawn (frame 13) | S2: "both Guoba and Pyronado snapshot". S1 "Guoba Snapshots CW Stack" (yeow#5090, v2.1, a theory with video) | stated | yes |
| Two Guobas at once | | possible (no limit) | S1 "Guoba CDR Interaction" (miliana#9407, v1.4, video) | measured | yes |
| Hitlag, strike type | | none; default strike | none | unresolved | n/a |

### 2.4 Pyronado (burst)

| Mechanic | Unit | Legacy | Public evidence | Kind | Agrees |
|---|---|---|---|---|---|
| Cast frames | f | 80; swap 79 | none. gcsim: "TODO: anim length idk if this is accurate" | unresolved | n/a |
| Energy | | 80, consumed at frame 24 | 80 is official (layer 1); the frame is unresolved | — | — |
| Swing hitmarks 1–3 | f | 18, 33, 57; first spin hit at 56 | S6's first four procs per video, unlabelled, relative to the first: videos 1, 2, 3, 5: 0, 17, 41, 47 / 0, 19, 51, 70 / 0, 18, 51, 57 / 0, 19, 42, 48; video 4: 0, 30, 51, 85. The swings are against large bosses, with hitlag. Legacy relative: 0, 15, 38 (spin), 39 | measured but cannot be mapped to swings | unresolved. The order agrees with S2: the first spin hit "between the second and third swing" |
| Swing ICD | tag, group | burst tag, default group (2.5 s / 3); the three swings share it | S2: "the initial hits are subject to a standard three-hit ICD" | stated | yes; the standard estimate also agrees |
| Swing durability | U | 1U each | S3: burst 1U (no hit named) | stated | yes |
| Swing hitlag | s | halt 0.03, scale 0.01, defense halt | none | unresolved | n/a |
| Swing hitbox | | circles r 2.5, 2.5, 3 on the player | none | unresolved | n/a |
| Swing snapshot | | none: each swing reads stats at its hit | S1 "XL Pyronado Does Not Benefit From Her C6" (Sayline#5959, v2.0, Imgur a/Dvo4D9T, <https://www.youtube.com/watch?v=7ikm7JC2ueY>): "the 3rd hit of the initial swings … does benefit from her C6" | measured | yes (C6 is added at 56, swing 3 hits at 57) |
| Spin ICD | | none: every spin hit applies pyro | S2: "the actual Pyronado has no ICD on Elemental application … react every swing". Consistent with S1 "Hutao and Xiangling Vape" (60% reverse vape with Xingqiu, v1.6, sheets) | stated | yes. The standard estimate disagrees |
| Spin durability | U | 1U | S3 burst 1U. S1 "Pyronado Only Applies 1U To Cryo Regisvine Shield" (Mcpie#8672, v1.6, video) says 1U over its whole duration on that shield; that is shield-specific | stated | yes |
| Spin period | f per rotation | 73 (one hit per rotation, 9 hits at C0) | S6 videos 1–2 against Cryo Regisvine: 74.57 and 74.86 f per cycle. S1 "Pyronado Proc Frames and Interactions" (Visc, Raf, CQLQRS, phaZ; v2.4; five videos): "an enemy specific lock that lasts about 36f … constantly deal damage in 36f intervals which correspond to half a rotation" | measured | close (73 vs ≈74.7). One hit per rotation disagrees for large enemies, which take two (§3) |
| Spin duration and hit count | s; hits | 10 s (14 s at C4), with ticks at 0, 73, … ≤ duration: 9 hits at C0, 12 at C4 | Duration: official (10 s; C4 +40%). Count: S2 "a total of 8 and 12 Pyronado hits when standing still" (C0, C4). S6 "Total number of cycles 8 (7 + initial)" (constellation not stated) | duration official; count stated, and partly measured | C4 yes (12). C0 no: 9 vs 8 |
| Spin snapshot | frame | at 56, before C6 is added | S1 Sayline (above): ticks do not get C6. S1 "Bennett buff snapshotting when alt-swapping into Xiangling" (@pi45, @jamberry, v4.6, sheet): "Xiangling snapshots 46 frames from cast", without naming the action | measured | C6 yes. The snapshot frame no or unclear (56 vs 46) |
| Spin hitbox and movement | | circle r 2.5 on the player at each tick; follows the player | Official: "moves with your character". S1 "Burst Movement Interaction" (ganjo#1859, v1.4): the hit count depends on the enemy's position and size. S1 "Pyronado Reverse Direction Mechanic" (euphorysm#6156, v2.3) | hitbox unresolved; movement official | movement yes. gcsim notes "Pyronado mechanics (collision) are not properly implemented yet" |
| Spin hitlag, strike type | | none; default strike | none | unresolved | n/a |

### 2.5 Passives and constellations

| Mechanic | Legacy | Public evidence | Kind | Agrees |
|---|---|---|---|---|
| A1: Guoba range +20% | ×1.2 | official text | official | yes |
| A4: chili pepper, ATK +10% for 10 s | picked up by the active character `a4_delay` frames after Guoba ends (the user's param) | official text. The pickup timing is a rotation choice, not a hidden value | official | yes |
| C1: Pyro RES −15% for 6 s on Guoba hits | on each breath that hits | official text. S1 "Xiangling C1 Procs Via Burgeon and Overload" (Rathalos#2875, v3.6, videos) shows it applies on reaction hits too | official, measured | yes, for Guoba's own hits |
| C2: implode 2 s, then 75% ATK pyro, AoE | on the N5 hit; no ability tag; no snapshot; 1U; ICD none; circle r 2 on the target | official text. S1 "Xiangling C2 damage" (@jamberry, @peekays, v2.0, videos): no ability tag. S1 "C2 does not snapshot" (yeow#5090, v2.1, video). S3: 1U | measured, stated (1U) | yes. The radius is unresolved |
| C4: duration +40% | 14 s | official | official | yes |
| C6: party Pyro DMG +15% during Pyronado | added at 56 for the burst's duration; the spin does not snapshot it | official text; S1 Sayline (above) | official, measured | yes |

## 3. Consequential unknowns and their effect

**Method.** There is no Xiangling team in `configs/`, and the library is Gscouter's. So a National-style test team was written only in the session's scratchpad: Raiden, Xingqiu C6, Bennett C5, and Xiangling with The Catch R5 and 4 Emblem; Xiangling 9/9/9, ER +40%. The team-looped target was used: level 100, radius 2, at 0,2.4, with 1 flat particle every 480–720 f. It was run in three forms: Xiangling C6; Xiangling C0; and C6 with ER +10% (energy-starved). Each alternative was a temporary edit to `engine/src/chars/xiangling.rs`, built, run with `gse <config> 1000 150 4`, and reverted. Nothing was committed, and `git status` was clean after each one. Both reference scores still give 72,247.45 and 24,589.24.

**The numbers are model uncertainty for this one test team.** They are not a confidence interval or a library-wide result. Standard errors are about 100–250 on the team's DPS (≈0.3–0.6%). Waits are the mean seconds of Xiangling's `insufficient_energy` in the scored window.

| # | Alternative to legacy | Basis | Team DPS change (C6 / C0 / C6 low ER) | Xiangling DPS change | Xiangling energy wait (base → alt) |
|---|---|---|---|---|---|
| A | Spin gets the standard ICD (burst tag, default group: 2.5 s / 3 hits) | standard estimate of §4 layer 2 | −4.94% / −1.54% / −4.63% | −12.1% / −6.5% / −11.6% | 0.32→0.29 / 0.31→0.33 / 26.3→25.6 s |
| B | 8 spin hits at C0 (12 at C4), the S2/S6 count | public, stated and partly measured | 0.00% / **−3.41%** / 0.00% (C6 has 12 either way) | 0 / **−12.1%** / 0 | unchanged |
| C | Spin period 75 f (S6 measured ≈74.7) | measured | +2.21% / +1.15% / +3.52% | +5.0% / +3.5% / +7.6% | ≈ unchanged |
| D | Two spin hits per rotation (37 f), as S6 and S1 measured against large bosses | measured, target-dependent | **+23.6% / +16.0% / +27.9%** | **+53% / +46% / +57%** | 0.32→0.28 / — / 26.3→20.4 s |
| E | Guoba gets the standard ICD (pyro on breaths 1 and 3 only) | standard estimate | +1.44% / +1.62% / +0.26% | +3.3% / +4.3% / +1.1% | ≈ unchanged |
| F | Guoba breathes every 90 f (official text "every 1.5 s"), first breath landing at 130 f | official text, approximate | +1.51% / +2.28% / +0.65% | +4.9% / **+7.2%** / +0.8% | 0.31→0.28 at C0; 26.3→24.6 s at low ER |
| G | 3 particles per Guoba instead of 4 (a sensitivity bound only: no source suggests it) | none | −0.29% / −0.33% / **−26.8%** | +0.5% / +0.4% / **−28.1%** | 0.32→1.49 / 0.31→1.37 / **26.3→71.6 s** |

Reading the table:

- **Pyronado's per-enemy hit rate (D) is the largest unknown, and it is a modelling question, not a data question.** The measurements say that a large enemy, or one overlapping the player, takes a hit every ≈36 f (twice per rotation). gcsim's 2.5-radius circle hits once per 73 f whatever the target's size. Which applies to Gscouter's standard target (radius 2, 2.4 from the player) needs a measurement against a single enemy of that size. Until then, the legacy model is unverified for this, and a new character with an orbiting hitbox could not be estimated within 5% from standard values.
- **The C0 hit count (B) is a public disagreement with legacy.** It moves C0 Xiangling by 12% and this team by 3.4%. It is unresolved, not a reason to change the engine now: existing teams keep the 1% gcsim bar (`docs/independence.md` §7.2). It is a step 6 candidate.
- **The standard ICD estimate is wrong in direction for both of Xiangling's gadget-like attacks (A, E).** Public statements say the spin has no ICD. With the standard estimate, this test team's DPS moves −4.9% and Xiangling's −12%. Under the 5% sensitivity rule of §7.2, the standard estimate alone would mark a new character like this as highly uncertain on Xiangling's own DPS, though the team stays just inside 5%.
- **Particles (G) matter only when energy is short.** Losing one particle per Guoba costs 27% for an energy-starved build and under 0.4% otherwise. The count of 4 is a community statement plus the official breath count, not a counted measurement.
- **Timings (C, F) move DPS by 1–7%**, mostly through reaction alignment with Xingqiu's rain swords and with overload, not through hit counts. The spin period has a measurement (≈74.7 f). Guoba's breath interval has only the approximate official text.

## 4. Summary

The fields of §2 are counted below, one count per row of the tables (2.1: 9, 2.2: 6, 2.3: 11, 2.4: 15, 2.5: 6, which is 47). A row with several parts is counted by its main value: Guoba's timing row by its official breath count, the spin duration row by its stated hit count, and the spin hitbox row by its unresolved hitbox.

| Kind | Rows | Which |
|---|---|---|
| Measured (public test with evidence) | 12 | NA hitmarks and the three cancel rows (4); CA hitmark and cancels (2); Guoba self-infusion, two Guobas; swing snapshot, spin period, spin snapshot; C2 (no tag, no snapshot) |
| Stated (community, no evidence link) | 8 | Guoba durability, particles, snapshot; swing ICD and durability; spin ICD, durability, hit count |
| Official (layer 1 fixing a hidden value) | 7 | Guoba breath count and timing (approximate), Guoba range (A1 ratio); A1, A4, C1, C4, C6 |
| Estimated-standard | 3 | NA ICD, NA durability, CA durability with its hitlag and hitbox row (all equal legacy; physical) |
| Unresolved | 17 | NA hitlag, strike type, hitbox; CA ICD group; plunge frames; plunge poise and radius; skill cast frames; Guoba ICD; particle arrival; Guoba hitlag; burst cast frames; energy-consume frame; swing hitmarks, hitlag, hitbox; spin hitbox; spin hitlag |

Some rows mix kinds, so the counts are approximate; the table rows are the record. 20 of 47 rows have community evidence (12 measured, 8 stated), 7 more are fixed by the official text, 3 are standard estimates, and 17 are unresolved. Every row with public evidence agrees with legacy except four:

- spin hits at C0 (8 vs 9);
- spin hits against large enemies (two per rotation vs one);
- the snapshot frame (46 vs 56, action unnamed);
- Guoba's breath interval, if the official "every 1.5 s" is taken literally.

The standard estimate differs from legacy in three places: Guoba's ICD, the spin's ICD and the charged attack's ICD group.

## 5. Verdict: is layer 2 workable for a new character?

**Partly: yes for frames, no for the rest without new measurements.**

- **Frames work.** Community frame counts with videos exist. Where they exist they match legacy to within 1 f (S5), and gcsim's own frame values cite them. A new character's frames can be counted the same way (gcsim's `frames.md` procedure). For Xiangling, the skill and burst cast frames, Guoba's breath times and the plunges have no counted source, so even a well-studied character has gaps.
- **ICD, durability and particles work only through community statements, and the standard estimate can be wrong.** For Xiangling, the community states the values that matter (no spin ICD, 1U, one particle per breath), but mostly without linked tests. The standard estimate of 2.5 s / 3 hits would be wrong for both persistent attacks, enough to move Xiangling's DPS by 12%. A new character's persistent or gadget attacks (DoTs, summons, orbitals) need at least a reaction-count test before the standard estimate is used.
- **Hitlag, strike type, poise and hitboxes have no public source at all.** The plan's "hitlag of the weapon class" has no published table to read from. Each of these is unresolved here, and for a new character it would have to be measured from video (gcsim's `hitlag.md` and `aoe.md` describe how) or estimated by analogy with an existing character of the same weapon. An estimate by analogy is itself derived from legacy extracted data, which `docs/independence.md` §4 should rule on.
- **Geometry is the largest risk.** The Pyronado hit-rate question (§3 D, ±20–50%) is decided by hitbox and target size, which have no public measurement against a standard-size enemy. A new character whose damage depends on such geometry would exceed the 5% sensitivity check of §7.2 until it is measured.

Proposed next steps (not done here): measure Pyronado against a single enemy of Gscouter's target size (one video, counted with gcsim's procedure); count Guoba's breaths and the burst's cast frames; decide in `docs/independence.md` §4 whether "by analogy with an existing character" is an allowed estimate for hitlag and hitboxes; record the C0 spin-count disagreement as a step 6 candidate.
