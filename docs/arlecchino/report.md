# Arlecchino investigation

> Status: Milestone 3a item 4 (`docs/independence.md` §5 steps 3 and 6, `docs/independence-roadmap.md` 3a). Investigation only: no engine code, data, config or score changed · Observed 2026-11-05 · Last updated: 2026-11-05

Gscouter shows on every Arlecchino team (`issue.arlecchino`): "Arlecchino's damage is calculated too low ... Adding about 20% to this result comes close to the real number." This report traces that claim, checks her hits against official values, and reproduces the measurement it came from.

## 1. Verdict

**Status: no damage error reproduced. The ~20% traces to a looped-rotation effect on one team that no longer exists; whether the owner also had in-game evidence is open (§7).**

- **Where the 20% comes from (measured, transcript).** On 2026-10-26 a Gscouter session measured that the library team Arlecchino / Furina / Kazuha / Bennett lost 21–23% when looped against its one pass. It said why: Arlecchino waited 16 s/min for her skill's cooldown, because the rotation as written then used her skill every loop without her burst to reset it (§3). Three minutes after that report the owner asked for the note: "아를은 잘 모르고 일단 사이트에는 아를 딜이 낮게 나오는 버그가 있으며 우리도 원인 조사 중이며, 20%정도 올리면 비슷하다는 문구까지" (2026-10-26 10:53:56 UTC). It was committed as Gscouter `5e7d874`, with the −21% beside it in the same commit's `docs/design.md` (now `docs/fillers.md` §4). No other source for the 20% was found.
- **That −21% reproduces on the state of 2026-10-26** (Gscouter `5e7d874`, gcsim `2350be9`): looped 55,845 against one pass 70,061, −20.3% at 100 iterations, with the 16.5 s/min skill wait (§3). It is a rotation-loop effect: gcsim's one pass of the same rotation matches its source's DPS within 0.2%.
- **It does not reproduce now.** The library entry was later replaced by another source config whose rotation uses her skill once per loop; it keeps 104% when looped (69,193 against 66,562 one pass, gcsim, 100 iterations, 2026-11-05). Over the 171 supported Arlecchino teams (of 184 in the library) the library records a median of 103.6% kept, against 99.2% for the other supported teams (`teams/*/team.yaml`, `validation.kept`). Arlecchino teams lose less when looped than others do, not more.
- **Her hits agree with the official values.** All 211 talent multipliers on HoYoWiki agree with the engine's legacy data at the page's display precision (§4). Every one of 33 hits in a controlled fixture, recomputed by hand from official values and the official formula, is within 0.1% of gcsim (§5). The engine agrees with gcsim on that fixture to the last digit.
- **Even if her damage were 20% low, a team result would be about 15% low, not 20%.** Her median share of team DPS in the library is 73% (§6).

Nothing found justifies a score change. A correction needs evidence the investigation does not have: an in-game recording that departs from the hand computation (§7).

## 2. Sources and labels

| Label | Meaning here |
|---|---|
| official | HoYoWiki entry 5149 (Arlecchino), rendered 2026-11-05 with `scripts/read_hoyowiki.js` (the Xiangling pilot's reader, with an identifying User-Agent); the official Damage Formula page (entry 5654) as read by `docs/pilot-xiangling/official.md` |
| legacy_gcsim | `engine/src/gamedata/chars/arlecchino.rs` and gcsim `eb21884` `internal/characters/arlecchino` (Favonius Lance's base ATK; frames; hitboxes; constellations) |
| measured | runs of gcsim and `gse` recorded here |
| transcript | the Gscouter session `session_01QKJfPMuLZKu4hUVU8MxxYV` (2026-10-26), read through the session API |
| estimated | arithmetic on measured shares, not a simulation |

Two page loads of entry 5149 were made (one through the character list to find the entry id, one reading). The constellation tab did not render in the reader and was not read again: constellations stay legacy_gcsim and unverified here (§4).

## 3. The −21%: reproduction and cause

| | Gscouter `5e7d874` (2026-10-26), gcsim `2350be9` | Gscouter `e4d5796` (CI's pin), gcsim `eb21884` |
|---|---|---|
| Rotation | `arle skill;` then 4 × (… `arle charge, attack:3, dash` ×4, `attack:3, skill;` …); no burst | 4 × (`arle skill, attack;` … `arle charge, attack:3, dash` ×4, `attack:3;` …); no burst |
| One pass (source) | 70,061 (69,935) | 66,562 (66,498) |
| Looped, with fillers | 55,845, **−20.3%** | 69,193, **+4.0%** |
| Arlecchino's waits | skill_cd 16.54 s/min | none |

Commands: `go run ./cmd/validate -iter 100 -v teams/arlecchino-furina-kaedeharakazuha-bennett` in each checkout. In the old rotation her skill (30 s cooldown) comes once per pass of the loop body, which is shorter than 30 s, and nothing resets it (her burst would, but the rotation never uses it), so the looped fight stands still on it. The filler that attacks during a skill wait (added the same day) recovered only 2 points. This is the rotation as written meeting the loop (`docs/fillers.md`), not her damage.

## 4. Official values against the engine's data

`scripts/compare_talents.py` (reads `official/hoyowiki-entry5149.txt` and `engine/src/gamedata/chars/arlecchino.rs`):

| Group | Values | Differ beyond display rounding |
|---|---|---|
| Normal attack hits 1–6 (4th × 2), Lv.1–14 | 98 | 0 |
| Charged attack, plunge, low and high plunge, Lv.1–14 | 56 | 0 |
| Masque of the Red Death increase, Lv.1–14 | 14 | 0 |
| Spike, cleave, Blood-Debt Directive, Lv.1–10 | 30 | 0 |
| Balemoon Rising, Lv.1–13 | 13 | 0 |
| **Total** | **211** | **0** |

Mechanics read from the same page and checked against gcsim's code (`internal/characters/arlecchino`), all agreeing:

| Official text | gcsim / engine |
|---|---|
| Masque at Bond of Life ≥ 30% of max HP; normal, charged and plunging attacks become Pyro and cannot be overridden | `naBuff`: status while `debt ≥ 0.3 × maxHP`; `IgnoreInfusion` on all three |
| Normal attacks deal extra DMG = ATK × Masque% × current Bond % ; each hit consumes 7.5% of the Bond, at most every 0.03 s; each consumption cuts All Is Ash's CD by 0.8 s | `bondBonus` as flat DMG, `masque × debt/maxHP × ATK`; `bondConsumeCB`: −7.5%, 2-frame ICD, −48 frames of skill CD |
| Directive: 30 s, a tick every 5 s, at most 2; absorbed by a charged attack or the burst for 65% of max HP; at most 145% within 35 s of the skill, reset by the next skill | 30 s status, 2 ticks at 5 s and 10 s, absorption at frame 12 (charged) / 22 (burst), limit 1.45 × maxHP for 35 s, reset by `debtLimit` |
| A1: upgraded to Blood-Debt Due 5 s after it is applied, absorbed for 130%; 130% on defeat; within the same limit | `a1Upgrade` at 5 s, `directiveScaling = {0, 0.65, 1.3}`, `a1OnKill` |
| A4: 1% all RES per 100 ATK over 1,000, max 20% | not implemented (survival only; no effect on damage dealt) |
| Passive: 40% Pyro DMG Bonus; healed only by Balemoon Rising | `arlecchino-passive` +0.4 Pyro%; `ReceiveHeal` ignores every other heal |
| Burst: absorbs directives, Pyro DMG, clears All Is Ash's CD, heals 150% Bond + 150% ATK; CD 15 s; 60 energy | the same, the heal at frame 123 (it exceeds the Bond and clears it) |
| Base HP / ATK / DEF at Lv.90: 13,103 / 342 / 765; CRIT DMG 38.4% | the F1 fixture's max HP 17,883.1 = 13,103 + 4,780, ATK 1,640.4 (§5) |

Upstream gcsim has changed Arlecchino once since the pin: `fc65a50` (2026-11-04) guards A1's on-kill Bond behind ascension 1, which no level 90 build reaches. No damage fix exists upstream.

## 5. Controlled fixtures and hand computation

- **F1** (`fixtures/f1-solo-nocrit.txt`): Arlecchino alone, C0, talents 9/9/9, Favonius Lance R1, HP 4,780 / ATK 311 / ATK 46.6%, CRIT Rate −5% so that no hit crits; one level 100 enemy with 10% RES. Skill, wait for the directive's first tick and its upgrade, a charged attack that absorbs the Due, three six-hit strings in Masque, the burst, one more string.
- `scripts/hits/main.go` prints every hit (frame, ability, element, multiplier, flat DMG, ATK, DMG bonus, crit stats, Bond %) and every Bond change from gcsim (copy it to `reference/hits` to run). Its output on F1 is `fixtures/f1-hits-gcsim.tsv`.
- `scripts/hand_f1.py` recomputes each hit from the official values only: `(mult × ATK + flat) × (1 + DMG%) × DEF × RES`, flat = Masque 221.2% × Bond % × ATK, Bond 130% after the absorbed Due and × 0.925 per Masque hit, DEF = 190/390, RES = 0.9.

What F1 shows (measured):

- 33 hits; the largest difference from the hand computation is 0.093% (the spike, whose 25.2% multiplier the page rounds), most are under 0.04%. The flat part is exactly Masque × Bond % × ATK (Normal 0 at frame 452: 2.212 × 1.30 × 1,640.38 = 4,717.0).
- The Bond goes 130% → 29.56% over 19 Masque hits (× 0.925 each); at 29.56% Masque ends and the next two hits are Physical, as the text says. The burst's heal clears the Bond.
- Only one directive tick lands, at 5 s: the charged attack absorbs the directive before the second.
- `gse` gives the same hits and damage as gcsim to the last digit (team 2,770.5164 in both).
- Energy: no waits in F1 or T1.

- **T1** (`fixtures/t1-library.txt`): the current library team as dumped by `compare -configs`. Engine 69,007, gcsim 68,893 (1,000 iterations, window 150–450 s; +0.17%). Arlecchino 50,768 / 50,680; no Arlecchino waits.
- **T1 with her burst** (`fixtures/t1-burst.txt`): her burst after each attack string when ready. 181 of the 184 library rotations never use her burst, which looked like a candidate for a low score. It is not: the team falls to 56,358 (engine) / 56,322 (gcsim), −18%, because the burst's heal clears the Bond that Masque needs. Whether a player's real rotation gains from her burst depends on its timing, and is a rotation question, not a damage one.

## 6. Affected teams

`affected-teams.tsv`: every Arlecchino library team (184) on the engine at `main` `f1dbe45`, 500 iterations, window 150–450 s, with her share of team DPS and of her damage from normal attacks.

- Her share of team DPS: median 73.0% (34.4% to 94.4%). Normal attacks are a median 92.5% of her damage; Masque's flat DMG is most of that.
- **No candidate correction was found, so no team moves.** No engine branch was made.
- For scale only (estimated, not a correction): if her damage were 20% higher, team DPS would rise by 20% × her share, a median of +14.6% (+6.9% to +18.9%). The note's "add about 20% to this result" would overstate even that hypothesis on every team.

## 7. Open questions and the evidence that would settle them

1. **Did the 20% come from anything but the −21% loop measurement?** Only the owner can say. If it came from an in-game comparison (a player's or the owner's own), that comparison is the evidence to collect next. If it came from the 2026-10-26 report, the note's premise no longer holds (§3) and the owner may decide what the site should say.
2. **Do in-game hits match the hand computation?** A Test Run or Domain recording settles it, for one stated build (Enka UID or the in-game character screen with weapon, artifacts and talents) on a stated enemy (level, and RES if known): the damage numbers of
   - the skill's spike and cleave, a Blood-Debt Directive tick;
   - a charged attack absorbing a Blood-Debt Due (5 s after the skill), with the Bond of Life bar shown before and after;
   - a full Masque string N1–N6 right after it, without crits (or with the crits marked), and the hit where Masque ends;
   - the burst, and the Bond after its heal.
   From the build, the expected number of each hit can be printed with F1's method; a difference above about 1% on any hit would localize a mechanic (Masque's flat DMG, the Bond's consumption, the absorb amount or limit).
3. **Constellations** (C1 +100% Masque, C2 Due at once and a 900% hit, C4, C6's flat burst DMG and crit) were not read from the page (§2). A reading of the constellation tab, and for a C1+ build a recording as in 2, would check them.
4. **A4's RES** is not implemented by either simulator. It does not change damage dealt; noted for completeness.

## 8. Files

| File | What |
|---|---|
| `official/hoyowiki-entry5149.txt` | the rendered talent and ascension tabs of HoYoWiki entry 5149, 2026-11-05 |
| `scripts/read_hoyowiki.js` | the reader (the Xiangling pilot's, with the entry as an argument and an identifying User-Agent) |
| `scripts/compare_talents.py` | §4's comparison |
| `scripts/hits/main.go` | gcsim hit and Bond logger |
| `scripts/hand_f1.py` | §5's hand computation |
| `fixtures/f1-solo-nocrit.txt`, `fixtures/f1-hits-gcsim.tsv` | F1 and its hit log |
| `fixtures/t1-library.txt`, `fixtures/t1-burst.txt` | T1 and its burst variant |
| `affected-teams.tsv` | §6 |
