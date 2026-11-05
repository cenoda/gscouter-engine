# Probe coverage

> Status: In use (Phase 6 step 3, milestone 3a item 2) · Last updated: 2026-11-05

What the library builds and the 918 probes reach, item by item, and what nothing reaches. Phase 6 keeps every probe and its gaps (`docs/independence.md` §5, step 3); this map is the record that they are kept. The probe format is `docs/verification.md` §3.

## 1. The map

`reference/coverage` writes `reference/probes/coverage.json` from files only (no gcsim):

- every probe (`reference/probes/*.txt`), read and applied as `compare` does (a build line with nothing to replace, or a hunk that matches no place or several, is an error);
- every library team's `build.txt` and `rotation.gcsl` in a Gscouter checkout at the commit `compare.yml` pins (`GSCOUTER_COMMIT`; the tool refuses another);
- the release manifest of that checkout (`internal/engine/release/manifest.json`): which library teams the pinned release verifies;
- the engine's key catalog (`gse --catalog`): what is implemented, and the names that resolve to each key;
- the known gaps, `reference/coverage/gaps.txt`.

For each probe it records its team, its description, whether the release verifies the team, the members it changes (constellation, weapon and refinement, sets and pieces, as the probe's build gives them), the actions its rotation hunks add (each hunk's new lines' actions less its old lines', `escoffier charge`, `razor skill[hold=1]`, `keqing attack:5`) and the items it reaches. A member is changed when the probe replaces one of its lines or its hunks add one of its actions. Fifteen probes change only conditions or the order of actions (`condition-*`, `skirk-loop`, `tartaglia-melee-burst`): they have `rotation_hunks` and no actions.

The items are every implemented character at C1 to C6, every implemented weapon at R1 and R5 and every implemented set at 2 and 4 pieces. A build reaches a character's Cn when its `cons` is n or more (C6 runs C1's code), a weapon's Rn when its `refine` is n, and a set's n pieces when its `count` is n or more. Each item is:

- `library`: a verified team's library build reaches it (`library_builds` counts them; `library_builds_unverified` counts builds of the teams the release does not verify, which do not count);
- `probe`: no verified library build does, and a probe reaches it through a member it changes (probes count whether or not their team is verified: CI compares every probe);
- `gap`: neither, with a reason from the documents in `gaps.txt`;
- `gap-new`: neither, listed in `gaps.txt` as found by this map, with what the library and probes carry and no reason yet;
- `unexplained`: neither, and not in `gaps.txt`.

An item can also carry `partial` gaps: it is reached, but a part of its code is not (`docs/roadmap.md` §3, Phase 3).

The variants, the build space and the page configs are not counted: they are no longer verified (`docs/verification.md` §5).

## 2. Regenerating and checking

From `reference/`, with the engine built and Gscouter checked out at the pinned commit:

```
go run ./coverage -gscouter ../../Gscouter            # writes probes/coverage.json, prints the counts and any unexplained item
go run ./coverage -gscouter ../../Gscouter -check     # writes nothing; fails as below
```

`-check` fails when `coverage.json` differs from what the run makes, a probe it lists is gone, an item is unexplained, or `gaps.txt` calls an item unreached that is covered. CI runs it, with `go test ./coverage`, in the library job's part 0 (step "Probe coverage map is current"); it takes under a second and is not part of `compare`'s verdict. So:

- adding or changing a probe, porting a character, weapon or set, or bumping `GSCOUTER_COMMIT` regenerates the map in the same commit;
- a new item that nothing reaches needs a probe, or a line in `gaps.txt` with its reason;
- a probe that now reaches a listed gap removes the gap's line.

## 3. Counts (2026-11-05)

At `main` `f1dbe45`, Gscouter `e4d5796`, release manifest of engine `c0a3248`:

| | |
|---|---|
| Probes | 918 (906 on verified teams; the other 12 are on `nefer-lauma-columbina-nahida` and `skirk-xingqiu-candace-escoffier`) |
| Probes with rotation hunks | 164, 149 of them adding actions |
| Library teams | 4,570, 4,430 verified |
| Items | 1,412: 133 characters × 6, 252 weapons × 2, 55 sets × 2 |
| Covered by a verified library build | 975 (characters 661, weapons 218, sets 96) |
| Covered by probes only | 323 (characters 81, weapons 229, sets 13) |
| Gaps with a documented reason | 0 |
| Gaps found by the map (`gap-new`) | 114 (characters 56, weapons 57, sets 1) |
| Unexplained | 0 |
| Covered items with partial gaps | 50 |

## 4. The gaps

### Partial: reached, a part unverified

These are `docs/roadmap.md` §3's list for the last weapons and sets, item by item in `gaps.txt` (reasons `kills`, `enemy-hp`, `enemy-attacks`, `stellar`, `moondrift`, `range`, `hexerei`, `moonsign`, `bond-of-life`, `stamina`, `heal-target`, `error`): the five Blackcliff weapons, Gambler 4pc, Twin Nephrite and White Iron Greatsword (kills); Braveheart 4pc (enemy HP); Beacon of the Reed Sea and Golden Majesty's shield part on Memory of Dust, Summit Shaper, Vortex Vanquisher and The Unforged (enemy attacks; the roadmap names the passive, which the four share); Song of the Vigil and Frostbreath (Stellar reactions); Golden Frostbound Oath (Moondrifts); King's Squire (range); The Daybreak Chronicles (Hexerei); Snare Hook (Moonsign); Silvershower Heartstrings (Bond of Life); Wine and Song and Sturdy Bone (stamina); Otherworldly Story (heal target); Gest of the Mighty Wolf and Cloudforged (errors at fight setup). How to reach each is in the roadmap.

### Unreached, found by the map

None of the documents names these: `docs/verification.md` §3 said every constellation, R1 and R5 and 2 and 4 pieces had a probe, and the map shows 114 items with no verified library build and no probe. They run in the engine but no comparison checks them. Each needs a probe, as `docs/verification.md` §3 says; until then they are `gap-new` in `gaps.txt`, with what the library carries.

- **Constellations** (56): Escoffier (324 library build lines, all C0), Vesna (75), Sandrone (21), Zibai (7), Aloy (6), Illuga (4) and Varka (4) at C1 to C6, all their library builds at C0; Jean C5 and C6 (her library builds are C0 and C4); the cryo Travelers (`aethercryo`, `luminecryo`) at C1 to C6, with no library build at all.
- **Refinements** (57): weapons the library carries at one refinement only, mostly R5 or R3 with no R1 probe (Calamity of Eshu, Cinnabar Spindle, Fading Twilight, Favonius Greatsword, Festering Desire, The Catch, Rust, Sacrificial Bow and Sword, Thrilling Tales and others) or R1 with no R5 probe (Ballad of the Fjords, Polar Star, Skyward Harp, Skyward Pride, The Viridescent Hunt, Wolf's Gravestone and others). The Bell has no library build or probe at all; Mappa Mare's library builds are R3 only; Sharpshooter's Oath's one R5 build is on an unverified team (`amber-furina-bennett-rosaria`).
- **Sets** (1): Bloodstained Chivalry 4pc. The library carries it at 2 pieces (Xinyan) and its probe `bloodstained-rosaria` is 2+2.

The full list, item by item, is `gaps.txt`'s last section and the `gap-new` items of `coverage.json`.
