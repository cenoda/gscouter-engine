# Source pilot: Xiangling from official pages

> Status: Step 0 pilot (`docs/independence.md` §5, `docs/independence-roadmap.md` milestone 0), observed 2026-11-05. Documentation and data only: no engine code, data, config or score changed · Last updated: 2026-11-05

Xiangling was read from HoYoverse's official HoYoWiki and compared with her `legacy_gcsim` data in the engine. Every value the page shows agrees with the legacy data at the page's display precision. The Enka mapping could not be observed (§6). The records are in [`official.json`](official.json), the rendered tables in [`hoyowiki-entry50-tables.txt`](hoyowiki-entry50-tables.txt), and the scripts in [`scripts/`](scripts/).

## 1. Verdict

- **The workflow can be used for layer 1** (multipliers, cooldowns, energy, base stats, numbers in passives and constellations). On this page it gives every value a player can reach: normal attack to level 11, skill and burst to level 13, base stats at all eight ascension breakpoints. It confirms the legacy values but cannot replace them to full precision, because the page rounds them (§4).
- **The page has to be rendered in a browser.** A plain fetch gets a 7 KB application shell with no content. Talents, constellations and ascension levels are tabs, read by clicking them on one loaded page.
- **The Enka check is unresolved for Xiangling.** The method works (HP and DEF match to float precision for four other characters, §6), but no showcase available to this pilot has her.
- **Layer 2 is still all legacy** (§7). The pilot does not change that.

## 2. How the pages were read

| # | Request | Tool | Result |
|---|---|---|---|
| 1 | `wiki.hoyolab.com/pc/genshin/entry/52` | `curl` (plain fetch) | 200, 7,262 bytes, a Nuxt shell with no article content. The ID was a guess and is not Xiangling's; the point was to see whether plain HTML carries content. It does not. |
| 2–3 | `wiki.hoyolab.com/pc/genshin/search?keyword=Xiangling` | Chromium (Playwright), twice | "Oops, looks like the page is missing!": this is not the site's search URL |
| – | Web search "HoYoWiki Xiangling" | search engine, not the site | Gave entry 50 (Xiangling) and entry 5654 (Damage Formula) |
| 4–6 | `wiki.hoyolab.com/pc/genshin/entry/50?lang=en-us` | Chromium, 3 loads | (4) text as first rendered, (5) DOM to find the tab selectors, (6) the reading: clicked the 6 talent tabs and the 8 ascension levels, saved the rendered text (`scripts/read_hoyowiki.js`) |
| 7 | `wiki.hoyolab.com/pc/genshin/entry/5654?lang=en-us` | Chromium, 1 load | Damage Formula page, rendered text |
| 8 | `enka.network/api/uid/618285856` | `curl`, User-Agent `gscouter-engine-pilot/0.1` | 200, 21,923 bytes; no Xiangling (§6) |

**What a page load requests.** A browser loading entry 50 gets its content from HoYoWiki's own JSON API: `sg-act-public-api-static.hoyolab.com/hoyowiki/genshin/wapi/entry_page?entry_page_id=50`, then `wapi/entry_pages?str_entry_page_ids=...` for linked entries and `wapi/entry_page/history`. Account, fingerprinting and analytics requests also go to `sg-public-data-api.hoyoverse.com`, `log-upload-os.hoyoverse.com`, `minor-api-os.hoyoverse.com` and Google Analytics. The host is `sg-act-public-api(-static).hoyolab.com/hoyowiki/...`, not the `sg-wiki-api...` that §4 names. In load 5 the browser's own `entry_page` response was saved as it arrived. It was used only to check that the rendered tables show every level the data has (they do: the payload also stops at Lv.11 for normal attack and Lv.13 for skill and burst). No tool called the API directly.

**Against §4's rules** (as revised in `c0ef56a`, which landed on `main` during this pilot):

- Robots.txt and rate: all within the rules. Each load was the browser's normal page load, minutes apart, and only released content was read.
- Internal API: used only as page rendering calls it.
- Caching: entry 50 was loaded three times, while working out how to read the tabs. A tool can now do it in one load (`scripts/read_hoyowiki.js`).
- **User-Agent: not followed.** The browser loads used Chromium's default (`HeadlessChrome/153`), not an identifying one. That rule arrived after the first loads. The next reader should set `userAgent` on the page, e.g. `gscouter-engine/<version> (+https://github.com/cenoda/gscouter-engine)`.
- One page per character is enough once the selectors are known. The damage formula is a separate page shared by all characters, read once.

**What kind of source it is.** HoYoWiki calls itself "the database co-built by HoYoLAB editors and players". The character page's tables are HoYoverse's presentation of the game's talent screen. The Damage Formula page is filed under "Gameplay Research" and reads as an article written by editors. This pilot records the character tables as `official` and the formula page as `official` (HoYoverse's site) with this note. **Whether a "Gameplay Research" article counts as official or as a community source is for the owner to decide** (§8).

The page does not state a game version. Its revision timestamp (`entry_page.version`) is 1765178058, i.e. 2025-12-08T07:14:18Z. Each record gives "live on the observation date" with that revision.

## 3. Values read and compared

All records give: content key, unit, game version, source URL, observation date, evidence kind, official value, legacy value and source, difference.

| Group | Official values | Legacy values compared | Agree | Differ |
|---|---|---|---|---|
| Normal attack hits 1–5, Lv.1–11 (3rd hit ×2 parts, 4th ×4) | 66 numbers | 99 | 99 | 0 |
| Charged attack, Lv.1–11 | 11 | 11 | 11 | 0 |
| Plunge (collision), low, high, Lv.1–11 | 33 | 33 | 33 | 0 |
| Guoba flame DMG, Lv.1–13 | 13 | 13 | 13 | 0 |
| Pyronado swing hits 1–3 and Pyronado DMG, Lv.1–13 | 52 | 52 | 52 | 0 |
| Stamina 25, skill CD 12 s, burst duration 10 s, burst CD 20 s, energy 80 | 5 | 5 | 5 | 0 |
| A1, A4, C1–C6: amounts, durations, conditions | 15 | 15 | 15 | 0 |
| Base HP, ATK, DEF, ascension stat (EM): Lv.1, 20, 40, 50, 60, 70, 80 (before and after ascension), 90 | 56 | 56 | 56 | 0 |
| Damage formula statements | 10 | 10 | 10 | 0 |
| Character level multiplier (transformative), Lv.60, 70, 80, 85, 90, 95, 100 | 7 | 7 | 7 | 0 |
| **Total** | | **301** | **301** | **0** |

The levels shown are every level reachable: talents 1–10 (`engine/src/profile.rs` rejects others), +3 at C3 (burst) and C5 (skill), and +1 to normal attack from Tartaglia. The engine's other source of a talent bonus, Skirk's `skill_talent_bonus`, applies only to all-Cryo/Hydro parties, so never to Xiangling.

### Talent multipliers

The page prints a percentage to three significant figures below 100% (`42.1%`) and mostly to the integer above it (`103%`, `112%`), though the plunge row keeps one decimal (`101.1%`, `135.3%`). Each legacy fraction was rounded half-up to the decimals shown and compared (`scripts/compare_talents.py`). All 208 agree. The largest differences hidden by rounding are about ±0.5 percentage points on integer-shown values (e.g. Pyronado 3-hit Lv.1: legacy 109.6%, shown 110%).

### Scalars, passives and constellations

| Key | Page | Legacy | Where |
|---|---|---|---|
| Charged attack stamina | 25.0 | 25 (polearm default) | `engine/src/sim.rs` `abil_stam_cost` |
| Guoba CD | 12.0 s | 12·60 frames | `engine/src/chars/xiangling.rs` `skill` |
| Pyronado duration / CD / energy | 10.0 s / 20.0 s / 80 | 10·60 / 20·60 frames / `energy_max` 80 | `xiangling.rs`, `data.rs` |
| A1 Crossfire | Guoba flame range +20% | fan radius 5.0 × 1.2 | `TASK_GUOBA` |
| A4 Beware, It's Super Hot! | chili when Guoba ends; pickup: ATK +10% for 10 s | ATK% 0.1, 10·60 frames, given to the active character after `a4_delay` | `TASK_A4` |
| C1 | Guoba's hits: Pyro RES −15% for 6 s | −0.15, 6·60 | `attack_cb` |
| C2 | last normal attack: Implode 2 s, then 75% ATK AoE Pyro | 120 frames, 0.75 on the 5th hit | `attack`, `attack_cb`, `TASK_C2_EXPLODE` |
| C3 / C5 | Pyronado / Guoba level +3 (max 15) | `cons: [-1, 5, 3]` | `data.rs` |
| C4 | Pyronado duration +40% | 14·60 instead of 10·60 | `TASK_SPIN_START` |
| C6 | while Pyronado lasts: party Pyro DMG +15% | `PYROP` 0.15 for the spin's duration | `TASK_SPIN_START` |
| Chef de Cuisine | cooking passive | none | not combat; not needed |

Two things the descriptions leave open, where legacy makes a choice: who picks up the A4 chili, and when (the pickup delay is a rotation parameter). Neither is a layer 1 value; both are listed in §7.

### Base stats

Legacy base stats are `base × CURVES[curve][level−1] + promotion bonus` (`engine/src/profile.rs` `char_base`), with Xiangling's `base: [911.791, 18.87648, 56.0805]`, curves `[22, 32, 22]` and her promotion table in `engine/src/data.rs`. All 56 agree with the page's integers. The largest rounding gap is ATK at Lv.80 after ascension: legacy 209.549, page 210.

| Lv (phase) | Page HP / ATK / DEF / EM | Legacy |
|---|---|---|
| 1 (0) | 912 / 19 / 56 / 0 | 911.791 / 18.876 / 56.081 / 0 |
| 20 (0) → (1) | 2342 / 48 / 144 / 0 → 3024 / 63 / 186 / 0 | 2342.391 / 48.494 / 144.071 → 3023.546 / 62.596 / 185.966 |
| 40 (1) → (2) | 4529 / 94 / 279 / 0 → 5013 / 104 / 308 / 24 | 4528.913 / 93.761 / 278.555 → 5012.891 / 103.781 / 308.322 / 24 |
| 50 (2) → (3) | 5766 / 119 / 355 / 24 → 6411 / 133 / 394 / 48 | 5766.030 / 119.373 / 354.645 → 6411.334 / 132.732 / 394.335 / 48 |
| 60 (3) → (4) | 7164 / 148 / 441 / 48 → 7648 / 158 / 470 / 48 | 7164.474 / 148.324 / 440.657 → 7648.452 / 158.344 / 470.425 |
| 70 (4) → (5) | 8401 / 174 / 517 / 48 → 8885 / 184 / 546 / 72 | 8400.680 / 173.917 / 516.691 → 8884.658 / 183.937 / 546.459 / 72 |
| 80 (5) → (6) | 9638 / 200 / 593 / 72 → 10122 / 210 / 623 / 96 | 9637.797 / 199.529 / 592.781 → 10121.775 / 209.549 / 622.549 / 96 |
| 90 (6) | 10875 / 225 / 669 / 96 | 10874.915 / 225.141 / 668.871 / 96 |

The page shows the stats only at the breakpoints. The values between them come from the shared growth curves (`CURVES` 22 and 32), which are `legacy_gcsim`. The 14 breakpoints check those curves at 8 levels; they do not give the curve at other levels.

### Damage formula page

The page (entry 5654) states:

- The general formula: `[(Base DMG × Base DMG Multiplier) + Additive Base DMG Bonus] × DEF mult × RES mult × DMG Bonus mult`.
- DEF multiplier: `(Lc+100) / [(Lc+100) + (Le+100)(1−DEF reduced)(1−DEF ignored)]`.
- RES multiplier, piecewise at 0% and 75%.
- DMG Bonus multiplier: `(100% + general + elemental + ability + DMG taken) × amplifying × CRIT`, with CRIT `100% + CRIT DMG`.
- Amplifying reactions: 200% or 150%, with EM bonus `2.78·EM/(EM+1400)`.
- Additive reactions: Aggravate 115%, Spread 125%, with EM bonus `5·EM/(EM+1200)`.
- Transformative reactions: Burgeon, Hyperbloom and Shatter 300%, Overloaded 275%, Bloom and Electro-Charged 200%, Superconduct 150%, Swirl 60%, Burning 25%. Their EM bonus is `16·EM/(EM+2000)`, and the page gives the level multiplier at Lv.60/70/80/85/90/95/100.
- It also has a Lunar reactions section and long lists of which effects fall in which term; neither was examined.

Each item matches `engine/src/sim.rs` (`calc`, `reaction_attack`, the reaction functions) and `engine/src/gcsimdata/combat.rs` `REACTION_LVL_BASE` (e.g. Lv.90: 1446.8535 vs 1446.85). Not checked: where the engine puts "Damage Taken Multiplier"; and the engine caps a DEF increase (`defadj.min(0.9)`), which the page does not mention.

## 4. Derived values: growth ratios

The roadmap's plan was to derive levels past the page's last from shared growth ratios. For Xiangling nothing needs deriving: the page stops exactly where the game does (Lv.11 for normal attack, Lv.13 for skill and burst). The legacy tables' extra levels (normal attack 12–15, skill and burst 14–15) cannot be reached by her and are not used.

The pilot still checked the "shared ratio" assumption on the page's own numbers (`scripts/compare_talents.py`). For each level n, the ratio r_n = M_n / M_1 was bounded as an interval allowed by every row of a talent, given each value's display rounding. Then the legacy ratios were tested against those intervals:

- **Skill and burst (5 rows, Lv.1–13):** one shared sequence fits all rows. Legacy's ratios 1, 1.075, 1.15, 1.25, 1.325, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 2.0, 2.125 lie inside every interval.
- **Normal and charged attack (6 rows, Lv.1–11):** one sequence fits. Legacy's (1.0814, 1.1628, …, 2.1366 at Lv.11) lies inside.
- **Plunges (3 rows):** fit the normal attack sequence to Lv.10, but **not at Lv.11**. The rows taken together allow [2.1149, 2.1198], while normal attacks need [2.1341, 2.1391]. Legacy agrees: plunge 2.1163, normal attack 2.1366. So "shared ratios" hold within a family of rows, not across all of a talent's rows. This is the caution of §4 ("do not assume it applies to every talent"), now with an example.

**Derived full precision is not possible from the page alone.** Rounded values bound the ratios to about ±0.1–0.5%, enough to check legacy and not enough to replace it. To put an `official`- or `derived`-only value in place of a legacy one, the full-precision value needs another source: a showcase does not carry multipliers, and the in-game talent screen shows the same rounding.

## 5. Differences from legacy

None. Every one of the 301 compared values agrees at the page's precision. The differences in the records are display rounding only (talents: under half of the last shown digit; base stats: under 0.5).

## 6. Enka mapping and stat check

- **Legacy mapping.** Gscouter's `internal/enka/data/character.dm.json` (the brief's `data/character.dm.json` is at this path, Gscouter `2df20b3`) has `xiangling`: `id` 10000023, `sub_id` 2301, `burst_energy_cost` 80. The 80 agrees with the page.
- **Saved showcases.** None in Gscouter has Xiangling. `internal/enka/testdata/` holds `builds.json` (10000021, 10000037), `owner.json` (10000021), `skirk.json` (10000114, 10000999) and `good.json` (GOOD format, no avatar IDs). The string `10000023` occurs in Gscouter only in `character.dm.json`.
- **One request made.** UID 618285856, chosen because it is the example UID of Enka's own API documentation and already Gscouter's `owner.json` fixture, so a public showcase meant for this use. It shows 10000021 (Amber, Lv.90), 10000032 (Bennett, 90), 10000037 (Ganyu, 90) and 10000025 (Xingqiu, 80). **No Xiangling, so her `avatarId` is unobserved.** No further request was made, under the one-request limit.
- **The stat check works**, shown on the four characters it does have (`scripts/enka_base_stats.py`, legacy `data.rs` at the showcase's level and `propMap` 1002 ascension):
  - `fightPropMap` 1 (base HP) and 7 (base DEF) match legacy to float32 precision (relative ≤ 8e−8).
  - `fightPropMap` 4 (base ATK) includes the weapon's base ATK. Subtracting the weapon's displayed `FIGHT_PROP_BASE_ATTACK` (an integer) leaves a gap of under 0.5 (Amber +0.33, Bennett +0.07, Ganyu +0.07, Xingqiu −0.34). That is the rounding of the weapon's displayed value, so a check of character base ATK needs the weapon's unrounded curve or a tolerance of 0.5.

## 7. Fields not available from these sources

| Field | Legacy value (source) | Disposition |
|---|---|---|
| Xiangling's Enka `avatarId` | 10000023 (`character.dm.json`) | **Unresolved**: needs one showcase with Xiangling (§8) |
| Enka base-stat check for Xiangling | – | **Unresolved**, with the above; method demonstrated on 4 other characters |
| Talent values to full precision | `gcsimdata/chars/xiangling.rs` | **Kept legacy**; the official page confirms them to display precision (§4) |
| Normal attack Lv.12–15, skill and burst Lv.14–15 | legacy arrays | **Not needed**: unreachable for Xiangling |
| Base-stat growth between breakpoints | `CURVES` 22, 32 | **Kept legacy**; checked at the 14 breakpoints only |
| Game version of the values | – | **Partly**: page revision 2025-12-08, no version stated |
| Guoba lifetime and breath count/timing | 438 frames, breaths at 103/203/303/403 | **Layer 2**: not on the page ("continuously breathe fire") |
| Pyronado tick interval | 73 frames | **Layer 2** |
| Frames, hitmarks, hitlag, hitboxes, radii | `ATTACK_FRAMES`, `*_HITMARK(S)`, `ATTACK_HITBOXES`, `BURST_RADIUS`, etc. | **Layer 2** (measured frames) |
| ICD tags and groups, durability (25 = 1U), infusion window | `xiangling.rs`, `INFUSE_DURABILITY` 20, `INFUSE_WINDOW` 30 | **Layer 2** (standard estimates, then tests) |
| Particles: 1 per Guoba hit, 1 s ICD | `queue_particle(1.0, …)`, `PARTICLE_ICD` 60 | **Layer 2** |
| Poise of plunges | 100 / 150 | **Layer 2** |
| A4 chili: who picks it up, when | active character after `a4_delay` | **Layer 2 / rotation input**; the description says "picking up" only |
| C2 explosion durability and pattern | 25, circle 2.0 | **Layer 2** |
| Snapshot behaviour of Guoba and Pyronado | snapshot at spawn / spin start | **Layer 2** (behaviour, not shown) |
| Whether HoYoWiki's "Gameplay Research" article is `official` | – | **Owner's decision** (§8) |

## 8. For the owner

1. **Enka observation of Xiangling.** Give a UID whose public showcase has Xiangling (yours or a consenting player's), or allow a second request. Without that, milestone 0's "Enka mapping observed from a showcase" item stays open for the pilot.
2. **Evidence kind of HoYoWiki's Damage Formula page** (filed as "Gameplay Research" on a wiki "co-built by HoYoLAB editors and players"): `official`, or a community source like KQM?
3. **Precision.** Official pages confirm legacy talent values to display precision only. Is "official, agreeing at display precision" enough to keep a legacy value, as this pilot assumes? For new characters, are display-precision values (rounded at about 0.1–0.5%) acceptable, or does each need a derivation or measurement to full precision? The 1% accuracy bar is not threatened by the rounding, but bit-exact baselines are.
