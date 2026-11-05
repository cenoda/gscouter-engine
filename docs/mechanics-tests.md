# Mechanic tests from evidence

> Status: Phase 6 step 3a pilot (`docs/independence-roadmap.md`), running in CI beside the active gate, which is unchanged. Xiangling only so far · Last updated: 2026-11-05

Checks of single mechanics against public evidence, not against gcsim. They are `engine/tests/mechanics.rs`, with their fixtures in `engine/tests/mechanics/<character>/`, and run with `cargo test` (CI part 0 runs it). They do not decide whether a team moves to the engine; `reference/compare` still does (`docs/verification.md`).

## 1. How a check works

- **A controlled fixture**: one character, one enemy (Lv.100, 10% resistance), no crits (`add stats cr=-0.05`), no team buffs, one iteration, a short action list. Nothing random is left, so one iteration is the answer.
- **What it reads**: `gse`'s hits and damage by ability. The rotation can test a state through gcsl (`if .xiangling.energy ... { xiangling attack; }`), so a condition shows as a hit.
- **What it compares with**: a value worked out by hand from the cited sources (talent multiplier × ATK × DEF multiplier × RES multiplier for damage; the source's own number for counts).
- **Tolerance**: the sources' display rounding, summed to first order. A value shown to a step `s` lies within ±s/2; for Xiangling's N1, ATK 225 + 185 (each ±0.5) and 83.1% (±0.05) give ±0.30%. Never widened to fit, and never set from gcsim.
- **Known differences**: when evidence disagrees with the engine, the test asserts the engine's current value and names the evidence. A change either way fails it until this table is updated. These are step 6 candidates (`docs/independence.md` §5), changed only with an affected-team report and the owner's approval.

Source kinds are those of `docs/pilot-xiangling/`: official (HoYoverse's pages: the HoYoWiki character tables and ability text), stated (a community guide, without a measurement), measured (a community measurement with its video or sheet), community (HoYoLAB's Damage Formula page, by the owner's decision of 2026-11-05, and KQM's mechanics pages).

## 2. Xiangling

Lv.90/90, talents 10/10/10, Beginner's Protector Lv.70/70 (no passive). Values on 2026-11-05.

| Test | What | Evidence (kind) | Engine | Tolerance | Status |
|---|---|---|---|---|---|
| `normal_attack_hits_and_damage` | Hits of N1–N5: 1, 1, 2, 4, 1 | Talent table "51.5%×2", "27.9%×4" (official) | 1, 1, 2, 4, 1 | exact | agrees |
| | Damage per hit, N1–N5 | 83.1%, 83.3%, 51.5%, 27.9%, 140% (official); base ATK 225 (official); weapon ATK 185 (community stat table, no official page found); DEF and RES multipliers (community) | N1 149.65 against 149.39 (+0.17%) | ±0.30% (N1) to ±0.60% (N5) | agrees |
| `charged_attack` | One hit of 241% | official | 1 hit | ±0.45% | agrees |
| `guoba_breaths` | 4 breaths of 200% | "every 1.5 seconds 4 times" (official text); 200% (official) | 4; 360.58 against 359.54 (+0.29%) | ±0.49% | agrees |
| `pyronado_c0` | Swings 130%, 158%, 197%; spin 202% per hit | official | within tolerance | ±0.49% to ±0.63% | agrees |
| | Spin hits in 10 s at C0 | 8 standing still (KQM guide, stated); 8 cycles (frame-count sheet, measured) | 9 | exact | **known difference** (`docs/pilot-xiangling/hidden.md` §2, §3 B: −12% Xiangling DPS at C0 if 8) |
| `pyronado_c4_hits` | Spin hits in 14 s at C4 | 12 (KQM guide, stated); +40% duration (official) | 12 | exact | agrees |
| `energy` | Starts at 80; the burst takes 80 | 80 (official) | yes | exact | agrees |
| | Guoba gives 12 energy to Xiangling on field at 100% ER | one particle per breath that hits (KQM guide, stated); 3 energy per same-element particle on field (KQM energy page, community) | 12 | exact | agrees |

Not checked yet, and why:

- Damage of the C4 spin: C3 raises the burst to Lv.13, whose multipliers are official but are not in these fixtures yet.
- C1's −15% Pyro RES, A4's ATK +10%, C2's implode and C6's +15%: official amounts, but whether the hit that applies C1 already benefits from it, and who picks up the chili and when, are choices the official text does not settle (`docs/pilot-xiangling/official.md` §3). They need a recorded hit sequence first.
- Frames, hitlag, ICD and hitboxes: `docs/pilot-xiangling/hidden.md` lists what is measured; measured frames can be checked once the engine exposes per-hit frames, which these fixtures do not need.
- The weapon's base ATK is community data; an official weapon page would replace it.

## 3. Adding a check

1. Write a fixture with nothing random and nothing else acting.
2. Cite each value with its kind; derive the tolerance from the sources' rounding.
3. If the engine disagrees with the evidence, do not change the engine here: record a known difference, with what it would move.
4. Keep checks cheap: one iteration each; the whole file runs in well under a second.
